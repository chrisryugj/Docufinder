//! 인덱싱 파이프라인
//!
//! 파일 파싱 → 청크 생성 → FTS5 인덱싱 → 벡터 인덱싱
//! rayon을 활용한 병렬 파싱 지원

use crate::constants::SUPPORTED_EXTENSIONS;
use crate::db;
use crate::embedder::Embedder;
use crate::parsers::{parse_file, ParsedDocument};
use crate::search::vector::VectorIndex;
use rayon::prelude::*;
use rusqlite::Connection;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::UNIX_EPOCH;

/// 단일 파일 인덱싱 (FTS + 벡터)
pub fn index_file(
    conn: &Connection,
    path: &Path,
    embedder: Option<&Arc<Mutex<Embedder>>>,
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

/// 폴더 내 모든 지원 파일 인덱싱 (rayon 병렬 처리)
pub fn index_folder(
    conn: &Connection,
    folder_path: &Path,
    embedder: Option<&Arc<Mutex<Embedder>>>,
    vector_index: Option<&Arc<VectorIndex>>,
) -> Result<FolderIndexResult, IndexError> {
    // 지원 확장자
    // 1. 파일 경로 수집 (재귀 탐색)
    let file_paths = collect_files(folder_path, SUPPORTED_EXTENSIONS);

    tracing::info!(
        "Found {} files to index in {:?}",
        file_paths.len(),
        folder_path
    );

    // 2. 병렬 파싱 (rayon)
    let parse_results: Vec<ParseResult> = file_paths
        .par_iter()
        .map(|path| {
            match parse_file(path) {
                Ok(doc) => ParseResult::Success {
                    path: path.clone(),
                    document: doc,
                },
                Err(e) => ParseResult::Failure {
                    path: path.clone(),
                    error: e.to_string(),
                },
            }
        })
        .collect();

    // 3. 순차적 DB 저장 (SQLite 쓰기는 단일 스레드)
    let mut indexed = 0;
    let mut failed = 0;
    let mut vectors_total = 0;
    let mut errors: Vec<String> = Vec::new();

    for result in parse_results {
        match result {
            ParseResult::Success { path, document } => {
                match save_document_to_db(conn, &path, document, embedder, vector_index) {
                    Ok((_chunks, vectors)) => {
                        indexed += 1;
                        vectors_total += vectors;
                    }
                    Err(e) => {
                        failed += 1;
                        errors.push(format!("{:?}: {}", path, e));
                    }
                }
            }
            ParseResult::Failure { path, error } => {
                failed += 1;
                errors.push(format!("{:?}: {}", path, error));
            }
        }
    }

    // 벡터 인덱스 저장
    if let Some(vi) = vector_index {
        if let Err(e) = vi.save() {
            tracing::warn!("Failed to save vector index: {}", e);
        }
    }

    Ok(FolderIndexResult {
        folder_path: folder_path.to_string_lossy().to_string(),
        indexed_count: indexed,
        failed_count: failed,
        vectors_count: vectors_total,
        errors,
    })
}

/// 병렬 파싱 결과
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

/// 폴더 재귀 탐색으로 파일 경로 수집
fn collect_files(dir: &Path, extensions: &[&str]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut visited = std::collections::HashSet::new();

    // 시작 디렉토리를 정규화하여 visited에 추가
    if let Ok(canonical) = dir.canonicalize() {
        visited.insert(canonical);
    }

    collect_files_recursive(dir, extensions, &mut files, &mut visited);
    files
}

fn collect_files_recursive(
    dir: &Path,
    extensions: &[&str],
    files: &mut Vec<PathBuf>,
    visited: &mut std::collections::HashSet<PathBuf>,
) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!("Failed to read dir {:?}: {}", dir, e);
            return;
        }
    };

    for entry in entries.flatten() {
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
                        collect_files_recursive(&path, extensions, files, visited);
                    } else {
                        tracing::debug!("Skipping already visited dir: {:?}", path);
                    }
                } else {
                    // canonicalize 실패 시에도 시도 (접근 권한 등)
                    collect_files_recursive(&path, extensions, files, visited);
                }
            }
        } else if path.is_file() {
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

/// 파싱된 문서를 DB에 저장 (FTS + 벡터) - 공통 로직
/// 반환: (chunks_count, vectors_count)
fn save_document_to_db(
    conn: &Connection,
    path: &Path,
    document: ParsedDocument,
    embedder: Option<&Arc<Mutex<Embedder>>>,
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

    // 파일 정보 DB 저장
    let file_id = db::upsert_file(conn, &path_str, &file_name, &file_type, size, modified_at)
        .map_err(|e| IndexError::DbError(e.to_string()))?;

    // 기존 청크/벡터 삭제
    let old_chunk_ids = db::get_chunk_ids_for_file(conn, file_id)
        .map_err(|e| IndexError::DbError(e.to_string()))?;

    if let Some(vi) = vector_index {
        for chunk_id in &old_chunk_ids {
            vi.remove(*chunk_id).ok();
        }
    }

    db::delete_chunks_for_file(conn, file_id).map_err(|e| IndexError::DbError(e.to_string()))?;

    // 청크 저장 + FTS 인덱싱
    let chunks_count = document.chunks.len();
    let mut chunk_ids: Vec<i64> = Vec::new();
    let mut chunk_contents: Vec<String> = Vec::new();

    for (idx, chunk) in document.chunks.iter().enumerate() {
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
        chunk_contents.push(chunk.content.clone());
    }

    // 벡터 인덱싱
    let vectors_count = if let (Some(emb), Some(vi)) = (embedder, vector_index) {
        tracing::info!("Starting vector indexing for {} ({} chunks)", path_str, chunk_contents.len());
        match emb.lock() {
            Ok(mut emb_guard) => {
                tracing::info!("Embedder locked, calling embed_batch...");
                match emb_guard.embed_batch(&chunk_contents) {
                    Ok(embeddings) => {
                        tracing::info!("embed_batch succeeded, {} embeddings", embeddings.len());
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
            }
            Err(e) => {
                tracing::warn!("Failed to lock embedder for {}: {}", path_str, e);
                0
            }
        }
    } else {
        tracing::debug!("No embedder/vector_index available, skipping vector indexing");
        0
    };

    tracing::info!(
        "Indexed: {} ({} chunks, {} vectors)",
        path_str,
        chunks_count,
        vectors_count
    );

    Ok((chunks_count, vectors_count))
}

#[derive(Debug)]
pub struct IndexResult {
    pub file_path: String,
    pub chunks_count: usize,
    pub vectors_count: usize,
    pub total_chars: usize,
}

#[derive(Debug)]
pub struct FolderIndexResult {
    pub folder_path: String,
    pub indexed_count: usize,
    pub failed_count: usize,
    pub vectors_count: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
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
