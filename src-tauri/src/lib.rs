mod commands;
mod constants;
mod db;
mod embedder;
mod indexer;
mod parsers;
mod search;

use embedder::Embedder;
use indexer::manager::{IndexContext, WatchManager};
use search::vector::VectorIndex;
use std::path::PathBuf;
use once_cell::sync::OnceCell;
use std::sync::{Arc, Mutex, RwLock};

type SharedEmbedder = Arc<Mutex<Embedder>>;
use tauri::Manager;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};
use tracing_appender::rolling::{RollingFileAppender, Rotation};

/// 앱 전역 상태
pub struct AppState {
    /// 데이터베이스 경로
    pub db_path: PathBuf,
    /// 벡터 인덱스 경로
    pub vector_index_path: PathBuf,
    /// 모델 디렉토리 경로
    pub models_dir: PathBuf,
    /// 임베더 (lazy load)
    embedder: OnceCell<SharedEmbedder>,
    /// 벡터 인덱스 (lazy load)
    vector_index: OnceCell<Arc<VectorIndex>>,
    /// 파일 감시 매니저 (lazy load)
    watch_manager: OnceCell<RwLock<WatchManager>>,
}

impl AppState {
    /// 새 AppState 생성
    pub fn new(app_data_dir: &PathBuf) -> Self {
        let db_path = app_data_dir.join("docufinder.db");
        let vector_index_path = app_data_dir.join("vectors.usearch");
        let models_dir = app_data_dir.join("models");

        Self {
            db_path,
            vector_index_path,
            models_dir,
            embedder: OnceCell::new(),
            vector_index: OnceCell::new(),
            watch_manager: OnceCell::new(),
        }
    }

    /// 임베더 가져오기 (필요시 로드)
    pub fn get_embedder(&self) -> Result<SharedEmbedder, String> {
        self.embedder
            .get_or_try_init(|| {
                let model_dir = self.models_dir.join("multilingual-e5-small");
                let model_path = model_dir.join("model.onnx");
                let tokenizer_path = model_dir.join("tokenizer.json");
                let dll_path = model_dir.join("onnxruntime.dll");

                if !model_path.exists() {
                    return Err(format!(
                        "Model not found at {:?}. Please download the model first.",
                        model_path
                    ));
                }

                // ONNX Runtime DLL 경로 설정 (load-dynamic 모드)
                if dll_path.exists() {
                    std::env::set_var("ORT_DYLIB_PATH", &dll_path);
                    tracing::info!("ORT_DYLIB_PATH set to {:?}", dll_path);
                } else {
                    tracing::warn!("onnxruntime.dll not found at {:?}", dll_path);
                }

                Embedder::new(&model_path, &tokenizer_path)
                    .map(|e| Arc::new(Mutex::new(e)))
                    .map_err(|e| e.to_string())
            })
            .cloned()
    }

    /// 벡터 인덱스 가져오기 (필요시 생성/로드)
    pub fn get_vector_index(&self) -> Result<Arc<VectorIndex>, String> {
        self.vector_index
            .get_or_try_init(|| {
                VectorIndex::new(&self.vector_index_path)
                    .map(Arc::new)
                    .map_err(|e| e.to_string())
            })
            .cloned()
    }

    /// 시맨틱 검색 가능 여부 확인
    pub fn is_semantic_available(&self) -> bool {
        let model_path = self.models_dir.join("multilingual-e5-small").join("model.onnx");
        model_path.exists()
    }

    /// 파일 감시 매니저 가져오기 (필요시 생성)
    pub fn get_watch_manager(&self) -> Result<&RwLock<WatchManager>, String> {
        self.watch_manager
            .get_or_try_init(|| {
                // 시맨틱 검색 활성화 (ONNX Runtime 1.20.1)
                let ctx = IndexContext {
                    db_path: self.db_path.clone(),
                    embedder: self.get_embedder().ok(),
                    vector_index: self.get_vector_index().ok(),
                };

                WatchManager::new(ctx)
                    .map(RwLock::new)
                    .map_err(|e| format!("Failed to create WatchManager: {}", e))
            })
    }
}

/// 로깅 초기화 (파일 + 콘솔)
fn init_logging(app_data_dir: Option<&PathBuf>) {
    // 기본 필터: 릴리즈에서는 info, 디버그에서는 debug
    let default_filter = if cfg!(debug_assertions) {
        "docufinder=debug,tauri=info"
    } else {
        "docufinder=info,tauri=warn"
    };

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(default_filter));

    // 콘솔 출력 레이어
    let stdout_layer = fmt::layer()
        .with_target(true)
        .with_level(true)
        .with_thread_ids(false);

    // 파일 로깅 (app_data_dir이 있는 경우에만)
    if let Some(data_dir) = app_data_dir {
        let logs_dir = data_dir.join("logs");
        let _ = std::fs::create_dir_all(&logs_dir);

        let file_appender = RollingFileAppender::new(
            Rotation::DAILY,
            &logs_dir,
            "docufinder.log",
        );

        let file_layer = fmt::layer()
            .with_ansi(false)
            .with_target(true)
            .with_level(true)
            .with_writer(file_appender);

        tracing_subscriber::registry()
            .with(filter)
            .with(stdout_layer)
            .with(file_layer)
            .init();

        tracing::info!("Logging initialized. Log dir: {:?}", logs_dir);
    } else {
        // 콘솔만
        tracing_subscriber::registry()
            .with(filter)
            .with(stdout_layer)
            .init();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            // Initialize app data directory
            let app_data_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("Failed to get app data dir: {}", e))?;
            std::fs::create_dir_all(&app_data_dir)
                .map_err(|e| format!("Failed to create app data dir: {}", e))?;

            // 로깅 초기화 (콘솔 + 파일)
            init_logging(Some(&app_data_dir));

            // Create models directory
            let models_dir = app_data_dir.join("models");
            std::fs::create_dir_all(&models_dir).ok();

            // Initialize database
            let state = AppState::new(&app_data_dir);
            db::init_database(&state.db_path)
                .map_err(|e| format!("Failed to initialize database: {}", e))?;

            tracing::info!("DocuFinder initialized. DB: {:?}", state.db_path);

            // Check semantic search availability
            if state.is_semantic_available() {
                tracing::info!("Semantic search: enabled");
            } else {
                tracing::warn!(
                    "Semantic search: disabled (model not found at {:?})",
                    state.models_dir.join("multilingual-e5-small")
                );
            }

            // 기존 감시 폴더들 자동 감시 시작
            if let Ok(conn) = db::get_connection(&state.db_path) {
                if let Ok(folders) = db::get_watched_folders(&conn) {
                    if !folders.is_empty() {
                        if let Ok(wm) = state.get_watch_manager() {
                            if let Ok(mut wm) = wm.write() {
                                for folder in folders {
                                    let path = std::path::Path::new(&folder);
                                    if path.exists() {
                                        if let Err(e) = wm.watch(path) {
                                            tracing::warn!("Failed to watch {}: {}", folder, e);
                                        } else {
                                            tracing::info!("Resumed watching: {}", folder);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Store app state
            app.manage(Mutex::new(state));

            // 개발 모드에서 DevTools 열기
            #[cfg(debug_assertions)]
            if let Some(window) = app.get_webview_window("main") {
                window.open_devtools();
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            // 앱 종료 시 벡터 인덱스 저장
            if let tauri::WindowEvent::Destroyed = event {
                if let Some(state) = window.try_state::<Mutex<AppState>>() {
                    if let Ok(state) = state.lock() {
                        if let Ok(vi) = state.get_vector_index() {
                            if let Err(e) = vi.save() {
                                tracing::error!("Failed to save vector index: {}", e);
                            }
                        }
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::search::search_keyword,
            commands::search::search_semantic,
            commands::search::search_hybrid,
            commands::index::add_folder,
            commands::index::remove_folder,
            commands::index::get_index_status,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::file::open_file,
            commands::file::open_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
