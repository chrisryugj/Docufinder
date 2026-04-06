//! 벡터 인덱스 및 검색 모듈 (usearch)
//!
//! 메모리 최적화: 기본 mmap (view) 모드로 로드하여 RAM 절감.
//! 인덱싱 시에만 in-memory (loaded) 모드로 전환, 완료 후 다시 view로 복귀.

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

/// 인덱스 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IndexMode {
    /// 비어있음 (데이터 없음)
    Empty,
    /// 메모리 맵 (mmap, read-only — 검색만 가능, RAM 최소)
    View,
    /// 전체 로드 (in-memory, read-write — 추가/삭제/검색 모두 가능)
    Loaded,
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
    /// 현재 인덱스 모드 (View: mmap / Loaded: in-memory)
    /// 락 순서: mode → index → id_map → key_map → next_key
    mode: RwLock<IndexMode>,
}

impl VectorIndex {
    /// IndexOptions 생성 (재사용을 위한 헬퍼)
    fn create_options() -> IndexOptions {
        IndexOptions {
            dimensions: EMBEDDING_DIM,
            metric: MetricKind::Cos, // 코사인 유사도
            quantization: ScalarKind::F16,
            connectivity: 16,     // HNSW M parameter
            expansion_add: 128,   // efConstruction
            expansion_search: 128, // efSearch: 64→128 대규모 인덱스 재현율 개선
            multi: false,
        }
    }

    /// 새 벡터 인덱스 생성 또는 로드
    pub fn new(path: &Path) -> Result<Self, VectorError> {
        let options = Self::create_options();

        let index =
            Index::new(&options).map_err(|e| VectorError::IndexError(format!("{:?}", e)))?;

        // 생성 직후 차원 확인 + 초기 reserve (일부 usearch 버전에서 reserve 전 add 실패 방지)
        tracing::info!(
            "usearch Index created: dims={}, capacity={}, size={}",
            index.dimensions(),
            index.capacity(),
            index.size()
        );
        index
            .reserve(100)
            .map_err(|e| VectorError::IndexError(format!("Initial reserve failed: {:?}", e)))?;
        tracing::info!(
            "usearch Index after reserve: dims={}, capacity={}, size={}",
            index.dimensions(),
            index.capacity(),
            index.size()
        );

        let mut vector_index = Self {
            path: path.to_path_buf(),
            index: RwLock::new(index),
            id_map: RwLock::new(HashMap::new()),
            key_map: RwLock::new(HashMap::new()),
            next_key: RwLock::new(0),
            mode: RwLock::new(IndexMode::Empty),
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
                path,
                usearch_size / 1024,
                map_size_bytes / 1024
            );

            // mmap (view) 모드로 로드 시도, 실패 시 full load 폴백
            match vector_index.load_index(true) {
                Ok(()) => {
                    *vector_index
                        .mode
                        .write()
                        .map_err(|_| VectorError::LockPoisoned)? = IndexMode::View;
                    tracing::info!("Vector index loaded via mmap (view mode) — RAM optimized");
                }
                Err(e) => {
                    tracing::warn!(
                        "mmap view failed, falling back to full in-memory load: {}",
                        e
                    );
                    vector_index.load_index(false)?;
                    *vector_index
                        .mode
                        .write()
                        .map_err(|_| VectorError::LockPoisoned)? = IndexMode::Loaded;
                }
            }

            // 차원 검증: load/view는 파일에서 차원을 덮어씀
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
                *vector_index
                    .index
                    .write()
                    .map_err(|_| VectorError::LockPoisoned)? = new_index;
                vector_index
                    .id_map
                    .write()
                    .map_err(|_| VectorError::LockPoisoned)?
                    .clear();
                vector_index
                    .key_map
                    .write()
                    .map_err(|_| VectorError::LockPoisoned)?
                    .clear();
                *vector_index
                    .next_key
                    .write()
                    .map_err(|_| VectorError::LockPoisoned)? = 0;
                *vector_index
                    .mode
                    .write()
                    .map_err(|_| VectorError::LockPoisoned)? = IndexMode::Empty;
            }
        } else {
            tracing::info!(
                "Creating new vector index at {:?} (usearch_exists={}, map_exists={})",
                path,
                usearch_exists,
                map_exists
            );
            // 한쪽 파일만 있으면 불일치 → 삭제
            if usearch_exists {
                let _ = std::fs::remove_file(path);
            }
            if map_exists {
                let _ = std::fs::remove_file(&map_path);
            }
        }

        // 초기화 상태 로그
        let (index_size, index_dims) = {
            let idx = vector_index
                .index
                .read()
                .map_err(|_| VectorError::LockPoisoned)?;
            (idx.size(), idx.dimensions())
        };
        let map_size = vector_index
            .id_map
            .read()
            .map_err(|_| VectorError::LockPoisoned)?
            .len();
        let current_mode = *vector_index
            .mode
            .read()
            .map_err(|_| VectorError::LockPoisoned)?;
        tracing::info!(
            "VectorIndex initialized: dims={}, index_size={}, id_map_count={}, mode={:?}",
            index_dims,
            index_size,
            map_size,
            current_mode,
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
                map_size,
                index_size
            );
            vector_index.clear();
        } else if index_size > map_size && map_size > 0 {
            tracing::warn!(
                "Vector index ({}) > mapping ({}). Keeping valid mapped data ({} orphan vectors).",
                index_size,
                map_size,
                index_size - map_size
            );
        }

        Ok(vector_index)
    }

    /// View(mmap) 모드에서 Loaded(in-memory) 모드로 전환
    /// add()/remove() 호출 전에 자동으로 호출됨
    fn ensure_writable(&self) -> Result<(), VectorError> {
        // Fast path: 이미 Loaded이면 즉시 반환
        {
            let mode = self.mode.read().map_err(|_| VectorError::LockPoisoned)?;
            if *mode == IndexMode::Loaded {
                return Ok(());
            }
            // Empty → Loaded: 인덱스는 이미 in-memory, mode만 변경
            if *mode == IndexMode::Empty {
                drop(mode);
                let mut wmode = self.mode.write().map_err(|_| VectorError::LockPoisoned)?;
                if *wmode == IndexMode::Empty {
                    *wmode = IndexMode::Loaded;
                }
                return Ok(());
            }
        }

        // Slow path: View → Loaded 전환
        let mut mode = self.mode.write().map_err(|_| VectorError::LockPoisoned)?;
        if *mode != IndexMode::View {
            return Ok(()); // 다른 스레드가 이미 전환함
        }

        let mut index_guard = self.index.write().map_err(|_| VectorError::LockPoisoned)?;
        let path_str = self.path.to_string_lossy();

        // 새 인덱스를 만들어 full load (in-memory)
        let new_index = Index::new(&Self::create_options())
            .map_err(|e| VectorError::IndexError(format!("{:?}", e)))?;
        new_index
            .load(&path_str)
            .map_err(|e| VectorError::IndexError(format!("Full load failed: {:?}", e)))?;

        let loaded_size = new_index.size();
        *index_guard = new_index;
        *mode = IndexMode::Loaded;

        tracing::info!(
            "Vector index switched to writable mode (in-memory, {} vectors)",
            loaded_size
        );

        Ok(())
    }

    /// Loaded(in-memory) 모드에서 View(mmap) 모드로 전환
    /// 인덱싱 완료 후 호출하여 RAM 회수
    pub fn switch_to_view(&self) -> Result<(), VectorError> {
        // 먼저 현재 데이터 저장
        self.save()?;

        let mut mode = self.mode.write().map_err(|_| VectorError::LockPoisoned)?;
        if *mode == IndexMode::View {
            return Ok(());
        }
        if *mode == IndexMode::Empty {
            return Ok(());
        }

        // 파일이 없으면 전환 불가
        if !self.path.exists() {
            return Ok(());
        }

        let path_str = self.path.to_string_lossy();
        let mut index_guard = self.index.write().map_err(|_| VectorError::LockPoisoned)?;

        let old_memory = index_guard.memory_usage();

        // 새 인덱스를 만들어 mmap view
        let new_index = Index::new(&Self::create_options())
            .map_err(|e| VectorError::IndexError(format!("{:?}", e)))?;

        match new_index.view(&path_str) {
            Ok(()) => {
                *index_guard = new_index;
                *mode = IndexMode::View;
                tracing::info!(
                    "Vector index switched to view mode (mmap). Freed ~{}MB",
                    old_memory / 1024 / 1024
                );
            }
            Err(e) => {
                // view 실패 시 기존 Loaded 모드 유지
                tracing::warn!("Failed to switch to view mode, keeping in-memory: {:?}", e);
            }
        }

        Ok(())
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

        // View 모드이면 writable로 전환 (ensure_writable은 mode → index 순서로 락)
        self.ensure_writable()?;

        // 모든 write lock을 한번에 획득 (TOCTOU 방지, 원자적 연산 보장)
        // 락 순서: index → id_map → key_map → next_key (데드락 방지를 위해 고정 순서)
        let index = self.index.write().map_err(|_| VectorError::LockPoisoned)?;
        let mut id_map = self.id_map.write().map_err(|_| VectorError::LockPoisoned)?;
        let mut key_map = self
            .key_map
            .write()
            .map_err(|_| VectorError::LockPoisoned)?;
        let mut next_key = self
            .next_key
            .write()
            .map_err(|_| VectorError::LockPoisoned)?;

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
        self.id_map.read().map(|map| map.len()).unwrap_or(0)
    }

    /// 벡터 삭제 (원자적 연산)
    pub fn remove(&self, chunk_id: i64) -> Result<(), VectorError> {
        // View 모드이면 writable로 전환
        self.ensure_writable()?;

        // 모든 write lock을 한번에 획득 (add와 동일 순서)
        let index = self.index.write().map_err(|_| VectorError::LockPoisoned)?;
        let mut id_map = self.id_map.write().map_err(|_| VectorError::LockPoisoned)?;
        let mut key_map = self
            .key_map
            .write()
            .map_err(|_| VectorError::LockPoisoned)?;

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
    pub fn search(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<VectorResult>, VectorError> {
        /// 최소 코사인 유사도 임계값 (이 미만은 무관한 결과로 판단)
        /// KoSimCSE-roberta에서 의미적 관련 쌍은 보통 0.5+, 0.35 미만은 노이즈
        const MIN_SIMILARITY: f32 = 0.35;

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
                    vector_results.push(VectorResult { chunk_id, score });
                }
            }
        }

        Ok(vector_results)
    }

    /// 인덱스 저장 (원자적: tmp 파일에 먼저 기록 후 rename)
    ///
    /// 크래시 안전성: 저장 중 앱이 종료되어도 기존 인덱스 파일 손상 없음
    pub fn save(&self) -> Result<(), VectorError> {
        // View 모드에서는 저장 불필요 (이미 디스크 파일 = 최신 상태)
        {
            let mode = self.mode.read().map_err(|_| VectorError::LockPoisoned)?;
            if *mode == IndexMode::View {
                return Ok(());
            }
            // Empty 모드에서 id_map이 비어있으면 저장 불필요
            // 비어있지 않은 경우는 id_map 길이 체크(아래)가 처리함
        }

        let id_map = self.id_map.read().map_err(|_| VectorError::LockPoisoned)?;
        let map_len = id_map.len();

        // 빈 인덱스는 저장하지 않음 (빈 파일이 다음 로드 시 에러 유발 가능)
        if map_len == 0 {
            return Ok(());
        }

        // Step 1: tmp 파일에 먼저 저장
        let tmp_index_path = self.path.with_extension("usearch.tmp");
        let tmp_map_path = self.path.with_extension("map.tmp");
        let final_map_path = self.path.with_extension("map");

        let tmp_index_str = tmp_index_path.to_string_lossy();
        self.index
            .read()
            .map_err(|_| VectorError::LockPoisoned)?
            .save(&tmp_index_str)
            .map_err(|e| {
                let _ = std::fs::remove_file(&tmp_index_path);
                VectorError::IndexError(format!("{:?}", e))
            })?;

        // 매핑 파일 → tmp
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
        if let Err(e) = std::fs::write(&tmp_map_path, &json_str) {
            let _ = std::fs::remove_file(&tmp_index_path);
            let _ = std::fs::remove_file(&tmp_map_path);
            return Err(e.into());
        }

        // Step 2: 원자적 rename (NTFS: 동일 볼륨 내 rename은 원자적)
        if let Err(e) = std::fs::rename(&tmp_index_path, &self.path) {
            let _ = std::fs::remove_file(&tmp_index_path);
            let _ = std::fs::remove_file(&tmp_map_path);
            return Err(VectorError::IndexError(format!(
                "Atomic rename failed for index: {}",
                e
            )));
        }
        if let Err(e) = std::fs::rename(&tmp_map_path, &final_map_path) {
            // 인덱스는 이미 rename 됨 — 매핑만 실패. 다음 로드 시 자동 복구됨.
            tracing::warn!("Map file rename failed (will recover on next load): {}", e);
            let _ = std::fs::remove_file(&tmp_map_path);
        }

        // 저장 확인 로그
        if let (Ok(usearch_meta), Ok(map_meta)) = (
            std::fs::metadata(&*self.path),
            std::fs::metadata(&final_map_path),
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

    /// 인덱스 로드 (초기화 시에만 호출)
    /// use_mmap=true: mmap view (read-only, RAM 최소)
    /// use_mmap=false: full load (in-memory, read-write)
    fn load_index(&mut self, use_mmap: bool) -> Result<(), VectorError> {
        let path_str = self.path.to_string_lossy();
        let index = self.index.write().map_err(|_| VectorError::LockPoisoned)?;

        if use_mmap {
            index
                .view(&path_str)
                .map_err(|e| VectorError::IndexError(format!("mmap view failed: {:?}", e)))?;
            tracing::debug!(
                "Loaded vector index via mmap (view): {} vectors",
                index.size()
            );
        } else {
            index
                .load(&path_str)
                .map_err(|e| VectorError::IndexError(format!("{:?}", e)))?;
            tracing::debug!("Loaded vector index file: {} vectors", index.size());
        }

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
                let mut key_map = self
                    .key_map
                    .write()
                    .map_err(|_| VectorError::LockPoisoned)?;

                for pair in pairs {
                    if let (Some(chunk_id), Some(key)) = (
                        pair.get(0).and_then(|v| v.as_i64()),
                        pair.get(1).and_then(|v| v.as_u64()),
                    ) {
                        id_map.insert(chunk_id, key);
                        key_map.insert(key, chunk_id);
                    }
                }

                // HashMap 용량 최적화: 기본 2배 확장 슬랙 제거
                id_map.shrink_to_fit();
                key_map.shrink_to_fit();

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

    /// 인덱스 초기화 (모든 데이터 삭제 — 메모리 + 디스크)
    /// Note: best-effort — lock 실패 시 조용히 무시
    /// 락 순서 준수: mode → index → id_map → key_map → next_key
    pub fn clear(&self) {
        // 1. mode (문서화된 순서 첫 번째)
        if let Ok(mut mode) = self.mode.write() {
            *mode = IndexMode::Empty;

            // 2. index (mode 락 보유 상태에서 — 순서 보장)
            if let Ok(new_index) = Index::new(&Self::create_options()) {
                if let Ok(mut index) = self.index.write() {
                    *index = new_index;
                }
            }

            // 3. id_map
            if let Ok(mut id_map) = self.id_map.write() {
                id_map.clear();
            }
            // 4. key_map
            if let Ok(mut key_map) = self.key_map.write() {
                key_map.clear();
            }
            // 5. next_key
            if let Ok(mut next_key) = self.next_key.write() {
                *next_key = 0;
            }

            // mode 락은 여기서 drop — 모든 하위 락이 이미 해제된 상태
            drop(mode);
        }

        // 디스크 파일 삭제 (재시작 시 오래된 데이터 복원 방지)
        let _ = std::fs::remove_file(&self.path); // .usearch 파일
        let map_path = self.path.with_extension("map");
        let _ = std::fs::remove_file(&map_path); // .map 파일

        tracing::info!("Vector index cleared (memory + disk)");
    }
}

// RwLock<Index>가 Send + Sync를 자동으로 구현하므로
// VectorIndex도 자동으로 Send + Sync를 구현함
// (unsafe impl 불필요)
