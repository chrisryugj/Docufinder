//! 벡터 인덱스 및 검색 모듈 (stub)
//!
//! 시맨틱 검색 빌드 시 활성화됨

#[allow(unused_imports)]
use crate::embedder::EMBEDDING_DIM;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum VectorError {
    #[error("Vector search not available")]
    NotAvailable,

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// 벡터 검색 결과
#[derive(Debug, Clone)]
pub struct VectorResult {
    pub chunk_id: i64,
    pub score: f32,
}

/// 벡터 인덱스 (stub)
pub struct VectorIndex {
    path: PathBuf,
}

impl VectorIndex {
    pub fn new(path: &Path) -> Result<Self, VectorError> {
        Ok(Self {
            path: path.to_path_buf(),
        })
    }

    pub fn add(&self, _chunk_id: i64, _embedding: &[f32]) -> Result<(), VectorError> {
        Ok(()) // no-op
    }

    pub fn remove(&self, _chunk_id: i64) -> Result<(), VectorError> {
        Ok(()) // no-op
    }

    pub fn search(&self, _query_embedding: &[f32], _limit: usize) -> Result<Vec<VectorResult>, VectorError> {
        Ok(vec![]) // 항상 빈 결과
    }

    pub fn save(&self) -> Result<(), VectorError> {
        Ok(())
    }

    pub fn size(&self) -> usize {
        0
    }

    pub fn capacity(&self) -> usize {
        0
    }
}

unsafe impl Send for VectorIndex {}
unsafe impl Sync for VectorIndex {}
