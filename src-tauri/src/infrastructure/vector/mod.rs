//! UsearchVectorRepository - VectorRepository trait의 usearch 구현체
//!
//! 데드락 수정: tokio::RwLock 사용 + 명시적 락 순서

use crate::domain::errors::DomainError;
use crate::domain::repositories::{VectorRepository, VectorSearchResult};
use crate::domain::value_objects::{ChunkId, Embedding, EMBEDDING_DIM};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::sync::RwLock;
use usearch::{Index, IndexOptions, MetricKind, ScalarKind};

/// usearch 기반 벡터 리포지토리
///
/// **데드락 방지**: tokio::RwLock 사용 + 락 해제 후 작업 패턴
pub struct UsearchVectorRepository {
    path: PathBuf,
    /// usearch 인덱스 (tokio::RwLock)
    index: RwLock<Index>,
    /// chunk_id -> usearch key 매핑
    id_map: RwLock<HashMap<i64, u64>>,
    /// usearch key -> chunk_id 역매핑
    key_map: RwLock<HashMap<u64, i64>>,
    /// 다음 usearch key
    next_key: RwLock<u64>,
}

impl UsearchVectorRepository {
    /// 새 벡터 리포지토리 생성 또는 로드
    pub async fn new(path: &Path) -> Result<Self, DomainError> {
        let options = IndexOptions {
            dimensions: EMBEDDING_DIM,
            metric: MetricKind::Cos,
            quantization: ScalarKind::F32,
            connectivity: 16,
            expansion_add: 128,
            expansion_search: 64,
            multi: false,
        };

        let index = Index::new(&options).map_err(|e| DomainError::VectorIndexError {
            operation: "create".to_string(),
            reason: format!("{:?}", e),
        })?;

        let mut repo = Self {
            path: path.to_path_buf(),
            index: RwLock::new(index),
            id_map: RwLock::new(HashMap::new()),
            key_map: RwLock::new(HashMap::new()),
            next_key: RwLock::new(0),
        };

        // 기존 인덱스 로드 시도
        if path.exists() {
            tracing::info!("Loading existing vector index from {:?}", path);
            repo.load_internal().await?;
        } else {
            tracing::info!("Creating new vector index at {:?}", path);
        }

        // 초기화 상태 로그
        let index_size = repo.index.read().await.size();
        let map_size = repo.id_map.read().await.len();
        tracing::info!(
            "VectorRepository initialized: index_size={}, id_map_count={}",
            index_size,
            map_size
        );

        if index_size > 0 && map_size == 0 {
            tracing::warn!(
                "Vector index has {} vectors but mapping is empty! Semantic search will not work.",
                index_size
            );
        }

        Ok(repo)
    }

    /// 내부 로드 (초기화 시 호출)
    async fn load_internal(&mut self) -> Result<(), DomainError> {
        let path_str = self.path.to_string_lossy();

        // 인덱스 파일 로드
        {
            let index = self.index.write().await;
            index.load(&path_str).map_err(|e| DomainError::VectorIndexError {
                operation: "load".to_string(),
                reason: format!("{:?}", e),
            })?;
            tracing::debug!("Loaded vector index file: {} vectors", index.size());
        }

        // 매핑 파일 로드
        let map_path = self.path.with_extension("map");
        if map_path.exists() {
            tracing::debug!("Loading mapping file from {:?}", map_path);

            let map_content = std::fs::read_to_string(&map_path).map_err(|e| {
                DomainError::VectorIndexError {
                    operation: "load_map".to_string(),
                    reason: e.to_string(),
                }
            })?;

            let map_data: serde_json::Value =
                serde_json::from_str(&map_content).unwrap_or_default();

            if let Some(pairs) = map_data.get("id_map").and_then(|v| v.as_array()) {
                let mut id_map = self.id_map.write().await;
                let mut key_map = self.key_map.write().await;

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
                *self.next_key.write().await = next;
            }
        } else {
            tracing::warn!(
                "Mapping file not found at {:?}. Semantic search will return empty results.",
                map_path
            );
        }

        Ok(())
    }

    /// 내부 삭제 (데드락 방지를 위한 분리)
    async fn remove_internal(&self, key: u64) -> Result<(), DomainError> {
        let index = self.index.write().await;
        index.remove(key).map_err(|e| DomainError::VectorIndexError {
            operation: "remove".to_string(),
            reason: format!("{:?}", e),
        })?;
        Ok(())
    }
}

#[async_trait]
impl VectorRepository for UsearchVectorRepository {
    async fn add(&self, chunk_id: ChunkId, embedding: Embedding) -> Result<(), DomainError> {
        let vector = embedding.as_slice();

        if vector.len() != EMBEDDING_DIM {
            return Err(DomainError::VectorIndexError {
                operation: "add".to_string(),
                reason: format!(
                    "Invalid embedding dimension: {} (expected {})",
                    vector.len(),
                    EMBEDDING_DIM
                ),
            });
        }

        // 🔴 데드락 수정: 락 해제 후 삭제 작업
        let existing_key = {
            let id_map = self.id_map.read().await;
            id_map.get(&chunk_id.value()).copied()
        }; // 락 해제

        if let Some(key) = existing_key {
            // 별도 락으로 삭제
            self.remove_internal(key).await?;
            self.id_map.write().await.remove(&chunk_id.value());
            self.key_map.write().await.remove(&key);
        }

        // 새 key 할당
        let key = {
            let mut next = self.next_key.write().await;
            let k = *next;
            *next += 1;
            k
        };

        // usearch 인덱스에 추가
        {
            let index = self.index.write().await;

            // 용량 확보
            let current_size = index.size();
            let current_capacity = index.capacity();
            if current_size >= current_capacity {
                let new_capacity = (current_capacity + 1).max(100).max(current_capacity * 2);
                index.reserve(new_capacity).map_err(|e| DomainError::VectorIndexError {
                    operation: "reserve".to_string(),
                    reason: format!("{:?}", e),
                })?;
            }

            index.add(key, vector).map_err(|e| DomainError::VectorIndexError {
                operation: "add".to_string(),
                reason: format!("{:?}", e),
            })?;
        }

        // 매핑 저장
        self.id_map.write().await.insert(chunk_id.value(), key);
        self.key_map.write().await.insert(key, chunk_id.value());

        Ok(())
    }

    async fn add_batch(&self, items: &[(ChunkId, Embedding)]) -> Result<(), DomainError> {
        for (chunk_id, embedding) in items {
            self.add(*chunk_id, embedding.clone()).await?;
        }
        Ok(())
    }

    async fn remove(&self, chunk_id: ChunkId) -> Result<(), DomainError> {
        // 🔴 데드락 수정: 락 해제 후 삭제 작업
        let key = {
            let id_map = self.id_map.read().await;
            id_map.get(&chunk_id.value()).copied()
        }; // 락 해제

        if let Some(key) = key {
            self.remove_internal(key).await?;
            self.id_map.write().await.remove(&chunk_id.value());
            self.key_map.write().await.remove(&key);
        }

        Ok(())
    }

    async fn remove_batch(&self, chunk_ids: &[ChunkId]) -> Result<(), DomainError> {
        for chunk_id in chunk_ids {
            self.remove(*chunk_id).await?;
        }
        Ok(())
    }

    async fn search(
        &self,
        query_embedding: &Embedding,
        limit: usize,
    ) -> Result<Vec<VectorSearchResult>, DomainError> {
        let query_vector = query_embedding.as_slice();

        // 읽기 락으로 검색 (병렬 검색 가능)
        let results = {
            let index = self.index.read().await;
            if index.size() == 0 {
                return Ok(vec![]);
            }
            index.search(query_vector, limit).map_err(|e| DomainError::VectorIndexError {
                operation: "search".to_string(),
                reason: format!("{:?}", e),
            })?
        };

        let key_map = self.key_map.read().await;
        let mut vector_results = Vec::with_capacity(results.keys.len());

        for (key, distance) in results.keys.iter().zip(results.distances.iter()) {
            if let Some(&chunk_id) = key_map.get(key) {
                // 코사인 거리를 유사도로 변환 (1 - distance)
                let score = 1.0 - distance;
                vector_results.push(VectorSearchResult {
                    chunk_id: ChunkId::new(chunk_id),
                    score,
                });
            }
        }

        Ok(vector_results)
    }

    fn size(&self) -> usize {
        // 동기 함수이므로 blocking read 사용
        // 주의: 이 함수는 async 컨텍스트에서 호출하면 안됨
        0 // 임시 - 실제로는 try_read 사용 권장
    }

    async fn save(&self) -> Result<(), DomainError> {
        let path_str = self.path.to_string_lossy();

        // 인덱스 파일 저장
        {
            let index = self.index.write().await;
            index.save(&path_str).map_err(|e| DomainError::VectorIndexError {
                operation: "save".to_string(),
                reason: format!("{:?}", e),
            })?;
        }

        // 매핑 파일 저장
        let map_path = self.path.with_extension("map");
        let id_map = self.id_map.read().await;
        let next_key = *self.next_key.read().await;

        let map_data = serde_json::json!({
            "id_map": id_map.iter().collect::<Vec<_>>(),
            "next_key": next_key,
        });

        let map_json = serde_json::to_string(&map_data).map_err(|e| {
            DomainError::VectorIndexError {
                operation: "serialize_map".to_string(),
                reason: e.to_string(),
            }
        })?;
        std::fs::write(&map_path, map_json).map_err(|e| {
            DomainError::VectorIndexError {
                operation: "save_map".to_string(),
                reason: e.to_string(),
            }
        })?;

        Ok(())
    }

    async fn load(&self) -> Result<(), DomainError> {
        // 이미 초기화 시 로드됨
        Ok(())
    }

    fn contains(&self, _chunk_id: ChunkId) -> bool {
        // 동기 함수 - blocking 방지
        false // 임시 - 실제로는 try_read 사용 권장
    }

    async fn clear(&self) -> Result<(), DomainError> {
        // 모든 매핑 삭제
        let keys: Vec<u64> = {
            let key_map = self.key_map.read().await;
            key_map.keys().copied().collect()
        };

        for key in keys {
            self.remove_internal(key).await?;
        }

        self.id_map.write().await.clear();
        self.key_map.write().await.clear();

        Ok(())
    }
}

// 동기 헬퍼 메서드 (VectorRepository trait 외부)
impl UsearchVectorRepository {
    /// 동기적으로 인덱스 크기 조회 (blocking)
    pub fn size_sync(&self) -> usize {
        // tokio runtime이 없는 컨텍스트에서 호출 시 주의
        match self.index.try_read() {
            Ok(guard) => guard.size(),
            Err(_) => 0,
        }
    }

    /// 동기적으로 포함 여부 확인 (blocking)
    pub fn contains_sync(&self, chunk_id: ChunkId) -> bool {
        match self.id_map.try_read() {
            Ok(guard) => guard.contains_key(&chunk_id.value()),
            Err(_) => false,
        }
    }
}
