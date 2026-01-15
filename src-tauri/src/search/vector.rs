//! 벡터 인덱스 및 검색 모듈
//!
//! usearch를 사용한 HNSW 기반 벡터 검색

use crate::embedder::EMBEDDING_DIM;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use thiserror::Error;
use usearch::{Index, IndexOptions, MetricKind, ScalarKind};

#[derive(Error, Debug)]
pub enum VectorError {
    #[error("Index error: {0}")]
    IndexError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// 벡터 검색 결과
#[derive(Debug, Clone)]
pub struct VectorResult {
    pub chunk_id: i64,
    pub score: f32,
}

/// 벡터 인덱스 (usearch HNSW)
pub struct VectorIndex {
    index: RwLock<Index>,
    path: PathBuf,
}

impl VectorIndex {
    /// 새 벡터 인덱스 생성 또는 로드
    ///
    /// # Arguments
    /// * `path` - 인덱스 파일 경로
    pub fn new(path: &Path) -> Result<Self, VectorError> {
        let options = IndexOptions {
            dimensions: EMBEDDING_DIM,
            metric: MetricKind::Cos, // 코사인 유사도
            quantization: ScalarKind::F32,
            connectivity: 16,       // HNSW M 파라미터
            expansion_add: 128,     // 인덱싱 시 탐색 범위
            expansion_search: 64,   // 검색 시 탐색 범위
            multi: false,           // 단일 벡터 per key
        };

        let index = Index::new(&options).map_err(|e| VectorError::IndexError(e.to_string()))?;

        // 기존 인덱스 로드 시도
        if path.exists() {
            index
                .load(path.to_str().unwrap())
                .map_err(|e| VectorError::IndexError(e.to_string()))?;
            tracing::info!("Loaded vector index from {:?}", path);
        } else {
            tracing::info!("Created new vector index at {:?}", path);
        }

        // 초기 용량 예약
        index
            .reserve(10000)
            .map_err(|e| VectorError::IndexError(e.to_string()))?;

        Ok(Self {
            index: RwLock::new(index),
            path: path.to_path_buf(),
        })
    }

    /// 벡터 추가
    ///
    /// # Arguments
    /// * `chunk_id` - 청크 ID (key로 사용)
    /// * `embedding` - 384차원 임베딩 벡터
    pub fn add(&self, chunk_id: i64, embedding: &[f32]) -> Result<(), VectorError> {
        let index = self.index.write().unwrap();
        index
            .add(chunk_id as u64, embedding)
            .map_err(|e| VectorError::IndexError(e.to_string()))?;
        Ok(())
    }

    /// 벡터 삭제
    ///
    /// # Arguments
    /// * `chunk_id` - 삭제할 청크 ID
    pub fn remove(&self, chunk_id: i64) -> Result<(), VectorError> {
        let index = self.index.write().unwrap();
        index
            .remove(chunk_id as u64)
            .map_err(|e| VectorError::IndexError(e.to_string()))?;
        Ok(())
    }

    /// 유사 벡터 검색
    ///
    /// # Arguments
    /// * `query_embedding` - 쿼리 임베딩 벡터
    /// * `limit` - 최대 결과 수
    ///
    /// # Returns
    /// 유사도 점수 내림차순 정렬된 결과
    pub fn search(&self, query_embedding: &[f32], limit: usize) -> Result<Vec<VectorResult>, VectorError> {
        let index = self.index.read().unwrap();

        if index.size() == 0 {
            return Ok(vec![]);
        }

        let results = index
            .search(query_embedding, limit)
            .map_err(|e| VectorError::IndexError(e.to_string()))?;

        Ok(results
            .keys
            .iter()
            .zip(results.distances.iter())
            .map(|(&key, &distance)| VectorResult {
                chunk_id: key as i64,
                score: 1.0 - distance, // 코사인 거리 → 유사도
            })
            .collect())
    }

    /// 인덱스 저장
    pub fn save(&self) -> Result<(), VectorError> {
        let index = self.index.read().unwrap();
        index
            .save(self.path.to_str().unwrap())
            .map_err(|e| VectorError::IndexError(e.to_string()))?;
        tracing::info!("Saved vector index to {:?}", self.path);
        Ok(())
    }

    /// 인덱스 크기 (벡터 개수)
    pub fn size(&self) -> usize {
        let index = self.index.read().unwrap();
        index.size()
    }

    /// 인덱스 용량
    pub fn capacity(&self) -> usize {
        let index = self.index.read().unwrap();
        index.capacity()
    }
}

// Thread-safe
unsafe impl Send for VectorIndex {}
unsafe impl Sync for VectorIndex {}
