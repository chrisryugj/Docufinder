//! 파일 수집/탐색 로직
//!
//! 폴더에서 인덱싱 대상 파일 경로를 수집하고,
//! 파일 메타데이터만 DB에 저장하는 기능 제공

use crate::db;
use crate::indexer::exclusions::is_excluded_dir;
use crate::indexer::pipeline::IndexError;

use rusqlite::Connection;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::UNIX_EPOCH;

/// 폴더 탐색으로 파일 경로 수집
pub(crate) fn collect_files(
    dir: &Path,
    recursive: bool,
    cancel_flag: &AtomicBool,
    excluded_dirs: &[String],
) -> Vec<PathBuf> {
    let mut files = Vec::new();

    if cancel_flag.load(Ordering::Acquire) {
        return files;
    }

    if recursive {
        let mut visited = std::collections::HashSet::new();
        // 시작 디렉토리를 정규화하여 visited에 추가
        if let Ok(canonical) = dir.canonicalize() {
            visited.insert(canonical);
        }
        collect_files_recursive(dir, &mut files, &mut visited, cancel_flag, excluded_dirs);
    } else {
        // 현재 폴더만 탐색
        collect_files_shallow(dir, &mut files, cancel_flag);
    }

    files
}

/// 현재 폴더만 탐색 (하위폴더 제외)
/// 현재 폴더의 모든 파일 수집 (확장자 무관, 임시파일만 제외)
fn collect_files_shallow(dir: &Path, files: &mut Vec<PathBuf>, cancel_flag: &AtomicBool) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!("Failed to read dir {:?}: {}", dir, e);
            return;
        }
    };

    for entry in entries.flatten() {
        if cancel_flag.load(Ordering::Acquire) {
            break;
        }

        // entry.file_type() 사용 (read_dir에서 캐시됨, HDD 최적화)
        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        let path = entry.path();
        if file_type.is_file() {
            // Office 임시 파일 (~$로 시작) 제외
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if file_name.starts_with("~$") {
                continue;
            }

            files.push(path);
        }
    }
}

/// 재귀적으로 모든 파일 수집 (확장자 무관, 임시파일/숨김폴더/제외 디렉토리 제외)
fn collect_files_recursive(
    dir: &Path,
    files: &mut Vec<PathBuf>,
    visited: &mut std::collections::HashSet<PathBuf>,
    cancel_flag: &AtomicBool,
    excluded_dirs: &[String],
) {
    if cancel_flag.load(Ordering::Acquire) {
        return;
    }

    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!("Failed to read dir {:?}: {}", dir, e);
            return;
        }
    };

    for entry in entries.flatten() {
        if cancel_flag.load(Ordering::Acquire) {
            break;
        }

        // entry.file_type() 사용 (read_dir에서 캐시됨, HDD 최적화)
        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        let path = entry.path();

        if file_type.is_dir() {
            let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            // 숨김 폴더 제외
            if dir_name.starts_with('.') {
                continue;
            }

            // 제외 디렉토리 목록에 포함된 폴더 스킵
            if is_excluded_dir(&path, excluded_dirs) {
                tracing::debug!("Skipping excluded dir: {:?}", path);
                continue;
            }

            // 심볼릭 링크 순환 방지: 정규화된 경로로 중복 체크
            if let Ok(canonical) = path.canonicalize() {
                if visited.insert(canonical) {
                    collect_files_recursive(&path, files, visited, cancel_flag, excluded_dirs);
                } else {
                    tracing::debug!("Skipping already visited dir: {:?}", path);
                }
            } else if visited.insert(path.clone()) {
                collect_files_recursive(&path, files, visited, cancel_flag, excluded_dirs);
            } else {
                tracing::debug!("Skipping already visited dir (no canonical): {:?}", path);
            }
        } else if file_type.is_file() {
            // Office 임시 파일 (~$로 시작) 제외
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if file_name.starts_with("~$") {
                continue;
            }

            files.push(path);
        }
    }
}

/// 파일 메타데이터만 저장 (파일명 검색용) - 외부 호출용 래퍼
/// 반환: 저장된 파일 경로 문자열
pub fn save_file_metadata_and_cache(conn: &Connection, path: &Path) -> Result<String, IndexError> {
    save_file_metadata_only(conn, path)?;
    Ok(path.to_string_lossy().to_string())
}

/// 파일 메타데이터만 저장 (파일명 검색용)
pub(crate) fn save_file_metadata_only(conn: &Connection, path: &Path) -> Result<(), IndexError> {
    let path_str = path.to_string_lossy().to_string();

    let metadata = fs::metadata(path).map_err(|e| IndexError::IoError(e.to_string()))?;
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();
    let file_type = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let size = metadata.len() as i64;
    let modified_at = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    // stale chunks 정리는 하지 않음:
    // - 문서 파일: FTS 인덱싱 시 save_document_to_db_fts_only_no_tx에서 자동 삭제
    // - DLL/EXE 등 바이너리: 애초에 chunk가 없어 no-op DELETE만 발생 → DB 락 경쟁 원인

    db::retry_on_busy(|| {
        db::upsert_file(conn, &path_str, &file_name, &file_type, size, modified_at)
    })
    .map_err(|e| IndexError::DbError(e.to_string()))?;

    Ok(())
}
