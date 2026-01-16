//! 인덱싱 파이프라인
//!
//! 파일 파싱 → 청크 생성 → FTS5 인덱싱 → 벡터 인덱싱
//! rayon을 활용한 병렬 파싱 지원

use crate::db;
use crate::embedder::Embedder;
use crate::parsers::{parse_file, ParsedDocument};
use crate::search::vector::VectorIndex;
use rayon::prelude::*;
use rusqlite::Connection;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

/// 단일 파일 인덱싱 (FTS + 벡터)
///
/// # Arguments
/// * `conn` - DB 연결
/// * `path` - 파일 경로
/// * `embedder` - 임베더 (없으면 벡터 인덱싱 스킵)
/// * `vector_index` - 벡터 인덱스 (없으면 벡터 인덱싱 스킵)
pub fn index_file(
    conn: &Connection,
    path: &Path,
    embedder: Option<&Arc<Embedder>>,
    vector_index: Option<&Arc<VectorIndex>>,
) -> Result<IndexResult, IndexError> {
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

    // 1. 파일 파싱
    let document = parse_file(path).map_err(|e| IndexError::ParseError(e.to_string()))?;

    // 2. 파일 정보 DB 저장
    let file_id = db::upsert_file(conn, &path_str, &file_name, &file_type, size, modified_at)
        .map_err(|e| IndexError::DbError(e.to_string()))?;

    // 3. 기존 청크/벡터 삭제 (재인덱싱 시)
    let old_chunk_ids = db::get_chunk_ids_for_file(conn, file_id)
        .map_err(|e| IndexError::DbError(e.to_string()))?;

    // 벡터 인덱스에서 기존 벡터 삭제
    if let Some(vi) = vector_index {
        for chunk_id in &old_chunk_ids {
            vi.remove(*chunk_id).ok(); // 에러 무시 (없을 수 있음)
        }
    }

    // DB에서 기존 청크 삭제
    db::delete_chunks_for_file(conn, file_id).map_err(|e| IndexError::DbError(e.to_string()))?;

    // 4. 청크 저장 + FTS 인덱싱
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

    // 5. 벡터 인덱싱 (embedder와 vector_index가 모두 있는 경우)
    let vectors_indexed = if let (Some(emb), Some(vi)) = (embedder, vector_index) {
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

    tracing::info!(
        "Indexed: {} ({} chunks, {} vectors, {} chars)",
        path_str,
        document.chunks.len(),
        vectors_indexed,
        document.content.len()
    );

    Ok(IndexResult {
        file_path: path_str,
        chunks_count: document.chunks.len(),
        vectors_count: vectors_indexed,
        total_chars: document.content.len(),
    })
}

/// 폴더 내 모든 지원 파일 인덱싱 (rayon 병렬 처리)
pub fn index_folder(
    conn: &Connection,
    folder_path: &Path,
    embedder: Option<&Arc<Embedder>>,
    vector_index: Option<&Arc<VectorIndex>>,
) -> Result<FolderIndexResult, IndexError> {
    // 지원 확장자
    let supported_extensions = ["txt", "md", "hwpx", "docx", "xlsx", "xls", "pdf"];

    // 1. 파일 경로 수집 (재귀 탐색)
    let file_paths = collect_files(folder_path, &supported_extensions);

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
                match save_to_db(conn, &path, document, embedder, vector_index) {
                    Ok(vectors) => {
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
    collect_files_recursive(dir, extensions, &mut files);
    files
}

fn collect_files_recursive(dir: &Path, extensions: &[&str], files: &mut Vec<PathBuf>) {
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
                collect_files_recursive(&path, extensions, files);
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

/// 파싱된 문서를 DB에 저장 (FTS + 벡터)
fn save_to_db(
    conn: &Connection,
    path: &Path,
    document: ParsedDocument,
    embedder: Option<&Arc<Embedder>>,
    vector_index: Option<&Arc<VectorIndex>>,
) -> Result<usize, IndexError> {
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
    let vectors_indexed = if let (Some(emb), Some(vi)) = (embedder, vector_index) {
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

    tracing::info!(
        "Indexed: {} ({} chunks, {} vectors)",
        path_str,
        document.chunks.len(),
        vectors_indexed
    );

    Ok(vectors_indexed)
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
