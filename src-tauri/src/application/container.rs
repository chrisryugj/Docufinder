//! AppContainer - Dependency Injection Container
//!
//! 모든 서비스와 인프라스트럭처를 관리하는 DI 컨테이너
//! 기존 AppState를 대체하여 클린 아키텍처 적용

use crate::application::services::{FolderService, IndexService, SearchService};
use crate::commands::settings::{self, Settings, VectorIndexingMode};
use crate::embedder::Embedder;
use crate::indexer::manager::{IndexContext, WatchManager, WatchRuntimeSettings};
use crate::indexer::vector_worker::{VectorProgressCallback, VectorWorker};
use crate::reranker::Reranker;
use crate::search::filename_cache::FilenameCache;
use crate::search::vector::VectorIndex;
use crate::tokenizer::{LinderaKoTokenizer, TextTokenizer};
use crate::ApiError;
use once_cell::sync::OnceCell;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

type IncrementalCallback = RwLock<Option<Arc<dyn Fn(usize) + Send + Sync>>>;
type HwpDetectedCallback = RwLock<Option<Arc<dyn Fn(Vec<String>) + Send + Sync>>>;
type VectorProgressState = Arc<RwLock<Option<VectorProgressCallback>>>;

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
    /// 설정 파일 경로 (항상 AppData 고정, data_root와 무관)
    pub app_data_dir: PathBuf,

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
    settings_cache: Arc<RwLock<Settings>>,
    /// 증분 인덱싱 완료 시 프론트엔드 알림 콜백
    incremental_update_callback: IncrementalCallback,
    /// HWP 파일 감지 시 프론트엔드 알림 콜백
    hwp_detected_callback: HwpDetectedCallback,
    vector_progress_callback: VectorProgressState,
}

impl AppContainer {
    /// 새 AppContainer 생성
    /// data_root 설정이 있으면 DB/벡터를 해당 경로에 저장 (C: 부족 대응)
    pub fn new(app_data_dir: &Path) -> Self {
        // 디스크에서 설정 로드 (1회만, 이후 캐시 사용)
        let cached_settings = settings::get_settings_sync(app_data_dir);

        // data_root가 설정되어 있으면 해당 경로에 DB/벡터 저장
        let data_dir = if let Some(ref root) = cached_settings.data_root {
            let p = PathBuf::from(root);
            if p.exists() || std::fs::create_dir_all(&p).is_ok() {
                tracing::info!("Using custom data_root: {:?}", p);
                p
            } else {
                tracing::warn!(
                    "data_root {:?} is not accessible, falling back to app_data_dir",
                    p
                );
                app_data_dir.to_path_buf()
            }
        } else {
            app_data_dir.to_path_buf()
        };

        let db_path = data_dir.join("docufinder.db");
        let vector_index_path = data_dir.join("vectors.usearch");
        let models_dir = app_data_dir.join("models"); // 모델은 항상 AppData (번들 복사 위치)

        Self {
            db_path,
            vector_index_path,
            models_dir,
            app_data_dir: app_data_dir.to_path_buf(),
            embedder: OnceCell::new(),
            vector_index: OnceCell::new(),
            watch_manager: OnceCell::new(),
            vector_worker: Arc::new(RwLock::new(VectorWorker::new())),
            tokenizer: OnceCell::new(),
            reranker: OnceCell::new(),
            filename_cache: Arc::new(FilenameCache::new()),
            indexing_cancel_flag: Arc::new(AtomicBool::new(false)),
            settings_cache: Arc::new(RwLock::new(cached_settings)),
            incremental_update_callback: RwLock::new(None),
            hwp_detected_callback: RwLock::new(None),
            vector_progress_callback: Arc::new(RwLock::new(None)),
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
                let model_dir = self.models_dir.join("kosimcse-roberta-multitask");
                // INT8 양자화 모델 우선, F32 원본 폴백
                let int8_path = model_dir.join("model_int8.onnx");
                let model_path = if int8_path.exists() {
                    tracing::info!("INT8 양자화 모델 사용 (model_int8.onnx)");
                    int8_path
                } else {
                    tracing::info!("F32 원본 모델 사용 (model.onnx)");
                    model_dir.join("model.onnx")
                };
                let tokenizer_path = model_dir.join("tokenizer.json");

                if !model_path.exists() {
                    return Err(ApiError::ModelNotFound(format!("{:?}", model_path)));
                }

                // ORT_DYLIB_PATH는 lib.rs setup()에서 단일 스레드 시점에 설정됨
                // (멀티스레드 환경에서 unsafe set_var 호출 방지)

                // 8GB RAM 환경 경고: ONNX 모델(INT8 ~106MB / F32 ~840MB) + Reranker ~24MB 상주
                let sys_mem = sysinfo_total_memory_mb();
                if sys_mem > 0 && sys_mem <= 8192 {
                    tracing::warn!(
                        "시맨틱 모델 로드 중 (RAM {}MB). 8GB 환경에서는 메모리 부족이 발생할 수 있습니다. 16GB 이상 권장.",
                        sys_mem
                    );
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

    /// 시맨틱 검색 가능 여부 확인 (INT8 또는 F32 모델 존재 시 true)
    pub fn is_semantic_available(&self) -> bool {
        let model_dir = self.models_dir.join("kosimcse-roberta-multitask");
        model_dir.join("model_int8.onnx").exists() || model_dir.join("model.onnx").exists()
    }

    /// 증분 인덱싱 완료 시 호출할 콜백 설정 (WatchManager 초기화 전에 호출해야 함)
    pub fn set_incremental_update_callback(&self, callback: Arc<dyn Fn(usize) + Send + Sync>) {
        if let Ok(mut cb) = self.incremental_update_callback.write() {
            *cb = Some(callback);
        }
    }

    pub fn set_hwp_detected_callback(&self, callback: Arc<dyn Fn(Vec<String>) + Send + Sync>) {
        if let Ok(mut cb) = self.hwp_detected_callback.write() {
            *cb = Some(callback);
        }
    }

    pub fn set_vector_progress_callback(&self, callback: VectorProgressCallback) {
        if let Ok(mut cb) = self.vector_progress_callback.write() {
            *cb = Some(callback);
        }
    }

    /// 파일 감시 매니저 가져오기 (lazy load) - Arc 참조 반환
    pub fn get_watch_manager(&self) -> Result<Arc<RwLock<WatchManager>>, ApiError> {
        self.watch_manager
            .get_or_try_init(|| {
                let callback = self
                    .incremental_update_callback
                    .read()
                    .ok()
                    .and_then(|cb| cb.clone());
                let settings_cache = self.settings_cache.clone();
                let runtime_settings = Arc::new(move || {
                    let settings = settings_cache
                        .read()
                        .unwrap_or_else(|e| e.into_inner())
                        .clone();
                    let mut excluded_dirs: Vec<String> = crate::constants::DEFAULT_EXCLUDED_DIRS
                        .iter()
                        .map(|s| s.to_string())
                        .collect();
                    excluded_dirs.extend(settings.exclude_dirs.clone());
                    WatchRuntimeSettings {
                        max_file_size_mb: settings.max_file_size_mb,
                        excluded_dirs,
                        hwp_auto_detect: settings.hwp_auto_detect,
                    }
                });
                // 벡터 워커 트리거 콜백: watcher 증분 인덱싱 후 pending 벡터 자동 백필
                let vector_trigger: Option<Arc<dyn Fn() + Send + Sync>> = {
                    let vw = self.vector_worker.clone();
                    let db = self.db_path.clone();
                    let settings_cache = self.settings_cache.clone();
                    let emb = self.get_embedder().ok();
                    let vi = self.get_vector_index().ok();
                    match (emb, vi) {
                        (Some(embedder), Some(vector_index)) => Some(Arc::new(move || {
                            let settings = settings_cache
                                .read()
                                .unwrap_or_else(|e| e.into_inner())
                                .clone();
                            if !settings.semantic_search_enabled
                                || settings.vector_indexing_mode != VectorIndexingMode::Auto
                            {
                                return;
                            }

                            // 단일 write lock 스코프 (TOCTOU 방지)
                            if let Ok(mut worker) = vw.write() {
                                if !worker.is_running() {
                                    let _ = worker.start(
                                        db.clone(),
                                        embedder.clone(),
                                        vector_index.clone(),
                                        None,
                                        Some(settings.indexing_intensity.clone()),
                                    );
                                    tracing::debug!("[WatchManager] Vector worker triggered after incremental update");
                                }
                            }
                        })),
                        _ => None,
                    }
                };
                let hwp_callback = self
                    .hwp_detected_callback
                    .read()
                    .ok()
                    .and_then(|cb| cb.clone());
                let ctx = IndexContext {
                    db_path: self.db_path.clone(),
                    embedder: self.get_embedder().ok(),
                    vector_index: self.get_vector_index().ok(),
                    filename_cache: self.filename_cache.clone(),
                    runtime_settings,
                    on_incremental_update: callback,
                    on_vector_trigger: vector_trigger,
                    on_hwp_detected: hwp_callback,
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
        let model_path = self
            .models_dir
            .join("ms-marco-MiniLM-L6-v2")
            .join("model.onnx");
        model_path.exists()
    }

    /// 캐시된 설정 조회 (디스크 I/O 없음)
    pub fn get_settings(&self) -> Settings {
        self.settings_cache
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
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

/// 시스템 총 메모리 조회 (MB 단위, 실패 시 0)
#[cfg(windows)]
fn sysinfo_total_memory_mb() -> u64 {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    unsafe {
        let mut mem = std::mem::zeroed::<MEMORYSTATUSEX>();
        mem.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
        if GlobalMemoryStatusEx(&mut mem) != 0 {
            mem.ullTotalPhys / 1_048_576
        } else {
            0
        }
    }
}

#[cfg(not(windows))]
fn sysinfo_total_memory_mb() -> u64 {
    0
}
