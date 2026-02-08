//! AppContainer - Dependency Injection Container
//!
//! 모든 서비스와 인프라스트럭처를 관리하는 DI 컨테이너
//! 기존 AppState를 대체하여 클린 아키텍처 적용

use crate::application::services::{FolderService, IndexService, SearchService};
use crate::commands::settings::{self, Settings};
use crate::embedder::Embedder;
use crate::indexer::manager::{IndexContext, WatchManager};
use crate::indexer::vector_worker::VectorWorker;
use crate::reranker::Reranker;
use crate::search::filename_cache::FilenameCache;
use crate::search::vector::VectorIndex;
use crate::tokenizer::{LinderaKoTokenizer, TextTokenizer};
use crate::ApiError;
use once_cell::sync::OnceCell;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};


/// DI 컨테이너 - 앱 전역 의존성 관리
pub struct AppContainer {
    // ============================================
    // Paths
    // ============================================
    /// 데이터베이스 경로
    pub db_path: PathBuf,
    /// 벡터 인덱스 경로
    pub vector_index_path: PathBuf,
    /// 모델 디렉토리 경로
    pub models_dir: PathBuf,

    // ============================================
    // Infrastructure (Lazy Load)
    // ============================================
    embedder: OnceCell<Arc<Embedder>>,
    vector_index: OnceCell<Arc<VectorIndex>>,
    watch_manager: OnceCell<Arc<RwLock<WatchManager>>>,
    /// 벡터 워커 - Arc로 공유하여 IndexService에서 동일 인스턴스 사용
    vector_worker: Arc<RwLock<VectorWorker>>,
    tokenizer: OnceCell<Arc<dyn TextTokenizer>>,
    reranker: OnceCell<Arc<Reranker>>,
    /// 파일명 캐시 (Everything 스타일 빠른 검색)
    filename_cache: Arc<FilenameCache>,

    // ============================================
    // Shared State
    // ============================================
    indexing_cancel_flag: Arc<AtomicBool>,
    /// 인메모리 설정 캐시 (디스크 I/O 제거)
    settings_cache: RwLock<Settings>,
}

impl AppContainer {
    /// 새 AppContainer 생성
    pub fn new(app_data_dir: &Path) -> Self {
        let db_path = app_data_dir.join("docufinder.db");
        let vector_index_path = app_data_dir.join("vectors.usearch");
        let models_dir = app_data_dir.join("models");

        // 디스크에서 설정 로드 (1회만, 이후 캐시 사용)
        let cached_settings = settings::get_settings_sync(app_data_dir);

        Self {
            db_path,
            vector_index_path,
            models_dir,
            embedder: OnceCell::new(),
            vector_index: OnceCell::new(),
            watch_manager: OnceCell::new(),
            vector_worker: Arc::new(RwLock::new(VectorWorker::new())),
            tokenizer: OnceCell::new(),
            reranker: OnceCell::new(),
            filename_cache: Arc::new(FilenameCache::new()),
            indexing_cancel_flag: Arc::new(AtomicBool::new(false)),
            settings_cache: RwLock::new(cached_settings),
        }
    }

    // ============================================
    // Service Factory Methods
    // ============================================

    /// SearchService 생성
    pub fn search_service(&self) -> SearchService {
        SearchService::new(
            self.db_path.clone(),
            self.get_embedder().ok(),
            self.get_vector_index().ok(),
            self.get_tokenizer().ok(),
            self.get_reranker().ok(),
            Some(self.filename_cache.clone()),
        )
    }

    /// IndexService 생성 - 공유된 vector_worker 사용
    pub fn index_service(&self) -> IndexService {
        IndexService::new(
            self.db_path.clone(),
            self.get_embedder().ok(),
            self.get_vector_index().ok(),
            self.vector_worker.clone(), // 공유 인스턴스
            self.indexing_cancel_flag.clone(),
        )
    }

    /// FolderService 생성
    pub fn folder_service(&self) -> FolderService {
        FolderService::new(
            self.db_path.clone(),
            self.get_watch_manager().ok(),
            self.get_vector_index().ok(),
        )
    }

    // ============================================
    // Infrastructure Access (Backward Compatible)
    // ============================================

    /// 인덱싱 취소 플래그 가져오기
    pub fn get_cancel_flag(&self) -> Arc<AtomicBool> {
        self.indexing_cancel_flag.clone()
    }

    /// 인덱싱 취소 플래그 리셋
    pub fn reset_cancel_flag(&self) {
        self.indexing_cancel_flag.store(false, Ordering::Relaxed);
    }

    /// 인덱싱 취소 요청
    pub fn cancel_indexing(&self) {
        self.indexing_cancel_flag.store(true, Ordering::Relaxed);
    }

    /// 임베더 가져오기 (lazy load)
    pub fn get_embedder(&self) -> Result<Arc<Embedder>, ApiError> {
        self.embedder
            .get_or_try_init(|| {
                let model_dir = self.models_dir.join("multilingual-e5-small");
                let model_path = model_dir.join("model.onnx");
                let tokenizer_path = model_dir.join("tokenizer.json");
                let dll_path = model_dir.join("onnxruntime.dll");

                if !model_path.exists() {
                    return Err(ApiError::ModelNotFound(format!("{:?}", model_path)));
                }

                // ONNX Runtime DLL 경로 설정
                // SAFETY: OnceCell이 1회만 실행을 보장하며, Embedder 초기화 전에 설정 필수
                if dll_path.exists() {
                    unsafe { std::env::set_var("ORT_DYLIB_PATH", &dll_path) };
                    tracing::info!("ORT_DYLIB_PATH set to {:?}", dll_path);
                }

                Embedder::new(&model_path, &tokenizer_path)
                    .map(Arc::new)
                    .map_err(|e| ApiError::EmbeddingFailed(e.to_string()))
            })
            .cloned()
    }

    /// 벡터 인덱스 가져오기 (lazy load)
    pub fn get_vector_index(&self) -> Result<Arc<VectorIndex>, ApiError> {
        if !self.is_semantic_available() {
            return Err(ApiError::SemanticSearchDisabled);
        }

        self.vector_index
            .get_or_try_init(|| {
                VectorIndex::new(&self.vector_index_path)
                    .map(Arc::new)
                    .map_err(|e| ApiError::SearchFailed(e.to_string()))
            })
            .cloned()
    }

    /// 시맨틱 검색 가능 여부 확인
    pub fn is_semantic_available(&self) -> bool {
        let model_path = self.models_dir.join("multilingual-e5-small").join("model.onnx");
        model_path.exists()
    }

    /// 파일 감시 매니저 가져오기 (lazy load) - Arc 참조 반환
    pub fn get_watch_manager(&self) -> Result<Arc<RwLock<WatchManager>>, ApiError> {
        self.watch_manager
            .get_or_try_init(|| {
                let ctx = IndexContext {
                    db_path: self.db_path.clone(),
                    embedder: self.get_embedder().ok(),
                    vector_index: self.get_vector_index().ok(),
                    filename_cache: self.filename_cache.clone(),
                };

                WatchManager::new(ctx)
                    .map(|wm| Arc::new(RwLock::new(wm)))
                    .map_err(|e| ApiError::IndexingFailed(format!("WatchManager 생성 실패: {}", e)))
            })
            .cloned()
    }

    /// 벡터 워커 가져오기 - Arc 공유
    pub fn get_vector_worker(&self) -> Arc<RwLock<VectorWorker>> {
        self.vector_worker.clone()
    }

    /// 한국어 형태소 분석기 가져오기 (lazy load)
    pub fn get_tokenizer(&self) -> Result<Arc<dyn TextTokenizer>, ApiError> {
        self.tokenizer
            .get_or_try_init(|| {
                LinderaKoTokenizer::new()
                    .map(|t| Arc::new(t) as Arc<dyn TextTokenizer>)
                    .map_err(|e| ApiError::IndexingFailed(format!("토크나이저 초기화 실패: {}", e)))
            })
            .cloned()
    }

    /// Cross-Encoder Reranker 가져오기 (lazy load)
    pub fn get_reranker(&self) -> Result<Arc<Reranker>, ApiError> {
        self.reranker
            .get_or_try_init(|| {
                let model_dir = self.models_dir.join("ms-marco-MiniLM-L6-v2");
                let model_path = model_dir.join("model.onnx");
                let tokenizer_path = model_dir.join("tokenizer.json");

                if !model_path.exists() {
                    return Err(ApiError::ModelNotFound(format!(
                        "Reranker 모델을 찾을 수 없습니다: {:?}",
                        model_path
                    )));
                }

                Reranker::new(&model_path, &tokenizer_path)
                    .map(Arc::new)
                    .map_err(|e| ApiError::IndexingFailed(format!("Reranker 초기화 실패: {}", e)))
            })
            .cloned()
    }

    /// Reranker 모델 사용 가능 여부 확인
    pub fn is_reranker_available(&self) -> bool {
        let model_path = self.models_dir.join("ms-marco-MiniLM-L6-v2").join("model.onnx");
        model_path.exists()
    }

    /// 캐시된 설정 조회 (디스크 I/O 없음)
    pub fn get_settings(&self) -> Settings {
        self.settings_cache.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// 설정 캐시 갱신 (update_settings 커맨드에서 호출)
    pub fn update_settings_cache(&self, settings: Settings) {
        if let Ok(mut cache) = self.settings_cache.write() {
            *cache = settings;
        }
    }

    /// 파일명 캐시 가져오기
    pub fn get_filename_cache(&self) -> Arc<FilenameCache> {
        self.filename_cache.clone()
    }

    /// 파일명 캐시 로드 (DB에서)
    pub fn load_filename_cache(&self) -> Result<usize, ApiError> {
        let conn = crate::db::get_connection(&self.db_path)
            .map_err(|e| ApiError::DatabaseConnection(format!("DB connection failed: {}", e)))?;

        self.filename_cache
            .load_from_db(&conn)
            .map_err(|e| ApiError::DatabaseQuery(format!("Failed to load filename cache: {}", e)))
    }
}
