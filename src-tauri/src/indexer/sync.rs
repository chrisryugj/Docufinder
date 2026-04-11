//! 폴더 동기화 로직
//!
//! DB와 파일시스템 간 변경분 감지 및 증분 인덱싱

use crate::constants::{METADATA_EXCLUDED_EXTENSIONS, OCR_IMAGE_EXTENSIONS, SUPPORTED_EXTENSIONS};
use crate::db;
use crate::indexer::collector::save_file_metadata_only;
use crate::indexer::exclusions::is_excluded_dir;
use crate::indexer::pipeline::{
    save_document_to_db_fts_only_no_tx, FtsIndexingProgress, FtsProgressCallback, IndexError,
    ParseResult, CHANNEL_BUFFER_SIZE, FTS_TOKENIZER, MAX_INDEXING_ERRORS, TRANSACTION_BATCH_SIZE,
};
use crate::ocr::OcrEngine;
use crate::parsers::parse_file;
use crate::tokenizer::TextTokenizer;
use crate::utils::idle_detector;

use crossbeam_channel::{bounded, RecvTimeoutError};
use rayon::prelude::*;
use rusqlite::{params, Connection};
use std::fs;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

/// 폴더 동기화 결과
#[derive(Debug)]
pub struct SyncResult {
    pub folder_path: String,
    pub added: usize,
    pub modified: usize,
    pub deleted: usize,
    pub failed: usize,
    pub unchanged: usize,
    pub errors: Vec<String>,
    /// 사용자에 의해 취소되었는지 여부
    pub was_cancelled: bool,
}

/// 폴더 동기화 - 변경분만 인덱싱 (추가/수정/삭제 감지)
#[allow(clippy::too_many_arguments)]
pub fn sync_folder_fts(
    conn: &Connection,
    folder_path: &Path,
    recursive: bool,
    cancel_flag: Arc<AtomicBool>,
    progress_callback: Option<FtsProgressCallback>,
    max_file_size_mb: u64,
    excluded_dirs: &[String],
    ocr_engine: Option<Arc<OcrEngine>>,
) -> Result<SyncResult, IndexError> {
    use crate::utils::disk_info::{detect_disk_type, DiskSettings};

    let folder_str = folder_path.to_string_lossy().to_string();

    let max_file_size_bytes = if max_file_size_mb > 0 {
        max_file_size_mb * 1_048_576
    } else {
        0
    };

    // 1. DB 임시 테이블로 파일시스템 스냅샷 적재
    //
    // 기존: db_files HashMap (~200MB) + fs_path_set HashSet (~200MB) = ~400MB
    // 개선: SQLite TEMP TABLE에 FS 경로 적재 → SQL JOIN으로 diff 계산 (~40-80MB)
    conn.execute_batch(
        "CREATE TEMP TABLE IF NOT EXISTS _sync_fs \
         (path TEXT PRIMARY KEY NOT NULL, modified_at INTEGER NOT NULL)",
    )
    .map_err(|e| IndexError::DbError(e.to_string()))?;
    conn.execute_batch("DELETE FROM _sync_fs")
        .map_err(|e| IndexError::DbError(e.to_string()))?;

    let walker = if recursive {
        walkdir::WalkDir::new(folder_path)
    } else {
        walkdir::WalkDir::new(folder_path).max_depth(1)
    };

    // 배치 INSERT (5_000개마다 COMMIT)
    conn.execute_batch("BEGIN")
        .map_err(|e| IndexError::DbError(e.to_string()))?;
    let mut insert_stmt = conn
        .prepare("INSERT OR REPLACE INTO _sync_fs (path, modified_at) VALUES (?1, ?2)")
        .map_err(|e| IndexError::DbError(e.to_string()))?;
    let mut insert_batch_count = 0usize;

    'walk: for entry in walker
        .into_iter()
        .filter_entry(|e| {
            if e.file_type().is_dir() {
                let name = e.file_name().to_str().unwrap_or("");
                if name.starts_with('.') {
                    return false;
                }
                if is_excluded_dir(e.path(), excluded_dirs) {
                    return false;
                }
            }
            true
        })
        .filter_map(|e| e.ok())
    {
        if cancel_flag.load(Ordering::Acquire) {
            drop(insert_stmt);
            let _ = conn.execute_batch("ROLLBACK");
            let _ = conn.execute_batch("DROP TABLE IF EXISTS _sync_fs");
            return Ok(SyncResult {
                folder_path: folder_str,
                added: 0,
                modified: 0,
                deleted: 0,
                failed: 0,
                unchanged: 0,
                errors: vec![],
                was_cancelled: true,
            });
        }

        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();

        // 임시 파일 / 숨김 파일 제외
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if file_name.starts_with("~$") || file_name.starts_with('.') {
            continue 'walk;
        }

        // 메타데이터 저장 제외 확장자 (DLL/EXE/SYS 등)
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if METADATA_EXCLUDED_EXTENSIONS.contains(&ext.as_str()) {
            continue 'walk;
        }

        let path_str = path.to_string_lossy();
        let modified_at = fs::metadata(path)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        if let Err(e) = insert_stmt.execute(params![path_str.as_ref(), modified_at]) {
            tracing::warn!("Failed to insert FS path into temp table: {}", e);
            continue 'walk;
        }
        insert_batch_count += 1;

        #[allow(clippy::manual_is_multiple_of)]
        if insert_batch_count % 5_000 == 0 {
            if let Err(e) = conn.execute_batch("COMMIT; BEGIN") {
                tracing::warn!("Sync temp table batch commit failed: {}", e);
                if conn.is_autocommit() {
                    let _ = conn.execute_batch("BEGIN");
                }
            }
        }
    }

    drop(insert_stmt);
    conn.execute_batch("COMMIT")
        .map_err(|e| IndexError::DbError(e.to_string()))?;

    // 2. SQL diff: 추가/수정 대상 파일
    //    LEFT JOIN files → DB에 없거나(is_new) modified_at 불일치(수정됨)
    let mut update_stmt = conn
        .prepare(
            "SELECT t.path FROM _sync_fs t \
             LEFT JOIN files f ON t.path = f.path \
             WHERE f.path IS NULL OR t.modified_at != f.modified_at",
        )
        .map_err(|e| IndexError::DbError(e.to_string()))?;
    let to_update: Vec<PathBuf> = update_stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| IndexError::DbError(e.to_string()))?
        .filter_map(|r| r.ok())
        .map(PathBuf::from)
        .collect();
    drop(update_stmt);

    // 3. SQL diff: 삭제된 파일 (DB에 있으나 FS 임시 테이블에 없음)
    let folder_escaped_unix = db::escape_like_pattern(&folder_str.replace('\\', "/"));
    let folder_escaped_win = db::escape_like_pattern(&folder_str.replace('/', "\\"));
    let pattern_unix = format!("{}/%", folder_escaped_unix);
    let pattern_win = format!("{}\\%", folder_escaped_win);

    let mut delete_stmt = conn
        .prepare(
            "SELECT f.path FROM files f \
             WHERE (f.path LIKE ?1 ESCAPE '\\' OR f.path LIKE ?2 ESCAPE '\\') \
             AND NOT EXISTS (SELECT 1 FROM _sync_fs t WHERE t.path = f.path)",
        )
        .map_err(|e| IndexError::DbError(e.to_string()))?;
    let to_delete: Vec<String> = delete_stmt
        .query_map(params![pattern_unix, pattern_win], |row| {
            row.get::<_, String>(0)
        })
        .map_err(|e| IndexError::DbError(e.to_string()))?
        .filter_map(|r| r.ok())
        .collect();
    drop(delete_stmt);

    // 4. unchanged 카운트 (전체 FS - to_update)
    let total_fs_count = conn
        .query_row("SELECT COUNT(*) FROM _sync_fs", [], |row| {
            row.get::<_, usize>(0)
        })
        .unwrap_or(0);
    let unchanged = total_fs_count.saturating_sub(to_update.len());

    // 임시 테이블 정리
    let _ = conn.execute_batch("DROP TABLE IF EXISTS _sync_fs");

    // 파싱 가능 / 메타데이터 전용 분리
    let has_ocr = ocr_engine.is_some();
    let (to_index, to_metadata): (Vec<_>, Vec<_>) = to_update.into_iter().partition(|p| {
        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        let is_supported = SUPPORTED_EXTENSIONS.contains(&ext.as_str());
        let is_ocr_image = has_ocr && OCR_IMAGE_EXTENSIONS.contains(&ext.as_str());
        if !is_supported && !is_ocr_image {
            return false;
        }
        if max_file_size_bytes > 0 {
            if let Ok(meta) = p.metadata() {
                if meta.len() > max_file_size_bytes {
                    return false;
                }
            }
        }
        true
    });

    let added_count = to_index.len();
    let metadata_count = to_metadata.len();
    let delete_count = to_delete.len();

    tracing::info!(
        "[Sync] {} - to_index: {}, metadata_only: {}, to_delete: {}, unchanged: {}",
        folder_str,
        added_count,
        metadata_count,
        delete_count,
        unchanged
    );

    // 4. 삭제 처리
    let mut deleted = 0;
    for path in &to_delete {
        if cancel_flag.load(Ordering::Acquire) {
            break;
        }
        if let Err(e) = db::delete_file(conn, path) {
            tracing::warn!("Failed to delete stale file {}: {}", path, e);
        } else {
            deleted += 1;
        }
    }

    // 4.5 메타데이터 전용 파일 저장
    if !to_metadata.is_empty() {
        let _ = conn.execute_batch("BEGIN");
        for (i, path) in to_metadata.iter().enumerate() {
            if cancel_flag.load(Ordering::Acquire) {
                break;
            }
            let _ = save_file_metadata_only(conn, path);
            if (i + 1) % TRANSACTION_BATCH_SIZE == 0 {
                if let Err(e) = conn.execute_batch("COMMIT; BEGIN") {
                    tracing::warn!("Sync metadata batch commit failed: {}", e);
                    if conn.is_autocommit() {
                        let _ = conn.execute_batch("BEGIN");
                    }
                }
            }
        }
        let _ = conn.execute_batch("COMMIT");
    }

    // 5. 인덱싱할 파일이 없으면 바로 완료 (progress 이벤트 없이 조용히)
    if to_index.is_empty() {
        return Ok(SyncResult {
            folder_path: folder_str,
            added: 0,
            modified: 0,
            deleted,
            failed: 0,
            unchanged,
            errors: vec![],
            was_cancelled: false,
        });
    }

    // 6. 변경된 파일만 인덱싱 (기존 파이프라인 재사용)
    let disk_type = detect_disk_type(folder_path);
    let disk_settings = DiskSettings::for_disk_type(disk_type);
    let total = to_index.len();

    // 진행률 throttling
    use std::cell::Cell;
    let last_progress_time = Cell::new(std::time::Instant::now());
    let last_progress_count = Cell::new(0usize);

    let send_progress =
        |phase: &str, total: usize, processed: usize, current: Option<&str>, force: bool| {
            if let Some(ref cb) = progress_callback {
                let now = std::time::Instant::now();
                let elapsed = now.duration_since(last_progress_time.get()).as_millis() as u64;
                let files_since = processed.saturating_sub(last_progress_count.get());
                if force || elapsed >= 100 || files_since >= 10 {
                    cb(FtsIndexingProgress {
                        phase: phase.to_string(),
                        total_files: total,
                        processed_files: processed,
                        current_file: current.map(|s| s.to_string()),
                        folder_path: folder_str.clone(),
                    });
                    last_progress_time.set(now);
                    last_progress_count.set(processed);
                }
            }
        };

    send_progress("indexing", total, 0, None, true);

    // Producer: 파싱
    let (sender, receiver) = bounded::<ParseResult>(CHANNEL_BUFFER_SIZE);
    let cancel_flag_producer = cancel_flag.clone();
    let parallel_threads = disk_settings.parallel_threads;
    let throttle_ms = disk_settings.throttle_ms;

    let producer_handle = std::thread::spawn(move || {
        let pool = match rayon::ThreadPoolBuilder::new()
            .num_threads(parallel_threads)
            .build()
            .or_else(|_| rayon::ThreadPoolBuilder::new().num_threads(2).build())
        {
            Ok(pool) => pool,
            Err(e) => {
                tracing::error!("Failed to create thread pool for sync: {}", e);
                let _ = sender.send(ParseResult::Failure {
                    path: to_index.first().cloned().unwrap_or_default(),
                    error: format!("Thread pool creation failed: {}", e),
                });
                return;
            }
        };

        let ocr_ref = ocr_engine.as_ref();

        pool.install(|| {
            let _ = to_index.par_iter().try_for_each(|path| {
                if cancel_flag_producer.load(Ordering::Acquire) {
                    return Err(());
                }

                let path_clone = path.clone();
                let ocr_deref = ocr_ref.map(|e| e.as_ref());
                let result =
                    match catch_unwind(AssertUnwindSafe(|| parse_file(&path_clone, ocr_deref))) {
                        Ok(Ok(doc)) => ParseResult::Success {
                            path: path.clone(),
                            document: doc,
                        },
                        Ok(Err(e)) => ParseResult::Failure {
                            path: path.clone(),
                            error: e.to_string(),
                        },
                        Err(_) => ParseResult::Failure {
                            path: path.clone(),
                            error: "Parser panicked".to_string(),
                        },
                    };

                if throttle_ms > 0 {
                    std::thread::sleep(std::time::Duration::from_millis(throttle_ms));
                }

                sender.send(result).map_err(|_| ())
            });
        });
    });

    // Consumer: DB 저장
    let mut indexed = 0;
    let mut failed = 0;
    let mut errors: Vec<String> = Vec::new();
    let mut suppressed_errors: usize = 0;
    let mut processed = 0;
    let recv_timeout = Duration::from_millis(100);

    if let Err(e) = conn.execute_batch("BEGIN") {
        return Err(IndexError::DbError(format!(
            "Failed to begin transaction: {}",
            e
        )));
    }

    let mut batch_count = 0;
    loop {
        if cancel_flag.load(Ordering::Acquire) {
            let _ = conn.execute_batch("COMMIT");
            break;
        }

        match receiver.recv_timeout(recv_timeout) {
            Ok(result) => {
                processed += 1;
                batch_count += 1;

                match result {
                    ParseResult::Success { path, document } => {
                        let file_name = path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown");
                        send_progress("indexing", total, processed, Some(file_name), false);
                        match save_document_to_db_fts_only_no_tx(conn, &path, document, FTS_TOKENIZER.as_ref().map(|t| t as &dyn TextTokenizer)) {
                            Ok(_) => indexed += 1,
                            Err(e) => {
                                failed += 1;
                                if errors.len() < MAX_INDEXING_ERRORS {
                                    errors.push(format!("{:?}: {}", path, e));
                                } else {
                                    suppressed_errors += 1;
                                }
                            }
                        }
                    }
                    ParseResult::Failure { path, error } => {
                        if let Err(e) = save_file_metadata_only(conn, &path) {
                            tracing::warn!("Failed to save metadata for {:?}: {}", path, e);
                        }
                        failed += 1;
                        if errors.len() < MAX_INDEXING_ERRORS {
                            errors.push(format!("{:?}: {}", path, error));
                        } else {
                            suppressed_errors += 1;
                        }
                    }
                }

                if batch_count >= TRANSACTION_BATCH_SIZE {
                    if let Err(e) = conn.execute_batch("COMMIT; BEGIN") {
                        tracing::warn!("Batch commit failed: {}", e);
                        if conn.is_autocommit() {
                            let _ = conn.execute_batch("BEGIN");
                        }
                    }
                    batch_count = 0;

                    // 유휴 감지 기반 throttle: 사용자 활동 감지 시 배치당 50ms 대기
                    // (startup sync가 사용자 작업을 방해하지 않도록)
                    if !idle_detector::is_user_idle(2000) {
                        std::thread::sleep(Duration::from_millis(50));
                    }
                }
            }
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }

    if let Err(e) = conn.execute_batch("COMMIT") {
        tracing::warn!("Final commit failed: {}", e);
    }
    let _ = producer_handle.join();

    send_progress("completed", total, processed, None, true);

    if suppressed_errors > 0 {
        errors.push(format!("... 외 {}건 에러 생략", suppressed_errors));
    }

    Ok(SyncResult {
        folder_path: folder_str,
        added: indexed,
        modified: 0, // added에 포함됨 (구분은 로그로)
        deleted,
        failed,
        unchanged,
        errors,
        was_cancelled: false,
    })
}
