//! 인덱싱 파이프라인
//!
//! 파일 파싱 → 청크 생성 → FTS5 인덱싱 → 벡터 인덱싱
//! rayon을 활용한 병렬 파싱 지원

use crate::constants::SUPPORTED_EXTENSIONS;
use crate::db;
use crate::embedder::Embedder;
use crate::parsers::{parse_file, ParsedDocument};
use crate::search::vector::VectorIndex;
use crossbeam_channel::{bounded, RecvTimeoutError};
use rayon::prelude::*;
use rusqlite::Connection;
use std::fs;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

/// 스트리밍 파이프라인 채널 버퍼 크기 - 병렬 처리 효율화를 위해 64로 증가
const CHANNEL_BUFFER_SIZE: usize = 64;

/// FTS 배치 트랜잭션 크기 - fsync 오버헤드 감소 (3~5배 성능 향상)
const TRANSACTION_BATCH_SIZE: usize = 200;

/// 단일 파일 인덱싱 (FTS + 벡터)
/// NOTE: 현재 미사용 (index_folder_streaming 사용 중)
#[allow(dead_code)]
pub fn index_file(
    conn: &Connection,
    path: &Path,
    embedder: Option<&Arc<Embedder>>,
    vector_index: Option<&Arc<VectorIndex>>,
) -> Result<IndexResult, IndexError> {
    // 1. 파일 파싱
    let document = parse_file(path).map_err(|e| IndexError::ParseError(e.to_string()))?;
    let total_chars = document.content.len();

    // 2. DB 저장 (공통 로직)
    let (chunks_count, vectors_count) =
        save_document_to_db(conn, path, document, embedder, vector_index)?;

    Ok(IndexResult {
        file_path: path.to_string_lossy().to_string(),
        chunks_count,
        vectors_count,
        total_chars,
    })
}

/// 파싱 결과 (스트리밍 파이프라인용)
enum ParseResult {
    Success {
        path: PathBuf,
        document: ParsedDocument,
    },
    Failure {
        path: PathBuf,
        error: String,
    },
}

/// 폴더 탐색으로 파일 경로 수집
fn collect_files(
    dir: &Path,
    extensions: &[&str],
    recursive: bool,
    cancel_flag: &AtomicBool,
    max_file_size_bytes: u64,
) -> Vec<PathBuf> {
    let mut files = Vec::new();

    if cancel_flag.load(Ordering::Relaxed) {
        return files;
    }

    if recursive {
        let mut visited = std::collections::HashSet::new();
        // 시작 디렉토리를 정규화하여 visited에 추가
        if let Ok(canonical) = dir.canonicalize() {
            visited.insert(canonical);
        }
        collect_files_recursive(dir, extensions, &mut files, &mut visited, cancel_flag, max_file_size_bytes);
    } else {
        // 현재 폴더만 탐색
        collect_files_shallow(dir, extensions, &mut files, cancel_flag, max_file_size_bytes);
    }

    files
}

/// 현재 폴더만 탐색 (하위폴더 제외)
fn collect_files_shallow(
    dir: &Path,
    extensions: &[&str],
    files: &mut Vec<PathBuf>,
    cancel_flag: &AtomicBool,
    max_file_size_bytes: u64,
) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!("Failed to read dir {:?}: {}", dir, e);
            return;
        }
    };

    for entry in entries.flatten() {
        if cancel_flag.load(Ordering::Relaxed) {
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

            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            if extensions.contains(&ext.as_str()) {
                // 파일 크기 제한 (0 = 무제한)
                if max_file_size_bytes > 0 {
                    if let Ok(meta) = path.metadata() {
                        if meta.len() > max_file_size_bytes {
                            tracing::debug!("Skipping large file ({} MB): {:?}", meta.len() / 1_048_576, path);
                            continue;
                        }
                    }
                }
                files.push(path);
            }
        }
    }
}

fn collect_files_recursive(
    dir: &Path,
    extensions: &[&str],
    files: &mut Vec<PathBuf>,
    visited: &mut std::collections::HashSet<PathBuf>,
    cancel_flag: &AtomicBool,
    max_file_size_bytes: u64,
) {
    if cancel_flag.load(Ordering::Relaxed) {
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
        if cancel_flag.load(Ordering::Relaxed) {
            break;
        }

        // entry.file_type() 사용 (read_dir에서 캐시됨, HDD 최적화)
        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        let path = entry.path();

        if file_type.is_dir() {
            // 숨김 폴더 제외
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with('.'))
                .unwrap_or(false)
            {
                // 심볼릭 링크 순환 방지: 정규화된 경로로 중복 체크
                if let Ok(canonical) = path.canonicalize() {
                    if visited.insert(canonical) {
                        collect_files_recursive(&path, extensions, files, visited, cancel_flag, max_file_size_bytes);
                    } else {
                        tracing::debug!("Skipping already visited dir: {:?}", path);
                    }
                } else if visited.insert(path.clone()) {
                    collect_files_recursive(&path, extensions, files, visited, cancel_flag, max_file_size_bytes);
                } else {
                    tracing::debug!("Skipping already visited dir (no canonical): {:?}", path);
                }
            }
        } else if file_type.is_file() {
            // Office 임시 파일 (~$로 시작) 제외
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if file_name.starts_with("~$") {
                continue;
            }

            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            if extensions.contains(&ext.as_str()) {
                // 파일 크기 제한 (0 = 무제한)
                if max_file_size_bytes > 0 {
                    if let Ok(meta) = path.metadata() {
                        if meta.len() > max_file_size_bytes {
                            tracing::debug!("Skipping large file ({} MB): {:?}", meta.len() / 1_048_576, path);
                            continue;
                        }
                    }
                }
                files.push(path);
            }
        }
    }
}

/// 파싱 실패 시 파일 메타데이터만 저장 (파일명 검색용)
fn save_file_metadata_only(conn: &Connection, path: &Path) -> Result<(), IndexError> {
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

    db::upsert_file(conn, &path_str, &file_name, &file_type, size, modified_at)
        .map_err(|e| IndexError::DbError(e.to_string()))?;

    Ok(())
}

/// 파싱된 문서를 DB에 저장 (FTS + 벡터) - 공통 로직
/// 반환: (chunks_count, vectors_count)
/// NOTE: index_file에서만 사용 (현재 미사용)
#[allow(dead_code)]
fn save_document_to_db(
    conn: &Connection,
    path: &Path,
    document: ParsedDocument,
    embedder: Option<&Arc<Embedder>>,
    vector_index: Option<&Arc<VectorIndex>>,
) -> Result<(usize, usize), IndexError> {
    let path_str = path.to_string_lossy().to_string();

    // 파일 메타데이터 수집
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

    conn.execute_batch("BEGIN")
        .map_err(|e| IndexError::DbError(e.to_string()))?;

    let result = (|| {
        // 파일 정보 DB 저장
        let file_id = db::upsert_file(conn, &path_str, &file_name, &file_type, size, modified_at)
            .map_err(|e| IndexError::DbError(e.to_string()))?;

        // 기존 청크 조회
        let old_chunk_ids = db::get_chunk_ids_for_file(conn, file_id)
            .map_err(|e| IndexError::DbError(e.to_string()))?;

        // _no_tx 버전 사용: 이미 BEGIN 트랜잭션 내에서 실행 중
        db::delete_chunks_for_file_no_tx(conn, file_id)
            .map_err(|e| IndexError::DbError(e.to_string()))?;

        // 청크 저장 + FTS 인덱싱
        // 성능 최적화: into_iter()로 소비하여 clone() 제거 (메모리 20% 절감)
        let chunks_count = document.chunks.len();
        let mut chunk_ids: Vec<i64> = Vec::with_capacity(chunks_count);
        let mut chunk_contents: Vec<String> = Vec::with_capacity(chunks_count);

        for (idx, chunk) in document.chunks.into_iter().enumerate() {
            let chunk_id = db::insert_chunk(
                conn,
                file_id,
                idx,
                &chunk.content,
                chunk.start_offset,
                chunk.end_offset,
                chunk.page_number,
                chunk.location_hint.as_deref(),
            )
            .map_err(|e| IndexError::DbError(e.to_string()))?;

            chunk_ids.push(chunk_id);
            chunk_contents.push(chunk.content);  // move, not clone
        }

        Ok((old_chunk_ids, chunk_ids, chunk_contents, chunks_count))
    })();

    let (old_chunk_ids, chunk_ids, chunk_contents, chunks_count) = match result {
        Ok(data) => data,
        Err(err) => {
            let _ = conn.execute_batch("ROLLBACK");
            return Err(err);
        }
    };

    if let Err(e) = conn.execute_batch("COMMIT") {
        let _ = conn.execute_batch("ROLLBACK");
        return Err(IndexError::DbError(e.to_string()));
    }

    if let Some(vi) = vector_index {
        for chunk_id in &old_chunk_ids {
            vi.remove(*chunk_id).ok();
        }
    }

    // 벡터 인덱싱 (락 불필요 - &self로 호출)
    let vectors_count = if let (Some(emb), Some(vi)) = (embedder, vector_index) {
        match emb.embed_batch(&chunk_contents) {
            Ok(embeddings) => {
                for (chunk_id, embedding) in chunk_ids.iter().zip(embeddings.iter()) {
                    if let Err(e) = vi.add(*chunk_id, embedding) {
                        tracing::warn!("Failed to add vector for chunk {}: {}", chunk_id, e);
                    }
                }
                chunk_ids.len()
            }
            Err(e) => {
                tracing::warn!("Failed to embed chunks for {}: {}", path_str, e);
                0
            }
        }
    } else {
        0
    };

    tracing::debug!(
        "Indexed: {} ({} chunks, {} vectors)",
        path_str,
        chunks_count,
        vectors_count
    );

    Ok((chunks_count, vectors_count))
}

// ==================== 2단계 인덱싱: FTS 전용 ====================

/// FTS 인덱싱 진행률 정보
#[derive(Debug, Clone, serde::Serialize)]
pub struct FtsIndexingProgress {
    pub phase: String,
    pub total_files: usize,
    pub processed_files: usize,
    pub current_file: Option<String>,
    pub folder_path: String,
}

/// FTS 진행률 콜백 타입
pub type FtsProgressCallback = Box<dyn Fn(FtsIndexingProgress) + Send + Sync>;

/// 폴더 인덱싱 - FTS만 (1단계, 벡터 제외)
/// skip_indexed: true이면 이미 fts_indexed_at이 있는 파일은 건너뜀 (resume 용)
pub fn index_folder_fts_only(
    conn: &Connection,
    folder_path: &Path,
    recursive: bool,
    cancel_flag: Arc<AtomicBool>,
    progress_callback: Option<FtsProgressCallback>,
    max_file_size_mb: u64,
) -> Result<FolderIndexResult, IndexError> {
    index_folder_fts_impl(conn, folder_path, recursive, cancel_flag, progress_callback, max_file_size_mb, false)
}

/// 폴더 인덱싱 재개 - 이미 인덱싱된 파일 스킵
pub fn resume_folder_fts(
    conn: &Connection,
    folder_path: &Path,
    recursive: bool,
    cancel_flag: Arc<AtomicBool>,
    progress_callback: Option<FtsProgressCallback>,
    max_file_size_mb: u64,
) -> Result<FolderIndexResult, IndexError> {
    index_folder_fts_impl(conn, folder_path, recursive, cancel_flag, progress_callback, max_file_size_mb, true)
}

fn index_folder_fts_impl(
    conn: &Connection,
    folder_path: &Path,
    recursive: bool,
    cancel_flag: Arc<AtomicBool>,
    progress_callback: Option<FtsProgressCallback>,
    max_file_size_mb: u64,
    skip_indexed: bool,
) -> Result<FolderIndexResult, IndexError> {
    use crate::utils::disk_info::{detect_disk_type, DiskSettings};

    let folder_str = folder_path.to_string_lossy().to_string();

    // 실제 디스크 타입에 맞춘 스레드 수 조정 (HDD: 2, SSD: 4)
    let disk_type = detect_disk_type(folder_path);
    let disk_settings = DiskSettings::for_disk_type(disk_type);
    tracing::info!(
        "[FTS] Disk: {:?}, threads: {}, throttle: {}ms",
        disk_type,
        disk_settings.parallel_threads,
        disk_settings.throttle_ms
    );

    // ⚡ 진행률 throttling (100ms 또는 10파일마다) - UI 렌더링 부하 감소
    use std::cell::Cell;
    let last_progress_time = Cell::new(std::time::Instant::now());
    let last_progress_count = Cell::new(0usize);
    const PROGRESS_THROTTLE_MS: u64 = 100;
    const PROGRESS_THROTTLE_FILES: usize = 10;

    let send_progress = |phase: &str, total: usize, processed: usize, current: Option<&str>, force: bool| {
        if let Some(ref cb) = progress_callback {
            // throttle: 100ms 또는 10파일마다, 또는 force=true일 때만 전송
            let now = std::time::Instant::now();
            let elapsed = now.duration_since(last_progress_time.get()).as_millis() as u64;
            let files_since = processed.saturating_sub(last_progress_count.get());

            if force || elapsed >= PROGRESS_THROTTLE_MS || files_since >= PROGRESS_THROTTLE_FILES {
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

    // 1. 파일 스캔
    send_progress("scanning", 0, 0, None, true); // force: 시작
    let max_file_size_bytes = if max_file_size_mb > 0 { max_file_size_mb * 1_048_576 } else { 0 };
    let mut file_paths = collect_files(
        folder_path,
        SUPPORTED_EXTENSIONS,
        recursive,
        cancel_flag.as_ref(),
        max_file_size_bytes,
    );

    // skip_indexed: 이미 인덱싱된 파일 제외 (resume 용)
    if skip_indexed {
        let already_indexed = crate::db::get_fts_indexed_paths_in_folder(conn, &folder_str)
            .unwrap_or_default();
        if !already_indexed.is_empty() {
            let before = file_paths.len();
            file_paths.retain(|p| {
                let path_str = p.to_string_lossy().to_string();
                !already_indexed.contains(&path_str)
            });
            let skipped = before - file_paths.len();
            tracing::info!("[FTS Resume] Skipping {} already-indexed files", skipped);
        }
    }

    let total = file_paths.len();

    tracing::info!("[FTS] Found {} files to index in {:?}", total, folder_path);
    send_progress("scanning", total, 0, None, true); // force: 스캔 완료

    if cancel_flag.load(Ordering::Relaxed) {
        send_progress("cancelled", total, 0, None, true); // force: 취소
        return Ok(FolderIndexResult {
            folder_path: folder_str,
            indexed_count: 0,
            failed_count: 0,
            vectors_count: 0,
            errors: vec!["Cancelled by user".to_string()],
        });
    }

    // 2. 스트리밍 파이프라인 (디스크 유형 기반 병렬화)
    let (sender, receiver) = bounded::<ParseResult>(CHANNEL_BUFFER_SIZE);
    let cancel_flag_producer = cancel_flag.clone();
    let parallel_threads = disk_settings.parallel_threads;
    let throttle_ms = disk_settings.throttle_ms;

    let producer_handle = std::thread::spawn(move || {
        // 커스텀 ThreadPool (디스크 유형에 따른 스레드 수)
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(parallel_threads)
            .build()
            .unwrap_or_else(|_| {
                rayon::ThreadPoolBuilder::new()
                    .num_threads(2)
                    .build()
                    .expect("Failed to create even a minimal 2-thread pool")
            });

        pool.install(|| {
            let _ = file_paths.par_iter().try_for_each(|path| {
                if cancel_flag_producer.load(Ordering::Relaxed) {
                    return Err(());
                }

                let path_clone = path.clone();
                let result = match catch_unwind(AssertUnwindSafe(|| parse_file(&path_clone))) {
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

                // HDD throttle: I/O 부하 감소
                if throttle_ms > 0 {
                    std::thread::sleep(std::time::Duration::from_millis(throttle_ms));
                }

                sender.send(result).map_err(|_| ())
            });
        });
    });

    // 3. Consumer: FTS만 저장 (벡터 제외) - 배치 트랜잭션 적용
    let mut indexed = 0;
    let mut failed = 0;
    let mut errors: Vec<String> = Vec::new();
    let mut processed = 0;
    let mut was_cancelled = false;
    let mut batch_count = 0;

    let recv_timeout = Duration::from_millis(100);

    // 배치 트랜잭션 시작
    if let Err(e) = conn.execute_batch("BEGIN") {
        return Err(IndexError::DbError(format!("Failed to begin transaction: {}", e)));
    }

    {
        loop {
            if cancel_flag.load(Ordering::Relaxed) {
                // 취소 시 현재까지 커밋
                let _ = conn.execute_batch("COMMIT");
                send_progress("cancelled", total, processed, None, true); // force: 취소
                errors.push("Cancelled by user".to_string());
                was_cancelled = true;
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
                            send_progress("indexing", total, processed, Some(file_name), false); // throttled

                            match save_document_to_db_fts_only_no_tx(conn, &path, document) {
                                Ok(_) => indexed += 1,
                                Err(e) => {
                                    failed += 1;
                                    errors.push(format!("{:?}: {}", path, e));
                                }
                            }
                        }
                        ParseResult::Failure { path, error } => {
                            if let Err(e) = save_file_metadata_only(conn, &path) {
                                tracing::warn!("Failed to save metadata for {:?}: {}", path, e);
                            }
                            failed += 1;
                            errors.push(format!("{:?}: {}", path, error));
                            send_progress("indexing", total, processed, None, false); // throttled
                        }
                    }

                    // 배치 크기마다 커밋 후 새 트랜잭션 시작
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
    }

    // 최종 커밋
    if !was_cancelled {
        if let Err(e) = conn.execute_batch("COMMIT") {
            tracing::warn!("Final commit failed: {}", e);
        }
    }

    if !was_cancelled {
        let _ = producer_handle.join();
    }

    let phase = if was_cancelled { "cancelled" } else { "completed" };
    send_progress(phase, total, processed, None, true); // force: 완료

    Ok(FolderIndexResult {
        folder_path: folder_str,
        indexed_count: indexed,
        failed_count: failed,
        vectors_count: 0, // FTS만이므로 0
        errors,
    })
}

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
pub fn sync_folder_fts(
    conn: &Connection,
    folder_path: &Path,
    recursive: bool,
    cancel_flag: Arc<AtomicBool>,
    progress_callback: Option<FtsProgressCallback>,
    max_file_size_mb: u64,
) -> Result<SyncResult, IndexError> {
    use crate::utils::disk_info::{detect_disk_type, DiskSettings};

    let folder_str = folder_path.to_string_lossy().to_string();

    // 1. DB에서 기존 파일 메타데이터 조회
    let db_files = db::get_file_metadata_in_folder(conn, &folder_str)
        .map_err(|e| IndexError::DbError(e.to_string()))?;

    // 2. 파일시스템 스캔
    let max_file_size_bytes = if max_file_size_mb > 0 { max_file_size_mb * 1_048_576 } else { 0 };
    let fs_files = collect_files(
        folder_path,
        SUPPORTED_EXTENSIONS,
        recursive,
        cancel_flag.as_ref(),
        max_file_size_bytes,
    );

    if cancel_flag.load(Ordering::Relaxed) {
        return Ok(SyncResult {
            folder_path: folder_str,
            added: 0, modified: 0, deleted: 0, failed: 0, unchanged: 0,
            errors: vec!["Cancelled".to_string()],
        });
    }

    // 3. Diff 계산
    let mut to_index: Vec<PathBuf> = Vec::new(); // 추가 + 수정
    let mut unchanged = 0usize;

    let fs_path_set: std::collections::HashSet<String> = fs_files.iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    for path in &fs_files {
        let path_str = path.to_string_lossy().to_string();
        if let Some(&(db_modified, _db_size)) = db_files.get(&path_str) {
            // DB에 있음 → modified_at 비교
            if let Ok(meta) = fs::metadata(path) {
                let fs_modified = meta.modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);
                if fs_modified != db_modified {
                    to_index.push(path.clone()); // 수정됨
                } else {
                    unchanged += 1;
                }
            } else {
                unchanged += 1;
            }
        } else {
            to_index.push(path.clone()); // 새 파일
        }
    }

    // 삭제된 파일: DB에는 있지만 파일시스템에 없음
    let to_delete: Vec<String> = db_files.keys()
        .filter(|db_path| !fs_path_set.contains(*db_path))
        .cloned()
        .collect();

    let added_count = to_index.len();
    let delete_count = to_delete.len();

    tracing::info!(
        "[Sync] {} - to_index: {}, to_delete: {}, unchanged: {}",
        folder_str, added_count, delete_count, unchanged
    );

    // 4. 삭제 처리
    let mut deleted = 0;
    for path in &to_delete {
        if cancel_flag.load(Ordering::Relaxed) { break; }
        if let Err(e) = db::delete_file(conn, path) {
            tracing::warn!("Failed to delete stale file {}: {}", path, e);
        } else {
            deleted += 1;
        }
    }

    // 5. 인덱싱할 파일이 없으면 바로 완료 (progress 이벤트 없이 조용히)
    if to_index.is_empty() {
        return Ok(SyncResult {
            folder_path: folder_str,
            added: 0, modified: 0, deleted, failed: 0, unchanged,
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

    let send_progress = |phase: &str, total: usize, processed: usize, current: Option<&str>, force: bool| {
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
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(parallel_threads)
            .build()
            .unwrap_or_else(|_| {
                rayon::ThreadPoolBuilder::new().num_threads(2).build()
                    .expect("Failed to create thread pool")
            });

        pool.install(|| {
            let _ = to_index.par_iter().try_for_each(|path| {
                if cancel_flag_producer.load(Ordering::Relaxed) { return Err(()); }

                let path_clone = path.clone();
                let result = match catch_unwind(AssertUnwindSafe(|| parse_file(&path_clone))) {
                    Ok(Ok(doc)) => ParseResult::Success { path: path.clone(), document: doc },
                    Ok(Err(e)) => ParseResult::Failure { path: path.clone(), error: e.to_string() },
                    Err(_) => ParseResult::Failure { path: path.clone(), error: "Parser panicked".to_string() },
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
    let mut processed = 0;
    let recv_timeout = Duration::from_millis(100);

    if let Err(e) = conn.execute_batch("BEGIN") {
        return Err(IndexError::DbError(format!("Failed to begin transaction: {}", e)));
    }

    let mut batch_count = 0;
    loop {
        if cancel_flag.load(Ordering::Relaxed) {
            let _ = conn.execute_batch("COMMIT");
            break;
        }

        match receiver.recv_timeout(recv_timeout) {
            Ok(result) => {
                processed += 1;
                batch_count += 1;

                match result {
                    ParseResult::Success { path, document } => {
                        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown");
                        send_progress("indexing", total, processed, Some(file_name), false);
                        match save_document_to_db_fts_only_no_tx(conn, &path, document) {
                            Ok(_) => indexed += 1,
                            Err(e) => {
                                failed += 1;
                                errors.push(format!("{:?}: {}", path, e));
                            }
                        }
                    }
                    ParseResult::Failure { path, error } => {
                        if let Err(e) = save_file_metadata_only(conn, &path) {
                            tracing::warn!("Failed to save metadata for {:?}: {}", path, e);
                        }
                        failed += 1;
                        errors.push(format!("{:?}: {}", path, error));
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

/// 문서를 DB에 저장 - FTS만 (벡터 제외)
#[allow(dead_code)]
fn save_document_to_db_fts_only(
    conn: &Connection,
    path: &Path,
    document: ParsedDocument,
) -> Result<usize, IndexError> {
    conn.execute_batch("BEGIN")
        .map_err(|e| IndexError::DbError(e.to_string()))?;

    let result = save_document_to_db_fts_only_no_tx(conn, path, document);

    match &result {
        Ok(_) => {
            if let Err(e) = conn.execute_batch("COMMIT") {
                let _ = conn.execute_batch("ROLLBACK");
                return Err(IndexError::DbError(e.to_string()));
            }
        }
        Err(_) => {
            let _ = conn.execute_batch("ROLLBACK");
        }
    }

    result
}

/// 문서를 DB에 저장 - FTS만 (트랜잭션 없음, 배치용)
fn save_document_to_db_fts_only_no_tx(
    conn: &Connection,
    path: &Path,
    document: ParsedDocument,
) -> Result<usize, IndexError> {
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

    // upsert_file_fts_only 사용 (vector_indexed_at = NULL)
    let file_id = db::upsert_file_fts_only(conn, &path_str, &file_name, &file_type, size, modified_at)
        .map_err(|e| IndexError::DbError(e.to_string()))?;

    // _no_tx 버전 사용: 호출자(index_folder_fts_only)가 이미 트랜잭션을 관리하므로
    // 중첩 BEGIN 방지 (SQLite는 중첩 트랜잭션 미지원)
    db::delete_chunks_for_file_no_tx(conn, file_id)
        .map_err(|e| IndexError::DbError(e.to_string()))?;

    let chunks_count = document.chunks.len();

    for (idx, chunk) in document.chunks.into_iter().enumerate() {
        db::insert_chunk(
            conn,
            file_id,
            idx,
            &chunk.content,
            chunk.start_offset,
            chunk.end_offset,
            chunk.page_number,
            chunk.location_hint.as_deref(),
        )
        .map_err(|e| IndexError::DbError(e.to_string()))?;
    }

    tracing::debug!("[FTS] Indexed: {} ({} chunks)", path_str, chunks_count);

    Ok(chunks_count)
}

// ==================== Phase 2: 메타데이터 전용 스캔 ====================
// NOTE: 현재 미사용 (향후 백그라운드 파싱 통합 예정)

/// 메타데이터 스캔 진행률
#[allow(dead_code)]
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
}

/// 메타데이터 전용 스캔 (파일 열지 않음, < 2초 목표)
/// 파일명 검색 즉시 가능하게 함
pub fn scan_metadata_only(
    conn: &Connection,
    folder_path: &Path,
    recursive: bool,
    cancel_flag: Arc<AtomicBool>,
    progress_callback: Option<Box<dyn Fn(MetadataScanProgress) + Send + Sync>>,
    max_file_size_mb: u64,
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

    let max_file_size_bytes = if max_file_size_mb > 0 { max_file_size_mb * 1_048_576 } else { 0 };
    let mut count = 0;
    let mut errors: Vec<String> = Vec::new();

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

    for entry in walker.into_iter().filter_map(|e| e.ok()) {
        if cancel_flag.load(Ordering::Relaxed) {
            let _ = conn.execute_batch("COMMIT");
            send_progress("cancelled", count, true);
            return Ok(MetadataScanResult {
                folder_path: folder_str,
                files_found: count,
                errors: vec!["Cancelled".to_string()],
            });
        }

        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();

        // 확장자 체크
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if !SUPPORTED_EXTENSIONS.contains(&ext.as_str()) {
            continue;
        }

        // 임시 파일 제외
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if file_name.starts_with("~$") || file_name.starts_with('.') {
            continue;
        }

        // 파일 크기 체크 (metadata 접근 - 파일 열지 않음)
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(e) => {
                errors.push(format!("{:?}: {}", path, e));
                continue;
            }
        };

        if max_file_size_bytes > 0 && metadata.len() > max_file_size_bytes {
            continue;
        }

        // DB에 메타데이터만 저장
        let path_str = path.to_string_lossy().to_string();
        let file_type = ext.clone();
        let size = metadata.len() as i64;
        let modified_at = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        if let Err(e) = db::insert_file_metadata_only(
            conn,
            &path_str,
            file_name,
            &file_type,
            size,
            modified_at,
        ) {
            errors.push(format!("{}: {}", path_str, e));
            continue;
        }

        count += 1;
        batch_count += 1;
        send_progress("scanning", count, false);

        // 배치 커밋
        if batch_count >= BATCH_SIZE {
            if let Err(e) = conn.execute_batch("COMMIT; BEGIN") {
                tracing::warn!("Batch commit failed: {}", e);
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

    Ok(MetadataScanResult {
        folder_path: folder_str,
        files_found: count,
        errors,
    })
}

// ==================== 단일 파일 FTS 인덱싱 (manager용) ====================

/// 단일 파일 FTS 인덱싱 (벡터 제외) - 변경 감시에서 사용
pub fn index_file_fts_only(conn: &Connection, path: &Path) -> Result<IndexResult, IndexError> {
    let document = parse_file(path).map_err(|e| IndexError::ParseError(e.to_string()))?;
    let total_chars = document.content.len();

    conn.execute_batch("BEGIN")
        .map_err(|e| IndexError::DbError(e.to_string()))?;

    let chunks_count = match save_document_to_db_fts_only_no_tx(conn, path, document) {
        Ok(c) => c,
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            return Err(e);
        }
    };

    if let Err(e) = conn.execute_batch("COMMIT") {
        let _ = conn.execute_batch("ROLLBACK");
        return Err(IndexError::DbError(e.to_string()));
    }

    Ok(IndexResult {
        file_path: path.to_string_lossy().to_string(),
        chunks_count,
        vectors_count: 0,
        total_chars,
    })
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct IndexResult {
    pub file_path: String,
    pub chunks_count: usize,
    pub vectors_count: usize,
    pub total_chars: usize,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct FolderIndexResult {
    pub folder_path: String,
    pub indexed_count: usize,
    pub failed_count: usize,
    pub vectors_count: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
#[allow(dead_code, clippy::enum_variant_names)]
pub enum IndexError {
    #[error("IO error: {0}")]
    IoError(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Database error: {0}")]
    DbError(String),
    #[error("Embedding error: {0}")]
    EmbeddingError(String),
    #[error("Vector error: {0}")]
    VectorError(String),
}
