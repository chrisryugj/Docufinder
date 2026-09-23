//! 파일 수집/탐색 로직
//!
//! 폴더에서 인덱싱 대상 파일 경로를 수집하고,
//! 파일 메타데이터만 DB에 저장하는 기능 제공

use crate::db;
use crate::indexer::exclusions::is_excluded_dir;
use crate::indexer::gitignore_matcher;
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

    // 루트가 git 프로젝트면 gitignore 매처 등록 (전역, 이후 watch 이벤트에서도 사용)
    if dir.join(".git").exists() {
        gitignore_matcher::global().register_root(dir);
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
            // Office 임시 파일 (~$) 및 Windows 시스템 파일 제외
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if file_name.starts_with("~$")
                || crate::indexer::exclusions::is_excluded_system_file(file_name)
            {
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

            // 탐색 도중 중첩된 git 프로젝트 발견 → 해당 루트 등록 (하위 .gitignore 존중)
            if path.join(".git").exists() {
                gitignore_matcher::global().register_root(&path);
            }

            // 상위 git 프로젝트의 .gitignore에 매치되는 폴더면 제외 (node_modules, target 등)
            if gitignore_matcher::global().is_ignored(&path, true) {
                tracing::debug!("Skipping gitignored dir: {:?}", path);
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
            // Office 임시 파일 (~$) 및 Windows 시스템 파일 제외
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if file_name.starts_with("~$")
                || crate::indexer::exclusions::is_excluded_system_file(file_name)
            {
                continue;
            }

            // gitignore 매치 (예: *.log, build/*.js 등)
            if gitignore_matcher::global().is_ignored(&path, false) {
                continue;
            }

            files.push(path);
        }
    }
}

/// 파일 메타데이터만 저장 (파일명 검색용) - 외부 호출용 래퍼
/// 반환: 저장된 파일 경로 문자열
pub fn save_file_metadata_and_cache(conn: &Connection, path: &Path) -> Result<String, IndexError> {
    // 감시 경로(manager)는 부르기 전에 벡터를 먼저 지운다(cleanup_stale_vectors)
    save_file_metadata_only(conn, path, None)?;
    Ok(path.to_string_lossy().to_string())
}

/// 경로의 청크 벡터를 지운다. 청크를 지우기 전에 불러야 한다(지운 뒤엔 청크 id 를 모른다).
/// 남기면 해제된 chunks.id 가 재사용될 때 다른 문서의 임베딩으로 오귀속된다.
pub(crate) fn remove_vectors_for_path(
    conn: &Connection,
    path: &Path,
    vector_index: Option<&crate::search::vector::VectorIndex>,
) {
    let Some(vi) = vector_index else {
        return;
    };
    if let Ok(chunk_ids) = db::get_chunk_ids_for_path(conn, &path.to_string_lossy()) {
        for chunk_id in chunk_ids {
            let _ = vi.remove(chunk_id);
        }
    }
}

/// 파일 메타데이터만 저장 (파일명 검색용). 예전 청크가 있으면 그 벡터와 함께 지운다.
pub(crate) fn save_file_metadata_only(
    conn: &Connection,
    path: &Path,
    vector_index: Option<&crate::search::vector::VectorIndex>,
) -> Result<(), IndexError> {
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
    let size = i64::try_from(metadata.len()).unwrap_or(i64::MAX);
    let modified_at = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let file_id = db::retry_on_busy(|| {
        db::upsert_file(conn, &path_str, &file_name, &file_type, size, modified_at)
    })
    .map_err(|e| IndexError::DbError(e.to_string()))?;

    // 본문을 못 읽어 메타데이터만 남기는 파일에 예전 청크가 있으면 지운다. 남겨 두면 예전 본문이
    // 계속 검색된다(나중에 암호가 걸리거나 손상된 문서가 옛 내용으로 검색되는 등). 청크가 없는
    // 파일(DLL/EXE 등)은 존재 확인 한 번으로 끝나 쓰기 락 경쟁을 만들지 않는다. 벡터도 여기서
    // 먼저 지운다 — 남기면 해제된 chunks.id 가 재사용될 때 다른 문서 임베딩이 붙는다
    // (클라우드 placeholder 로 바뀐 파일·크기 초과 등 호출부가 따로 챙기지 않던 경로들).
    let has_chunks: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM chunks WHERE file_id = ?1)",
            [file_id],
            |row| row.get(0),
        )
        .unwrap_or(false);
    if has_chunks {
        remove_vectors_for_path(conn, path, vector_index);
        db::retry_on_busy(|| db::delete_chunks_for_file_no_tx(conn, file_id))
            .map_err(|e| IndexError::DbError(e.to_string()))?;
        let _ = conn.execute(
            "UPDATE files SET fts_indexed_at = NULL, vector_indexed_at = NULL WHERE id = ?1",
            [file_id],
        );
    }

    // Lineage 부여 — 메타데이터만 수집된 파일도 즉시 그룹핑 가능하게
    if let Err(e) = crate::indexer::lineage::assign_for_file(
        conn,
        file_id,
        &path_str,
        &file_name,
        Some(modified_at),
    ) {
        tracing::warn!("lineage assign (metadata) failed for {}: {}", path_str, e);
    }

    Ok(())
}
