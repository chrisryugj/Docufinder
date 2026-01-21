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
const TRANSACTION_BATCH_SIZE: usize = 50;

/// 단일 파일 인덱싱 (FTS + 벡터)
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
        collect_files_recursive(dir, extensions, &mut files, &mut visited, cancel_flag);
    } else {
        // 현재 폴더만 탐색
        collect_files_shallow(dir, extensions, &mut files, cancel_flag);
    }

    files
}

/// 현재 폴더만 탐색 (하위폴더 제외)
fn collect_files_shallow(
    dir: &Path,
    extensions: &[&str],
    files: &mut Vec<PathBuf>,
    cancel_flag: &AtomicBool,
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

        let path = entry.path();
        if path.is_file() {
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

        let path = entry.path();

        if path.is_dir() {
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
                        // 새로 추가된 경우에만 재귀 호출
                        collect_files_recursive(&path, extensions, files, visited, cancel_flag);
                    } else {
                        tracing::debug!("Skipping already visited dir: {:?}", path);
                    }
                } else {
                    // canonicalize 실패 시에도 원본 경로로 visited 체크 (무한 루프 방지)
                    if visited.insert(path.clone()) {
                        collect_files_recursive(&path, extensions, files, visited, cancel_flag);
                    } else {
                        tracing::debug!("Skipping already visited dir (no canonical): {:?}", path);
                    }
                }
            }
        } else if path.is_file() {
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

        db::delete_chunks_for_file(conn, file_id)
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
pub fn index_folder_fts_only(
    conn: &Connection,
    folder_path: &Path,
    recursive: bool,
    cancel_flag: Arc<AtomicBool>,
    progress_callback: Option<FtsProgressCallback>,
) -> Result<FolderIndexResult, IndexError> {
    let folder_str = folder_path.to_string_lossy().to_string();

    let send_progress = |phase: &str, total: usize, processed: usize, current: Option<&str>| {
        if let Some(ref cb) = progress_callback {
            cb(FtsIndexingProgress {
                phase: phase.to_string(),
                total_files: total,
                processed_files: processed,
                current_file: current.map(|s| s.to_string()),
                folder_path: folder_str.clone(),
            });
        }
    };

    // 1. 파일 스캔
    send_progress("scanning", 0, 0, None);
    let file_paths = collect_files(
        folder_path,
        SUPPORTED_EXTENSIONS,
        recursive,
        cancel_flag.as_ref(),
    );
    let total = file_paths.len();

    tracing::info!("[FTS] Found {} files in {:?}", total, folder_path);
    send_progress("scanning", total, 0, None);

    if cancel_flag.load(Ordering::Relaxed) {
        send_progress("cancelled", total, 0, None);
        return Ok(FolderIndexResult {
            folder_path: folder_str,
            indexed_count: 0,
            failed_count: 0,
            vectors_count: 0,
            errors: vec!["Cancelled by user".to_string()],
        });
    }

    // 2. 스트리밍 파이프라인
    let (sender, receiver) = bounded::<ParseResult>(CHANNEL_BUFFER_SIZE);
    let cancel_flag_producer = cancel_flag.clone();

    let producer_handle = std::thread::spawn(move || {
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

            sender.send(result).map_err(|_| ())
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
                send_progress("cancelled", total, processed, None);
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
                            send_progress("indexing", total, processed, Some(file_name));

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
                            send_progress("indexing", total, processed, None);
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
    send_progress(phase, total, processed, None);

    Ok(FolderIndexResult {
        folder_path: folder_str,
        indexed_count: indexed,
        failed_count: failed,
        vectors_count: 0, // FTS만이므로 0
        errors,
    })
}

/// 문서를 DB에 저장 - FTS만 (벡터 제외)
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

    db::delete_chunks_for_file(conn, file_id)
        .map_err(|e| IndexError::DbError(e.to_string()))?;

    let chunks_count = document.chunks.len();

    for (idx, chunk) in document.chunks.iter().enumerate() {
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
#[allow(dead_code)]
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
