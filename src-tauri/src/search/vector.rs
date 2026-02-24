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

    #[error("Lock poisoned - another thread panicked while holding the lock")]
    LockPoisoned,
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
            quantization: ScalarKind::F16,
            connectivity: 16,       // HNSW M parameter
            expansion_add: 128,     // efConstruction
            expansion_search: 64,   // efSearch
            multi: false,
        };

        let index =
            Index::new(&options).map_err(|e| VectorError::IndexError(format!("{:?}", e)))?;

        // 생성 직후 차원 확인 + 초기 reserve (일부 usearch 버전에서 reserve 전 add 실패 방지)
        tracing::info!(
            "usearch Index created: dims={}, capacity={}, size={}",
            index.dimensions(), index.capacity(), index.size()
        );
        index
            .reserve(100)
            .map_err(|e| VectorError::IndexError(format!("Initial reserve failed: {:?}", e)))?;
        tracing::info!(
            "usearch Index after reserve: dims={}, capacity={}, size={}",
            index.dimensions(), index.capacity(), index.size()
        );

        let mut vector_index = Self {
            path: path.to_path_buf(),
            index: RwLock::new(index),
            id_map: RwLock::new(HashMap::new()),
            key_map: RwLock::new(HashMap::new()),
            next_key: RwLock::new(0),
        };

        // 기존 인덱스 로드 시도
        let map_path = path.with_extension("map");
        let usearch_exists = path.exists();
        let map_exists = map_path.exists();

        if usearch_exists && map_exists {
            let usearch_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
            let map_size_bytes = std::fs::metadata(&map_path).map(|m| m.len()).unwrap_or(0);
            tracing::info!(
                "Loading existing vector index from {:?} (usearch={}KB, map={}KB)",
                path, usearch_size / 1024, map_size_bytes / 1024
            );
            vector_index.load()?;

            // 차원 검증: load()는 파일에서 차원을 덮어씀
            // 모델 변경 등으로 차원이 다르면 전체 리빌드 필요
            let loaded_dims = vector_index
                .index
                .read()
                .map_err(|_| VectorError::LockPoisoned)?
                .dimensions();
            if loaded_dims != EMBEDDING_DIM {
                tracing::warn!(
                    "Loaded index has wrong dimensions ({} vs expected {}). Deleting stale files and recreating.",
                    loaded_dims, EMBEDDING_DIM
                );
                // 잘못된 파일 삭제
                let _ = std::fs::remove_file(path);
                let _ = std::fs::remove_file(&map_path);
                // 올바른 차원으로 새 인덱스 생성
                let new_index = Index::new(&options)
                    .map_err(|e| VectorError::IndexError(format!("{:?}", e)))?;
                *vector_index.index.write().map_err(|_| VectorError::LockPoisoned)? = new_index;
                vector_index.id_map.write().map_err(|_| VectorError::LockPoisoned)?.clear();
                vector_index.key_map.write().map_err(|_| VectorError::LockPoisoned)?.clear();
                *vector_index.next_key.write().map_err(|_| VectorError::LockPoisoned)? = 0;
            }
        } else {
            tracing::info!(
                "Creating new vector index at {:?} (usearch_exists={}, map_exists={})",
                path, usearch_exists, map_exists
            );
            // 한쪽 파일만 있으면 불일치 → 삭제
            if usearch_exists { let _ = std::fs::remove_file(path); }
            if map_exists { let _ = std::fs::remove_file(&map_path); }
        }

        // 초기화 상태 로그
        let (index_size, index_dims) = {
            let idx = vector_index.index.read().map_err(|_| VectorError::LockPoisoned)?;
            (idx.size(), idx.dimensions())
        };
        let map_size = vector_index
            .id_map
            .read()
            .map_err(|_| VectorError::LockPoisoned)?
            .len();
        tracing::info!(
            "VectorIndex initialized: dims={}, index_size={}, id_map_count={}",
            index_dims, index_size, map_size
        );

        // 인덱스/매핑 불일치 검증
        if index_size > 0 && map_size == 0 {
            tracing::warn!(
                "Vector index has {} vectors but mapping is empty. Resetting index.",
                index_size
            );
            vector_index.clear();
        } else if map_size > index_size {
            tracing::warn!(
                "Mapping ({}) > index ({}). Resetting to avoid stale references.",
                map_size, index_size
            );
            vector_index.clear();
        } else if index_size > map_size && map_size > 0 {
            tracing::warn!(
                "Vector index ({}) > mapping ({}). Keeping valid mapped data ({} orphan vectors).",
                index_size, map_size, index_size - map_size
            );
        }

        Ok(vector_index)
    }

    /// 벡터 추가 (원자적 연산: 모든 write lock을 한번에 획득)
    pub fn add(&self, chunk_id: i64, embedding: &[f32]) -> Result<(), VectorError> {
        if embedding.len() != EMBEDDING_DIM {
            return Err(VectorError::IndexError(format!(
                "Invalid embedding dimension: {} (expected {})",
                embedding.len(),
                EMBEDDING_DIM
            )));
        }

        // 모든 write lock을 한번에 획득 (TOCTOU 방지, 원자적 연산 보장)
        // 락 순서: index → id_map → key_map → next_key (데드락 방지를 위해 고정 순서)
        let index = self.index.write().map_err(|_| VectorError::LockPoisoned)?;
        let mut id_map = self.id_map.write().map_err(|_| VectorError::LockPoisoned)?;
        let mut key_map = self.key_map.write().map_err(|_| VectorError::LockPoisoned)?;
        let mut next_key = self.next_key.write().map_err(|_| VectorError::LockPoisoned)?;

        // 이미 존재하면 먼저 삭제 (인라인 처리 - self.remove() 호출 시 데드락)
        if let Some(&old_key) = id_map.get(&chunk_id) {
            let _ = index.remove(old_key); // best-effort
            id_map.remove(&chunk_id);
            key_map.remove(&old_key);
        }

        // 새 key 할당
        let key = *next_key;
        *next_key += 1;

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
        index.add(key, embedding).map_err(|e| {
            // 차원 불일치 진단 (첫 실패 시에만 상세 로그)
            if id_map.is_empty() {
                tracing::error!(
                    "First add() failed! index_dims={}, embedding_len={}, index_size={}, index_capacity={}, error={:?}",
                    index.dimensions(), embedding.len(), index.size(), index.capacity(), e
                );
            }
            VectorError::IndexError(format!("{:?}", e))
        })?;

        // 매핑 저장 (이미 write lock 보유)
        id_map.insert(chunk_id, key);
        key_map.insert(key, chunk_id);

        Ok(())
    }

    /// 특정 chunk_id가 벡터 인덱스에 존재하는지 확인
    pub fn contains_chunk(&self, chunk_id: i64) -> bool {
        self.id_map
            .read()
            .map(|map| map.contains_key(&chunk_id))
            .unwrap_or(false)
    }

    /// 벡터 인덱스에 저장된 청크 수
    pub fn chunk_count(&self) -> usize {
        self.id_map
            .read()
            .map(|map| map.len())
            .unwrap_or(0)
    }

    /// 벡터 삭제 (원자적 연산)
    pub fn remove(&self, chunk_id: i64) -> Result<(), VectorError> {
        // 모든 write lock을 한번에 획득 (add와 동일 순서)
        let index = self.index.write().map_err(|_| VectorError::LockPoisoned)?;
        let mut id_map = self.id_map.write().map_err(|_| VectorError::LockPoisoned)?;
        let mut key_map = self.key_map.write().map_err(|_| VectorError::LockPoisoned)?;

        if let Some(&key) = id_map.get(&chunk_id) {
            // usearch에서 삭제 (mark as removed)
            index
                .remove(key)
                .map_err(|e| VectorError::IndexError(format!("{:?}", e)))?;

            // 매핑 삭제
            id_map.remove(&chunk_id);
            key_map.remove(&key);
        }

        Ok(())
    }

    /// 유사도 검색
    pub fn search(&self, query_embedding: &[f32], limit: usize) -> Result<Vec<VectorResult>, VectorError> {
        /// 최소 코사인 유사도 임계값 (이 미만은 무관한 결과로 판단)
        const MIN_SIMILARITY: f32 = 0.25;

        // 읽기 락으로 인덱스 검색 (병렬 검색 가능)
        let results = {
            let index = self.index.read().map_err(|_| VectorError::LockPoisoned)?;
            if index.size() == 0 {
                return Ok(vec![]);
            }
            index
                .search(query_embedding, limit)
                .map_err(|e| VectorError::IndexError(format!("{:?}", e)))?
        };

        let key_map = self.key_map.read().map_err(|_| VectorError::LockPoisoned)?;
        let mut vector_results = Vec::with_capacity(results.keys.len());

        for (key, distance) in results.keys.iter().zip(results.distances.iter()) {
            if let Some(&chunk_id) = key_map.get(key) {
                // 코사인 거리를 유사도로 변환 (1 - distance)
                let score = 1.0 - distance;
                // 최소 유사도 미만 결과 필터링 (무관한 결과 제거)
                if score >= MIN_SIMILARITY {
                    vector_results.push(VectorResult {
                        chunk_id,
                        score,
                    });
                }
            }
        }

        Ok(vector_results)
    }

    /// 인덱스 저장
    pub fn save(&self) -> Result<(), VectorError> {
        let id_map = self.id_map.read().map_err(|_| VectorError::LockPoisoned)?;
        let map_len = id_map.len();

        // 빈 인덱스는 저장하지 않음 (빈 파일이 다음 로드 시 에러 유발 가능)
        if map_len == 0 {
            return Ok(());
        }

        // 읽기 락으로 저장: save()는 인덱스 데이터를 변경하지 않으므로
        // read lock으로 충분하며, 검색(read)과 동시 진행 가능
        let path_str = self.path.to_string_lossy();
        self.index
            .read()
            .map_err(|_| VectorError::LockPoisoned)?
            .save(&path_str)
            .map_err(|e| VectorError::IndexError(format!("{:?}", e)))?;

        // 매핑 파일 저장
        let map_path = self.path.with_extension("map");
        let next_key = *self
            .next_key
            .read()
            .map_err(|_| VectorError::LockPoisoned)?;

        let map_data = serde_json::json!({
            "id_map": id_map.iter().collect::<Vec<_>>(),
            "next_key": next_key,
        });

        let json_str = serde_json::to_string(&map_data)
            .map_err(|e| VectorError::IndexError(format!("JSON serialization failed: {}", e)))?;
        std::fs::write(&map_path, json_str)?;

        // 저장 확인 로그
        if let (Ok(usearch_meta), Ok(map_meta)) = (
            std::fs::metadata(&*self.path),
            std::fs::metadata(&map_path),
        ) {
            tracing::debug!(
                "Vector index saved: {} entries, usearch={}KB, map={}KB",
                map_len,
                usearch_meta.len() / 1024,
                map_meta.len() / 1024,
            );
        }

        Ok(())
    }

    /// 인덱스 로드
    fn load(&mut self) -> Result<(), VectorError> {
        // 인덱스 파일 로드 (초기화 시에만 호출, &mut self)
        let path_str = self.path.to_string_lossy();
        let index = self.index.write().map_err(|_| VectorError::LockPoisoned)?;
        index
            .load(&path_str)
            .map_err(|e| VectorError::IndexError(format!("{:?}", e)))?;

        tracing::debug!("Loaded vector index file: {} vectors", index.size());

        // 매핑 파일 로드
        let map_path = self.path.with_extension("map");
        if map_path.exists() {
            tracing::debug!("Loading mapping file from {:?}", map_path);

            let map_content = std::fs::read_to_string(&map_path)?;
            let map_data: serde_json::Value = match serde_json::from_str(&map_content) {
                Ok(data) => data,
                Err(e) => {
                    tracing::warn!("Failed to parse mapping file, starting fresh: {}", e);
                    serde_json::Value::default()
                }
            };

            if let Some(pairs) = map_data.get("id_map").and_then(|v| v.as_array()) {
                let mut id_map = self.id_map.write().map_err(|_| VectorError::LockPoisoned)?;
                let mut key_map = self.key_map.write().map_err(|_| VectorError::LockPoisoned)?;

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
                *self
                    .next_key
                    .write()
                    .map_err(|_| VectorError::LockPoisoned)? = next;
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
        self.index.read().map(|i| i.size()).unwrap_or(0)
    }

    /// 매핑 크기 (chunk_id 매핑 수)
    pub fn id_map_size(&self) -> usize {
        self.id_map.read().map(|m| m.len()).unwrap_or(0)
    }

    /// 인덱스 용량
    pub fn capacity(&self) -> usize {
        self.index.read().map(|i| i.capacity()).unwrap_or(0)
    }

    /// 인덱스 초기화 (모든 데이터 삭제)
    /// Note: best-effort - lock 실패 시 조용히 무시
    pub fn clear(&self) {
        // 매핑 초기화
        if let Ok(mut id_map) = self.id_map.write() {
            id_map.clear();
        }
        if let Ok(mut key_map) = self.key_map.write() {
            key_map.clear();
        }
        if let Ok(mut next_key) = self.next_key.write() {
            *next_key = 0;
        }

        // 새 인덱스로 교체
        let options = IndexOptions {
            dimensions: EMBEDDING_DIM,
            metric: MetricKind::Cos,
            quantization: ScalarKind::F16,
            connectivity: 16,
            expansion_add: 128,
            expansion_search: 64,
            multi: false,
        };

        if let Ok(new_index) = Index::new(&options) {
            if let Ok(mut index) = self.index.write() {
                *index = new_index;
            }
        }

        tracing::info!("Vector index cleared");
    }
}

// RwLock<Index>가 Send + Sync를 자동으로 구현하므로
// VectorIndex도 자동으로 Send + Sync를 구현함
// (unsafe impl 불필요)
