//! 폴더 동기화 로직
//!
//! DB와 파일시스템 간 변경분 감지 및 증분 인덱싱

use crate::constants::{OCR_IMAGE_EXTENSIONS, SUPPORTED_EXTENSIONS};
use crate::db;
use crate::indexer::collector::{collect_files, save_file_metadata_only};
use crate::indexer::pipeline::{
    save_document_to_db_fts_only_no_tx, FtsIndexingProgress, FtsProgressCallback, IndexError,
    ParseResult, CHANNEL_BUFFER_SIZE, FTS_TOKENIZER, MAX_INDEXING_ERRORS, TRANSACTION_BATCH_SIZE,
};
use crate::ocr::OcrEngine;
use crate::parsers::parse_file;
use crate::tokenizer::TextTokenizer;

use crossbeam_channel::{bounded, RecvTimeoutError};
use rayon::prelude::*;
use rusqlite::Connection;
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

    // 1. DB에서 기존 파일 메타데이터 조회
    let db_files = db::get_file_metadata_in_folder(conn, &folder_str)
        .map_err(|e| IndexError::DbError(e.to_string()))?;

    // 2. 파일시스템 스캔
    let max_file_size_bytes = if max_file_size_mb > 0 {
        max_file_size_mb * 1_048_576
    } else {
        0
    };
    let fs_files = collect_files(folder_path, recursive, cancel_flag.as_ref(), excluded_dirs);

    if cancel_flag.load(Ordering::Acquire) {
        return Ok(SyncResult {
            folder_path: folder_str,
            added: 0,
            modified: 0,
            deleted: 0,
            failed: 0,
            unchanged: 0,
            errors: vec!["Cancelled".to_string()],
        });
    }

    // 3. Diff 계산
    let mut to_update: Vec<PathBuf> = Vec::new(); // 추가 + 수정
    let mut unchanged = 0usize;

    let fs_path_set: std::collections::HashSet<String> = fs_files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    for path in &fs_files {
        let path_str = path.to_string_lossy().to_string();
        if let Some(&(db_modified, _db_size)) = db_files.get(&path_str) {
            // DB에 있음 → modified_at 비교
            if let Ok(meta) = fs::metadata(path) {
                let fs_modified = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);
                if fs_modified != db_modified {
                    to_update.push(path.clone()); // 수정됨
                } else {
                    unchanged += 1;
                }
            } else {
                unchanged += 1;
            }
        } else {
            to_update.push(path.clone()); // 새 파일
        }
    }

    // 삭제된 파일: DB에는 있지만 파일시스템에 없음
    let to_delete: Vec<String> = db_files
        .keys()
        .filter(|db_path| !fs_path_set.contains(*db_path))
        .cloned()
        .collect();

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
                let _ = conn.execute_batch("COMMIT; BEGIN");
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
                    }
                    batch_count = 0;
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
    })
}
