//! AppContainer - Dependency Injection Container
//!
//! 모든 서비스와 인프라스트럭처를 관리하는 DI 컨테이너
//! 기존 AppState를 대체하여 클린 아키텍처 적용

use crate::application::services::{FolderService, IndexService, SearchService};
use crate::commands::settings::{self, Settings};
use crate::embedder::Embedder;
use crate::indexer::batch::BatchController;
use crate::indexer::manager::{IndexContext, WatchManager, WatchPauseHandle, WatchRuntimeSettings};
use crate::indexer::vector_worker::{VectorProgressCallback, VectorWorker};
use crate::ocr::OcrEngine;
use crate::search::filename_cache::FilenameCache;
use crate::search::vector::VectorIndex;
use crate::tokenizer::{LinderaKoTokenizer, TextTokenizer};
use crate::ApiError;
use once_cell::sync::OnceCell;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, RwLock};

type IncrementalCallback = RwLock<Option<Arc<dyn Fn(usize) + Send + Sync>>>;
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
    embedder: Arc<OnceCell<Arc<Embedder>>>,
    vector_index: Arc<OnceCell<Arc<VectorIndex>>>,
    watch_manager: OnceCell<Arc<RwLock<WatchManager>>>,
    /// 벡터 워커 - Arc로 공유하여 IndexService에서 동일 인스턴스 사용
    vector_worker: Arc<RwLock<VectorWorker>>,
    tokenizer: OnceCell<Arc<dyn TextTokenizer>>,
    /// OCR 엔진 (PaddleOCR ONNX)
    /// embedder/vector_index 와 같은 이유로 `Arc<OnceCell>` 공유 — WatchManager 생성 시점에
    /// 아직 워밍업 전이라도, 이후 준비되면 WatchManager 가 매번 `.get()` 으로 최신 상태를
    /// 읽는다. Option 으로 캡처하면 생성 순서에 따라 OCR 이 영구히 꺼진 채로 굳는다.
    ocr_engine: Arc<OnceCell<Arc<OcrEngine>>>,
    /// OCR 워밍업 진행 중 플래그 — 중복 ONNX 세션 빌드 방지(`spawn_ocr_warmup`)
    ocr_warmup_running: Arc<AtomicBool>,
    /// 파일명 캐시 (Everything 스타일 빠른 검색)
    filename_cache: Arc<FilenameCache>,
    /// 배치 인덱싱 컨트롤러 (멀티 폴더 순차 실행 상태)
    batch_controller: Arc<BatchController>,

    // ============================================
    // Shared State
    // ============================================
    indexing_cancel_flag: Arc<AtomicBool>,
    /// 인메모리 설정 캐시 (디스크 I/O 제거)
    settings_cache: Arc<RwLock<Settings>>,
    /// 증분 인덱싱 완료 시 프론트엔드 알림 콜백
    incremental_update_callback: IncrementalCallback,
    vector_progress_callback: VectorProgressState,
    /// 마지막 주기 sync 완료 시각 (epoch seconds). 0이면 아직 실행 전.
    last_sync_at: Arc<AtomicI64>,
    /// 주기 sync task 종료 신호. 앱 종료 시 true로 세트해 루프 중단.
    sync_shutdown: Arc<AtomicBool>,
}

impl AppContainer {
    /// data_root 경로 검증: 심볼릭 링크 거부, 드라이브 루트 거부, 시스템 폴더 거부
    ///
    /// 반환:
    /// - `Some(path)` — 검증 통과한 절대 경로 (canonicalized)
    /// - `None` — 거부됨 (호출자는 app_data_dir로 fallback)
    fn validate_data_root(root: &str, app_data_dir: &Path) -> Option<PathBuf> {
        let raw = PathBuf::from(root);
        if !raw.is_absolute() {
            tracing::warn!("data_root must be absolute: {:?}", raw);
            return None;
        }

        // 최초 사용 시 디렉토리가 없을 수 있으므로 생성 시도
        if !raw.exists() && std::fs::create_dir_all(&raw).is_err() {
            tracing::warn!("data_root create_dir_all failed: {:?}", raw);
            return None;
        }

        // canonicalize로 심볼릭 링크/.. 해소
        let canonical = match raw.canonicalize() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("data_root canonicalize failed {:?}: {}", raw, e);
                return None;
            }
        };

        // 심볼릭 링크/재파싱 포인트 거부 (raw와 canonical이 다르면 링크를 따라간 것)
        if let (Ok(raw_meta), Ok(canon_meta)) = (
            std::fs::symlink_metadata(&raw),
            std::fs::metadata(&canonical),
        ) {
            if raw_meta.file_type().is_symlink() {
                tracing::warn!("data_root is a symlink: {:?}", raw);
                return None;
            }
            let _ = canon_meta;
        }

        // 드라이브 루트 거부 (e.g. C:\, D:\)
        // naive `\\?\` strip은 UNC(`\\?\UNC\...`)를 깨뜨리지만, 여기서는 `C:` 형태
        // 드라이브 루트 판정에만 쓰므로 무해 (깨진 UNC 문자열은 어차피 매칭 안 됨).
        // 경로 표시/저장 용도라면 utils::network_path::simplify 를 쓸 것.
        let canon_str = canonical.to_string_lossy().to_string();
        let stripped = canon_str.strip_prefix(r"\\?\").unwrap_or(&canon_str);
        if stripped.len() <= 3 && stripped.chars().nth(1) == Some(':') {
            tracing::warn!("data_root cannot be a drive root: {:?}", canonical);
            return None;
        }

        // 시스템 폴더 거부
        let canon_lower = canon_str.to_lowercase();
        for pattern in crate::constants::BLOCKED_PATH_PATTERNS {
            if canon_lower.contains(&pattern.to_lowercase()) {
                tracing::warn!(
                    "data_root is inside blocked path {:?}: {:?}",
                    pattern,
                    canonical
                );
                return None;
            }
        }

        // app_data_dir와 동일하면 의미 없음 (기본값 사용)
        if canonical == app_data_dir {
            return None;
        }

        tracing::info!("Using custom data_root: {:?}", canonical);
        Some(canonical)
    }

    /// 새 AppContainer 생성
    /// data_root 설정이 있으면 DB/벡터를 해당 경로에 저장 (C: 부족 대응)
    pub fn new(app_data_dir: &Path) -> Self {
        // 디스크에서 설정 로드 (1회만, 이후 캐시 사용)
        let cached_settings = settings::get_settings_sync(app_data_dir);
        // 전역 수식 OCR 토글 초기화 — kordoc 사이드카 호출 시 --formula-ocr 전파용
        crate::parsers::kordoc::set_formula_ocr_enabled(cached_settings.formula_ocr_enabled);
        // 클라우드/네트워크 본문 인덱싱 스킵 토글 초기화
        crate::utils::cloud_detect::set_skip_enabled(cached_settings.skip_cloud_body_indexing);
        // 시스템 폴더 추가 허용 토글 초기화 — validate_watch_path 우회 분기에 사용
        crate::constants::set_allow_system_folders(cached_settings.allow_system_folders);

        // data_root가 설정되어 있으면 해당 경로에 DB/벡터 저장
        // 보안: 사용자 입력 경로 검증 (심볼릭 링크, 드라이브 루트, 시스템 폴더 거부)
        let data_dir = cached_settings
            .data_root
            .as_ref()
            .and_then(|root| Self::validate_data_root(root, app_data_dir))
            .unwrap_or_else(|| app_data_dir.to_path_buf());

        let db_path = data_dir.join("docufinder.db");
        let vector_index_path = data_dir.join("vectors.usearch");
        let models_dir = app_data_dir.join("models"); // 모델은 항상 AppData (번들 복사 위치)

        Self {
            db_path,
            vector_index_path,
            models_dir,
            app_data_dir: app_data_dir.to_path_buf(),
            embedder: Arc::new(OnceCell::new()),
            vector_index: Arc::new(OnceCell::new()),
            watch_manager: OnceCell::new(),
            vector_worker: Arc::new(RwLock::new(VectorWorker::new())),
            tokenizer: OnceCell::new(),
            ocr_engine: Arc::new(OnceCell::new()),
            ocr_warmup_running: Arc::new(AtomicBool::new(false)),
            filename_cache: Arc::new(FilenameCache::new()),
            batch_controller: Arc::new(BatchController::new()),
            indexing_cancel_flag: Arc::new(AtomicBool::new(false)),
            settings_cache: Arc::new(RwLock::new(cached_settings)),
            incremental_update_callback: RwLock::new(None),
            vector_progress_callback: Arc::new(RwLock::new(None)),
            last_sync_at: Arc::new(AtomicI64::new(0)),
            sync_shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    // ============================================
    // Periodic Sync (v2.5.2)
    // ============================================

    /// 주기 sync 마지막 완료 시각 (epoch seconds, 0=아직 없음)
    pub fn get_last_sync_at(&self) -> i64 {
        self.last_sync_at.load(Ordering::Acquire)
    }

    /// 주기 sync 완료 시 호출 — 현재 시각 기록
    pub fn mark_sync_done(&self, ts: i64) {
        self.last_sync_at.store(ts, Ordering::Release);
    }

    /// 주기 sync task 종료 신호 Arc 공유 (lib.rs setup에서 루프가 읽음)
    pub fn sync_shutdown_flag(&self) -> Arc<AtomicBool> {
        self.sync_shutdown.clone()
    }

    /// 앱 종료 시 호출 — 주기 sync task 루프 탈출 유도
    pub fn signal_sync_shutdown(&self) {
        self.sync_shutdown.store(true, Ordering::Release);
    }

    /// 배치/벡터 인덱싱 중인지 (주기 sync skip 판단)
    pub fn is_indexing_busy(&self) -> bool {
        if self.batch_controller.is_running() {
            return true;
        }
        if let Ok(worker) = self.vector_worker.read() {
            if worker.is_running() {
                return true;
            }
        }
        false
    }

    // ============================================
    // Service Factory Methods
    // ============================================

    /// SearchService 생성
    pub fn search_service(&self) -> SearchService {
        // 임베더: semantic_search_enabled ON일 때만 로드
        // (OFF면 모델 파일이 있어도 ONNX 모델(~106MB+) 상주 로드 방지 — 키워드 검색은 임베더 미사용)
        let embedder = if self.get_settings().semantic_search_enabled {
            self.get_embedder().ok()
        } else {
            None
        };
        SearchService::new(
            self.db_path.clone(),
            embedder,
            self.get_vector_index().ok(),
            self.get_tokenizer().ok(),
            Some(self.filename_cache.clone()),
        )
    }

    /// IndexService 생성 - 공유된 vector_worker 사용
    pub fn index_service(&self) -> IndexService {
        let settings = self.get_settings();
        // OCR 엔진: 준비된 경우에만 전달 — 여기서 blocking 초기화하지 않는다
        // (근거는 ocr_engine_if_ready 주석). 아직 워밍업 전이면 이번 인덱싱만 OCR 없이 진행.
        let ocr = if settings.ocr_enabled {
            let ready = self.ocr_engine_if_ready();
            if ready.is_none() {
                tracing::warn!("OCR 엔진이 아직 준비되지 않아 이번 인덱싱은 OCR 없이 진행합니다");
                // 준비 안 됐으면 여기서 워밍업을 걸어둔다 — 안 그러면 시작 시점에
                // OCR 이 꺼져 있던 세션은 엔진을 만드는 경로가 없어 영영 OCR 없이 돈다.
                self.spawn_ocr_warmup();
            }
            ready
        } else {
            None
        };
        // 임베더: semantic_search_enabled ON일 때만 로드
        // (OFF면 startup/periodic sync의 index_service 생성 시점에도 ONNX 상주 로드 방지)
        let embedder = if settings.semantic_search_enabled {
            self.get_embedder().ok()
        } else {
            None
        };
        IndexService::new(
            self.db_path.clone(),
            embedder,
            self.get_vector_index().ok(),
            self.vector_worker.clone(), // 공유 인스턴스
            self.indexing_cancel_flag.clone(),
            ocr,
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
        self.indexing_cancel_flag.store(false, Ordering::Release);
    }

    /// 인덱싱 취소 요청
    pub fn cancel_indexing(&self) {
        self.indexing_cancel_flag.store(true, Ordering::Release);
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

                // 8GB RAM 환경 경고: ONNX 임베딩 모델(INT8 ~106MB / F32 ~840MB) 상주
                let sys_mem = crate::utils::disk_info::total_memory_mb();
                if sys_mem > 0 && sys_mem <= 8192 {
                    tracing::warn!(
                        "시맨틱 모델 로드 중 (RAM {}MB). 8GB 환경에서는 메모리 부족이 발생할 수 있습니다. 16GB 이상 권장.",
                        sys_mem
                    );
                }

                Embedder::new(&model_path, &tokenizer_path)
                    .map(Arc::new)
                    .map_err(|e| {
                        tracing::error!("Embedder 초기화 실패: {}", e);
                        ApiError::EmbeddingFailed(e.to_string())
                    })
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
                    }
                });
                let watch_pause_handle = WatchPauseHandle::new();
                // 벡터 인덱싱은 AI RAG 전용 → incremental update 후 자동 벡터 트리거 비활성화
                let vector_trigger: Option<Arc<dyn Fn() + Send + Sync>> = None;
                // OCR 엔진: OnceCell 을 **공유** — 여기서 blocking 초기화하지 않는다.
                // WatchManager 는 폴더 목록 조회(get_folders_with_info) 등 UI 경로가 함께 쓰는
                // OnceCell 이라, 여기서 OCR 빌드를 기다리면 폴더 트리가 통째로 안 뜬다(이슈 #35).
                // 공유이므로 생성 시점에 아직 워밍업 전이어도 준비되는 즉시 반영된다.
                let ocr = self.ocr_engine.clone();
                // 벡터/임베더 OnceCell을 **공유** — WatchManager 생성 시점에
                // OnceCell이 비어있어도, 이후 검색 등으로 init되면 WatchManager도
                // 매번 .get()으로 최신 상태를 읽는다 (orphan 벡터 방지)
                let ctx = IndexContext {
                    db_path: self.db_path.clone(),
                    embedder: self.embedder.clone(),
                    vector_index: self.vector_index.clone(),
                    filename_cache: self.filename_cache.clone(),
                    runtime_settings,
                    on_incremental_update: callback,
                    on_vector_trigger: vector_trigger,
                    ocr_engine: ocr,
                };

                WatchManager::new(ctx, watch_pause_handle.shared_counter())
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

    /// OCR 엔진 (PaddleOCR ONNX)
    pub fn get_ocr_engine(&self) -> Result<Arc<OcrEngine>, ApiError> {
        self.ocr_engine
            .get_or_try_init(|| {
                let ocr_dir = self.models_dir.join("paddleocr");
                OcrEngine::new(&ocr_dir, self.get_settings().ocr_layout_enabled)
                    .map(Arc::new)
                    .map_err(|e| ApiError::IndexingFailed(format!("OCR 엔진 초기화 실패: {}", e)))
            })
            .cloned()
    }

    /// 이미 초기화된 OCR 엔진만 반환 — **절대 blocking 초기화하지 않는다**.
    ///
    /// `get_ocr_engine` 은 OnceCell 초기화라, 초기화가 오래 걸리거나 멈추면 같은 OnceCell 을
    /// 기다리는 **모든** 호출자가 함께 멈춘다. OCR 은 ONNX 세션 빌드라 환경(내부망 EDR 의
    /// DLL 로드 검사 등)에 따라 수십 초 이상 걸릴 수 있어, UI 경로가 그 지연에 얽히면 앱이
    /// 멈춘 것처럼 보인다(이슈 #35). 워밍업은 `spawn_ocr_warmup` 이 담당하고, 그 외에는
    /// 이 peek 으로 준비된 경우에만 사용한다.
    pub fn ocr_engine_if_ready(&self) -> Option<Arc<OcrEngine>> {
        self.ocr_engine.get().cloned()
    }

    /// OCR 엔진을 백그라운드에서 준비한다(멱등, 논블로킹).
    ///
    /// v3.4.5 는 UI 굶김을 막으려 blocking 초기화 호출을 전부 peek 으로 바꿨는데,
    /// 그 결과 **엔진을 실제로 만드는 경로가 시작 워밍업 하나뿐** 이 됐다. 그래서
    /// `ocr_enabled:false` 로 켠 앱에서 설정으로 OCR 을 켜면 재시작 전까지 OCR 이
    /// 영영 동작하지 않았다(이슈 #35). 초기화 실패도 마찬가지로 재시도 경로가 없었다.
    ///
    /// - 이미 준비됐거나 워밍업이 진행 중이면 no-op
    /// - 실패하면 진행 플래그를 되돌려, 다음 인덱싱·설정 변경에서 자연히 재시도된다
    /// - 컨테이너 락을 잡지 않는다(OnceCell/경로만 clone) — 수십 초 걸려도 UI 무관
    pub fn spawn_ocr_warmup(&self) {
        if self.ocr_engine.get().is_some() {
            return;
        }
        if self.ocr_warmup_running.swap(true, Ordering::AcqRel) {
            return; // 이미 워밍업 중
        }

        let cell = self.ocr_engine.clone();
        let running = self.ocr_warmup_running.clone();
        let ocr_dir = self.models_dir.join("paddleocr");
        let layout_enabled = self.get_settings().ocr_layout_enabled;

        std::thread::spawn(move || {
            let started = std::time::Instant::now();
            // ort 는 동적 라이브러리 로드 실패를 Err 가 아니라 **panic** 으로 알린다
            // (보안솔루션이 DLL 로드를 막는 환경이 정확히 이 경로). 잡지 않으면 이 스레드가
            // 죽으면서 진행 플래그가 true 로 굳어 재시도가 영영 막힌다.
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                cell.get_or_try_init(|| OcrEngine::new(&ocr_dir, layout_enabled).map(Arc::new))
                    .map(|_| ())
            }));
            running.store(false, Ordering::Release);

            match result {
                Ok(Ok(())) => {
                    tracing::info!("OCR 엔진 워밍업 완료 ({}ms)", started.elapsed().as_millis())
                }
                Ok(Err(e)) => tracing::warn!(
                    "OCR 엔진 워밍업 실패 — 스캔 문서 OCR 만 비활성됩니다 (다음 인덱싱·설정 변경 때 재시도): {}",
                    e
                ),
                Err(_) => tracing::warn!(
                    "OCR 엔진 워밍업 중단(panic) — ONNX Runtime 로드가 막힌 환경일 수 있습니다. \
                     스캔 문서 OCR 만 비활성되며 다음 인덱싱·설정 변경 때 재시도합니다"
                ),
            }
        });
    }

    /// OCR 모델 사용 가능 여부
    pub fn is_ocr_available(&self) -> bool {
        let ocr_dir = self.models_dir.join("paddleocr");
        ocr_dir.join("det.onnx").exists() && ocr_dir.join("rec.onnx").exists()
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

    /// 배치 인덱싱 컨트롤러 가져오기 (Arc 공유)
    pub fn get_batch_controller(&self) -> Arc<BatchController> {
        self.batch_controller.clone()
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn resources_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources")
    }

    fn ort_dylib() -> PathBuf {
        let lib = if cfg!(target_os = "windows") {
            "onnxruntime.dll"
        } else if cfg!(target_os = "macos") {
            "libonnxruntime.dylib"
        } else {
            "libonnxruntime.so"
        };
        resources_dir().join("onnxruntime").join(lib)
    }

    /// 실물 파일인지 (CI placeholder 는 0바이트)
    fn is_real_file(path: &Path) -> bool {
        std::fs::metadata(path)
            .map(|m| m.len() > 0)
            .unwrap_or(false)
    }

    fn wait_ready(container: &AppContainer, timeout: Duration) -> bool {
        let started = Instant::now();
        while started.elapsed() < timeout {
            if container.ocr_engine_if_ready().is_some() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        false
    }

    /// 이슈 #35 회귀: 워밍업이 실패해도 **재시도** 되어야 한다.
    ///
    /// v3.4.5 는 OCR 초기화 경로가 시작 워밍업 하나뿐이라, 그 한 번이 실패하면
    /// (모델 seed 레이스·설정이 나중에 켜짐) 재시작 전까지 OCR 이 영영 죽었다.
    #[test]
    fn ocr_warmup_failure_is_retryable() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let container = AppContainer::new(tmp.path()); // models/paddleocr 없음

        container.spawn_ocr_warmup();
        assert!(
            !wait_ready(&container, Duration::from_secs(3)),
            "모델이 없는데 엔진이 준비됐다고 보고하면 안 된다"
        );

        // 실물 리소스가 있을 때만 "다시 걸면 준비된다" 까지 검증한다.
        // CI(mac gate)는 tauri 리소스 검증용으로 이 파일들을 0바이트로 비워두므로
        // (.github/workflows/ci.yml "Create placeholders"), 크기까지 봐야 한다.
        let ocr_src = resources_dir().join("paddleocr");
        if !is_real_file(&ocr_src.join("det.onnx")) || !is_real_file(&ort_dylib()) {
            eprintln!("skip: 번들 OCR 모델/ONNX Runtime 실물 없음 — 재시도 경로만 부분 검증");
            return;
        }

        // 모델을 채우고 다시 걸면 준비돼야 한다 = 진행 플래그가 실패로 굳지 않았다.
        let ocr_dst = tmp.path().join("models").join("paddleocr");
        fs_copy_models(&ocr_src, &ocr_dst);
        unsafe { std::env::set_var("ORT_DYLIB_PATH", ort_dylib()) };

        container.spawn_ocr_warmup();
        assert!(
            wait_ready(&container, Duration::from_secs(120)),
            "실패 후 재호출한 워밍업이 엔진을 만들지 못했다 — 진행 플래그가 굳었다"
        );
    }

    /// 이슈 #35 회귀: 시작 시 OCR 이 꺼져 있던 세션도 인덱싱 경로에서 워밍업이 걸려야 한다.
    /// (index_service 는 peek 만 하므로, 워밍업을 스스로 걸지 않으면 OCR 없이 영구 동작)
    #[test]
    fn index_service_triggers_warmup_when_ocr_not_ready() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let container = AppContainer::new(tmp.path());
        {
            let mut s = container.settings_cache.write().unwrap();
            s.ocr_enabled = true;
        }
        assert!(container.ocr_engine_if_ready().is_none());

        let _ = container.index_service();
        assert!(
            container.ocr_warmup_running.load(Ordering::Acquire)
                || container.ocr_engine_if_ready().is_some(),
            "OCR 이 준비 안 된 상태의 index_service 가 워밍업을 걸지 않았다"
        );
    }

    fn fs_copy_models(src: &Path, dst: &Path) {
        std::fs::create_dir_all(dst).unwrap();
        for name in ["det.onnx", "rec.onnx", "dict.txt"] {
            std::fs::copy(src.join(name), dst.join(name)).unwrap();
        }
    }
}
