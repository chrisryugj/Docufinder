//! 메타데이터 전용 스캔: 파일을 열지 않고 이름·크기·수정일만 저장 (파일명 검색용).

use super::{clean_path_display, clean_path_str, IndexError, MAX_INDEXING_ERRORS};
use crate::constants::METADATA_EXCLUDED_EXTENSIONS;
use crate::db;
use crate::indexer::exclusions::is_excluded_dir;

use rusqlite::Connection;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

// ==================== Phase 2: 메타데이터 전용 스캔 ====================

/// 메타데이터 스캔 진행률
#[derive(Debug, Clone, serde::Serialize)]
pub struct MetadataScanProgress {
    pub phase: String,
    pub scanned_files: usize,
    pub folder_path: String,
}

/// 메타데이터 스캔 결과
#[derive(Debug, Clone)]
pub struct MetadataScanResult {
    pub folder_path: String,
    pub files_found: usize,
    pub errors: Vec<String>,
    pub was_cancelled: bool,
}

/// 메타데이터 전용 스캔 (파일 열지 않음, < 2초 목표)
/// 파일명 검색 즉시 가능하게 함
pub fn scan_metadata_only(
    conn: &Connection,
    folder_path: &Path,
    recursive: bool,
    cancel_flag: Arc<AtomicBool>,
    progress_callback: Option<Box<dyn Fn(MetadataScanProgress) + Send + Sync>>,
    _max_file_size_mb: u64,
    excluded_dirs: &[String],
) -> Result<MetadataScanResult, IndexError> {
    let folder_str = folder_path.to_string_lossy().to_string();

    // 진행률 throttling
    use std::cell::Cell;
    let last_progress_time = Cell::new(std::time::Instant::now());
    let last_progress_count = Cell::new(0usize);
    const PROGRESS_THROTTLE_MS: u64 = 100;
    const PROGRESS_THROTTLE_FILES: usize = 100; // 메타 스캔은 빠르므로 100개 단위

    let send_progress = |phase: &str, count: usize, force: bool| {
        if let Some(ref cb) = progress_callback {
            let now = std::time::Instant::now();
            let elapsed = now.duration_since(last_progress_time.get()).as_millis() as u64;
            let files_since = count.saturating_sub(last_progress_count.get());

            if force || elapsed >= PROGRESS_THROTTLE_MS || files_since >= PROGRESS_THROTTLE_FILES {
                cb(MetadataScanProgress {
                    phase: phase.to_string(),
                    scanned_files: count,
                    folder_path: folder_str.clone(),
                });
                last_progress_time.set(now);
                last_progress_count.set(count);
            }
        }
    };

    send_progress("scanning", 0, true);

    let mut count = 0;
    let mut errors: Vec<String> = Vec::new();
    let mut suppressed_errors: usize = 0;

    // 배치 트랜잭션 (성능 최적화)
    conn.execute_batch("BEGIN")
        .map_err(|e| IndexError::DbError(e.to_string()))?;

    let mut batch_count = 0;
    const BATCH_SIZE: usize = 100;

    // WalkDir 직접 순회 (collect_files보다 메모리 효율적)
    let walker = if recursive {
        walkdir::WalkDir::new(folder_path)
    } else {
        walkdir::WalkDir::new(folder_path).max_depth(1)
    };

    // filter_entry로 제외 디렉토리 하위 전체를 건너뛰기
    for entry in walker
        .into_iter()
        .filter_entry(|e| {
            if e.file_type().is_dir() {
                let name = e.file_name().to_str().unwrap_or("");
                // 숨김 폴더 제외
                if name.starts_with('.') {
                    return false;
                }
                // 제외 디렉토리 목록 체크
                if is_excluded_dir(e.path(), excluded_dirs) {
                    return false;
                }
            }
            true
        })
        .filter_map(|e| e.ok())
    {
        if cancel_flag.load(Ordering::Acquire) {
            let _ = conn.execute_batch("COMMIT");
            send_progress("cancelled", count, true);
            return Ok(MetadataScanResult {
                folder_path: folder_str,
                files_found: count,
                errors: vec![],
                was_cancelled: true,
            });
        }

        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();

        // 임시 파일 제외
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if file_name.starts_with("~$") || file_name.starts_with('.') {
            continue;
        }

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        // 파일 크기 체크 (metadata 접근 - 파일 열지 않음)
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(e) => {
                if errors.len() < MAX_INDEXING_ERRORS {
                    errors.push(format!("{}\t{}", clean_path_display(path), e));
                } else {
                    suppressed_errors += 1;
                }
                continue;
            }
        };

        // 시스템 바이너리/임시 파일은 메타데이터 저장 제외
        // (DLL/EXE/SYS 수십만 개로 인한 DB 급팽창 + 검색 노이즈 방지)
        if METADATA_EXCLUDED_EXTENSIONS.contains(&ext.as_str()) {
            continue;
        }

        // DB에 메타데이터만 저장 (문서/데이터 파일 — 파일명 검색용)
        let path_str = path.to_string_lossy().to_string();
        let file_type = ext.clone();
        let size = i64::try_from(metadata.len()).unwrap_or(i64::MAX);
        let modified_at = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        if let Err(e) =
            db::insert_file_metadata_only(conn, &path_str, file_name, &file_type, size, modified_at)
        {
            if errors.len() < MAX_INDEXING_ERRORS {
                errors.push(format!("{}\t{}", clean_path_str(&path_str), e));
            } else {
                suppressed_errors += 1;
            }
            continue;
        }

        count += 1;
        batch_count += 1;
        send_progress("scanning", count, false);

        // 배치 커밋
        if batch_count >= BATCH_SIZE {
            if let Err(e) = conn.execute_batch("COMMIT; BEGIN") {
                tracing::warn!("Batch commit failed: {}", e);
                if conn.is_autocommit() {
                    let _ = conn.execute_batch("BEGIN");
                }
            }
            batch_count = 0;
        }
    }

    // 최종 커밋
    if let Err(e) = conn.execute_batch("COMMIT") {
        tracing::warn!("Final commit failed: {}", e);
    }

    send_progress("completed", count, true);
    tracing::info!("[MetadataScan] {} files found in {:?}", count, folder_path);

    if suppressed_errors > 0 {
        errors.push(format!("... 외 {}건 에러 생략", suppressed_errors));
    }

    Ok(MetadataScanResult {
        folder_path: folder_str,
        files_found: count,
        errors,
        was_cancelled: false,
    })
}
