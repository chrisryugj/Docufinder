//! 벡터 인덱스 및 검색 모듈 (usearch)

use crate::embedder::EMBEDDING_DIM;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use thiserror::Error;
use usearch::{Index, IndexOptions, MetricKind, ScalarKind};

#[derive(Error, Debug)]
pub enum VectorError {
    #[error("Vector index error: {0}")]
    IndexError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Vector not found: {0}")]
    NotFound(i64),
}

/// 벡터 검색 결과
#[derive(Debug, Clone)]
pub struct VectorResult {
    pub chunk_id: i64,
    pub score: f32,
}

/// 벡터 인덱스 (usearch 기반)
///
/// 스레드 안전성: 모든 필드가 RwLock으로 보호됨
pub struct VectorIndex {
    path: PathBuf,
    /// usearch 인덱스 (스레드 안전성을 위해 RwLock 사용)
    index: RwLock<Index>,
    /// chunk_id -> usearch key 매핑
    id_map: RwLock<HashMap<i64, u64>>,
    /// usearch key -> chunk_id 역매핑
    key_map: RwLock<HashMap<u64, i64>>,
    /// 다음 usearch key
    next_key: RwLock<u64>,
}

impl VectorIndex {
    /// 새 벡터 인덱스 생성 또는 로드
    pub fn new(path: &Path) -> Result<Self, VectorError> {
        let options = IndexOptions {
            dimensions: EMBEDDING_DIM,
            metric: MetricKind::Cos, // 코사인 유사도
            quantization: ScalarKind::F32,
            connectivity: 16,       // HNSW M parameter
            expansion_add: 128,     // efConstruction
            expansion_search: 64,   // efSearch
            multi: false,
        };

        let index =
            Index::new(&options).map_err(|e| VectorError::IndexError(format!("{:?}", e)))?;

        let mut vector_index = Self {
            path: path.to_path_buf(),
            index: RwLock::new(index),
            id_map: RwLock::new(HashMap::new()),
            key_map: RwLock::new(HashMap::new()),
            next_key: RwLock::new(0),
        };

        // 기존 인덱스 로드 시도
        if path.exists() {
            tracing::info!("Loading existing vector index from {:?}", path);
            vector_index.load()?;
        } else {
            tracing::info!("Creating new vector index at {:?}", path);
        }

        // 초기화 상태 로그
        let index_size = vector_index.index.read().unwrap().size();
        let map_size = vector_index.id_map.read().unwrap().len();
        tracing::info!(
            "VectorIndex initialized: index_size={}, id_map_count={}",
            index_size,
            map_size
        );

        // 인덱스에 데이터가 있는데 매핑이 없으면 경고
        if index_size > 0 && map_size == 0 {
            tracing::warn!(
                "Vector index has {} vectors but mapping is empty! Semantic search will not work.",
                index_size
            );
        }

        Ok(vector_index)
    }

    /// 벡터 추가
    pub fn add(&self, chunk_id: i64, embedding: &[f32]) -> Result<(), VectorError> {
        if embedding.len() != EMBEDDING_DIM {
            return Err(VectorError::IndexError(format!(
                "Invalid embedding dimension: {} (expected {})",
                embedding.len(),
                EMBEDDING_DIM
            )));
        }

        // 이미 존재하면 먼저 삭제
        if self.id_map.read().unwrap().contains_key(&chunk_id) {
            self.remove(chunk_id)?;
        }

        // 새 key 할당
        let key = {
            let mut next = self.next_key.write().unwrap();
            let k = *next;
            *next += 1;
            k
        };

        // usearch 인덱스에 추가 (쓰기 락 필요)
        {
            let index = self.index.write().unwrap();

            // 용량 확보 (필요시 확장)
            let current_size = index.size();
            let current_capacity = index.capacity();
            if current_size >= current_capacity {
                let new_capacity = (current_capacity + 1).max(100).max(current_capacity * 2);
                index
                    .reserve(new_capacity)
                    .map_err(|e| VectorError::IndexError(format!("Reserve failed: {:?}", e)))?;
            }

            // usearch에 추가
            index
                .add(key, embedding)
                .map_err(|e| VectorError::IndexError(format!("{:?}", e)))?;
        }

        // 매핑 저장
        self.id_map.write().unwrap().insert(chunk_id, key);
        self.key_map.write().unwrap().insert(key, chunk_id);

        Ok(())
    }

    /// 벡터 삭제
    pub fn remove(&self, chunk_id: i64) -> Result<(), VectorError> {
        let key = {
            let id_map = self.id_map.read().unwrap();
            id_map.get(&chunk_id).copied()
        };

        if let Some(key) = key {
            // usearch에서 삭제 (mark as removed)
            self.index
                .write()
                .unwrap()
                .remove(key)
                .map_err(|e| VectorError::IndexError(format!("{:?}", e)))?;

            // 매핑 삭제
            self.id_map.write().unwrap().remove(&chunk_id);
            self.key_map.write().unwrap().remove(&key);
        }

        Ok(())
    }

    /// 유사도 검색
    pub fn search(&self, query_embedding: &[f32], limit: usize) -> Result<Vec<VectorResult>, VectorError> {
        // 읽기 락으로 인덱스 검색 (병렬 검색 가능)
        let results = {
            let index = self.index.read().unwrap();
            if index.size() == 0 {
                return Ok(vec![]);
            }
            index
                .search(query_embedding, limit)
                .map_err(|e| VectorError::IndexError(format!("{:?}", e)))?
        };

        let key_map = self.key_map.read().unwrap();
        let mut vector_results = Vec::with_capacity(results.keys.len());

        for (key, distance) in results.keys.iter().zip(results.distances.iter()) {
            if let Some(&chunk_id) = key_map.get(key) {
                // 코사인 거리를 유사도로 변환 (1 - distance)
                let score = 1.0 - distance;
                vector_results.push(VectorResult {
                    chunk_id,
                    score,
                });
            }
        }

        Ok(vector_results)
    }

    /// 인덱스 저장
    pub fn save(&self) -> Result<(), VectorError> {
        // 인덱스 파일 저장 (쓰기 락으로 동시 수정 방지)
        let path_str = self.path.to_string_lossy();
        self.index
            .write()
            .unwrap()
            .save(&path_str)
            .map_err(|e| VectorError::IndexError(format!("{:?}", e)))?;

        // 매핑 파일 저장
        let map_path = self.path.with_extension("map");
        let id_map = self.id_map.read().unwrap();
        let next_key = *self.next_key.read().unwrap();

        let map_data = serde_json::json!({
            "id_map": id_map.iter().collect::<Vec<_>>(),
            "next_key": next_key,
        });

        std::fs::write(&map_path, serde_json::to_string(&map_data).unwrap())?;

        Ok(())
    }

    /// 인덱스 로드
    fn load(&mut self) -> Result<(), VectorError> {
        // 인덱스 파일 로드 (초기화 시에만 호출, &mut self)
        let path_str = self.path.to_string_lossy();
        let index = self.index.write().unwrap();
        index
            .load(&path_str)
            .map_err(|e| VectorError::IndexError(format!("{:?}", e)))?;

        tracing::debug!("Loaded vector index file: {} vectors", index.size());

        // 매핑 파일 로드
        let map_path = self.path.with_extension("map");
        if map_path.exists() {
            tracing::debug!("Loading mapping file from {:?}", map_path);

            let map_content = std::fs::read_to_string(&map_path)?;
            let map_data: serde_json::Value =
                serde_json::from_str(&map_content).unwrap_or_default();

            if let Some(pairs) = map_data.get("id_map").and_then(|v| v.as_array()) {
                let mut id_map = self.id_map.write().unwrap();
                let mut key_map = self.key_map.write().unwrap();

                for pair in pairs {
                    if let (Some(chunk_id), Some(key)) = (
                        pair.get(0).and_then(|v| v.as_i64()),
                        pair.get(1).and_then(|v| v.as_u64()),
                    ) {
                        id_map.insert(chunk_id, key);
                        key_map.insert(key, chunk_id);
                    }
                }

                tracing::debug!("Loaded {} chunk mappings", id_map.len());
            }

            if let Some(next) = map_data.get("next_key").and_then(|v| v.as_u64()) {
                *self.next_key.write().unwrap() = next;
            }
        } else {
            tracing::warn!(
                "Mapping file not found at {:?}. Semantic search will return empty results.",
                map_path
            );
        }

        Ok(())
    }

    /// 인덱스 크기 (usearch에 저장된 벡터 수)
    pub fn size(&self) -> usize {
        self.index.read().unwrap().size()
    }

    /// 매핑 크기 (chunk_id 매핑 수)
    pub fn id_map_size(&self) -> usize {
        self.id_map.read().unwrap().len()
    }

    /// 인덱스 용량
    pub fn capacity(&self) -> usize {
        self.index.read().unwrap().capacity()
    }

    /// 인덱스 초기화 (모든 데이터 삭제)
    pub fn clear(&self) {
        // 매핑 초기화
        self.id_map.write().unwrap().clear();
        self.key_map.write().unwrap().clear();
        *self.next_key.write().unwrap() = 0;

        // 새 인덱스로 교체
        let options = IndexOptions {
            dimensions: EMBEDDING_DIM,
            metric: MetricKind::Cos,
            quantization: ScalarKind::F32,
            connectivity: 16,
            expansion_add: 128,
            expansion_search: 64,
            multi: false,
        };

        if let Ok(new_index) = Index::new(&options) {
            *self.index.write().unwrap() = new_index;
        }

        tracing::info!("Vector index cleared");
    }
}

// RwLock<Index>가 Send + Sync를 자동으로 구현하므로
// VectorIndex도 자동으로 Send + Sync를 구현함
// (unsafe impl 불필요)
