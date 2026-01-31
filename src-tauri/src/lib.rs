mod application;      // 클린 아키텍처: Application Layer
mod commands;
mod constants;
mod db;
mod domain;           // 클린 아키텍처: Domain Layer
mod embedder;
mod error;
mod indexer;
mod infrastructure;   // 클린 아키텍처: Infrastructure Layer
mod model_downloader; // 모델 자동 다운로드
mod parsers;
mod reranker;         // Cross-Encoder Reranking (Phase 5)
mod search;
mod tokenizer;        // 한국어 형태소 분석 (Phase 5)
mod utils;            // 유틸리티 (idle_detector, disk_info)

pub use error::{ApiError, ApiResult};
pub use application::container::AppContainer;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tauri::{Emitter, Manager};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::menu::{Menu, MenuItem};
use tauri_plugin_autostart::MacosLauncher;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};
use tracing_appender::rolling::{RollingFileAppender, Rotation};

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
    // 크래시 핸들러 설정 (패닉 발생 시 로그 기록)
    std::panic::set_hook(Box::new(|panic_info| {
        let location = panic_info.location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "unknown".to_string());

        let message = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic".to_string()
        };

        eprintln!("╔══════════════════════════════════════════════════════════╗");
        eprintln!("║                    CRITICAL ERROR                        ║");
        eprintln!("╚══════════════════════════════════════════════════════════╝");
        eprintln!("Location: {}", location);
        eprintln!("Message: {}", message);
        eprintln!("Please report this issue at: https://github.com/your-repo/issues");

        // 긴급 로그 flush (tracing이 초기화되지 않았을 수 있음)
        if let Some(data_dir) = dirs::data_dir() {
            let crash_log = data_dir.join("com.anything.app").join("crash.log");
            let _ = std::fs::write(&crash_log, format!(
                "[{}] PANIC at {}: {}\n",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                location,
                message
            ));
        }
    }));

    // tokenizers 병렬 처리 비활성화 (rayon과의 데드락 방지)
    std::env::set_var("TOKENIZERS_PARALLELISM", "false");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        // updater 비활성화 (사내 배포용 - 외부 통신 차단)
        // .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec!["--minimized"])))
        .plugin(tauri_plugin_window_state::Builder::new().build())
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

            // 번들된 모델 리소스를 app_data로 복사 (최초 1회)
            let model_target = models_dir.join("multilingual-e5-small");
            let model_marker = model_target.join("model.onnx");
            if !model_marker.exists() {
                if let Ok(resource_dir) = app.path().resource_dir() {
                    let resource_model = resource_dir.join("models").join("multilingual-e5-small");
                    if resource_model.exists() {
                        tracing::info!("Copying bundled model from {:?} to {:?}", resource_model, model_target);
                        std::fs::create_dir_all(&model_target).ok();

                        // 모델 파일들 복사
                        for entry in std::fs::read_dir(&resource_model).into_iter().flatten() {
                            if let Ok(entry) = entry {
                                let src = entry.path();
                                let dest = model_target.join(entry.file_name());
                                if src.is_file() {
                                    if let Err(e) = std::fs::copy(&src, &dest) {
                                        tracing::warn!("Failed to copy {:?}: {}", src, e);
                                    }
                                }
                            }
                        }
                        tracing::info!("Model files copied successfully");
                    }
                }
            }

            // 모델이 없으면 자동 다운로드 (시맨틱 활성화 시에만)
            let setup_settings = crate::commands::settings::get_settings_sync(&app_data_dir);
            let e5_model = models_dir.join("multilingual-e5-small").join("model.onnx");
            let reranker_model = models_dir.join("ms-marco-MiniLM-L6-v2").join("model.onnx");

            if setup_settings.semantic_search_enabled && (!e5_model.exists() || !reranker_model.exists()) {
                tracing::info!("모델 파일이 없습니다. 자동 다운로드를 시작합니다...");
                match model_downloader::ensure_models(&models_dir) {
                    Ok(result) => {
                        let any_downloaded = result.onnx_runtime_downloaded
                            || result.model_downloaded
                            || result.tokenizer_downloaded
                            || result.reranker_model_downloaded
                            || result.reranker_tokenizer_downloaded;

                        if any_downloaded {
                            tracing::info!(
                                "모델 다운로드 완료: ONNX Runtime={}, E5 Model={}, E5 Tokenizer={}, Reranker Model={}, Reranker Tokenizer={}",
                                result.onnx_runtime_downloaded,
                                result.model_downloaded,
                                result.tokenizer_downloaded,
                                result.reranker_model_downloaded,
                                result.reranker_tokenizer_downloaded
                            );
                        }
                    }
                    Err(e) => {
                        tracing::error!("모델 다운로드 실패: {}. 일부 기능이 비활성화됩니다.", e);
                    }
                }
            }

            // Initialize database with AppContainer
            let container = AppContainer::new(&app_data_dir);
            db::init_database(&container.db_path)
                .map_err(|e| format!("Failed to initialize database: {}", e))?;

            tracing::info!("DocuFinder initialized. DB: {:?}", container.db_path);

            // Check semantic search availability
            if container.is_semantic_available() {
                tracing::info!("Semantic search: enabled");
            } else {
                tracing::warn!(
                    "Semantic search: disabled (model not found at {:?})",
                    container.models_dir.join("multilingual-e5-small")
                );
            }

            // Check reranker availability
            if container.is_reranker_available() {
                tracing::info!("Reranker: enabled (ms-marco-MiniLM-L6-v2)");
            } else {
                tracing::warn!(
                    "Reranker: disabled (model not found at {:?})",
                    container.models_dir.join("ms-marco-MiniLM-L6-v2")
                );
            }

            // 기존 감시 폴더들 자동 감시 시작
            if let Ok(conn) = db::get_connection(&container.db_path) {
                if let Ok(folders) = db::get_watched_folders(&conn) {
                    if !folders.is_empty() {
                        if let Ok(wm) = container.get_watch_manager() {
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

            // ⚡ 파일명 캐시 로드 (Everything 스타일 빠른 검색)
            match container.load_filename_cache() {
                Ok(count) => {
                    tracing::info!("FilenameCache loaded: {} files", count);
                }
                Err(e) => {
                    tracing::warn!("Failed to load filename cache: {}", e);
                }
            }

            // 벡터 인덱스 파일 검증 - DB와 불일치 시 리셋
            let vector_file = container.vector_index_path.clone();
            let map_file = container.vector_index_path.with_extension("map");
            let vector_file_exists = vector_file.exists();
            let map_file_exists = map_file.exists();

            if container.is_semantic_available() {
                if let Ok(conn) = db::get_connection(&container.db_path) {
                    if let Ok(stats) = db::get_vector_indexing_stats(&conn) {
                        // DB에는 벡터 인덱싱 완료된 파일이 있는데 인덱스 파일이 없으면 리셋
                        if stats.vector_indexed_files > 0 && (!vector_file_exists || !map_file_exists) {
                            tracing::warn!(
                                "Vector index file missing (usearch={}, map={}), but DB has {} indexed files. Resetting DB.",
                                vector_file_exists, map_file_exists, stats.vector_indexed_files
                            );
                            if let Ok(reset_count) = db::reset_all_vector_indexed(&conn) {
                                tracing::info!("Reset vector_indexed_at for {} files", reset_count);
                            }
                        }
                    }
                }
            }

            // Store app container
            app.manage(Mutex::new(container));

            // 미완료 벡터 인덱싱 자동 재개 (시맨틱 활성화 + 자동 모드일 때만)
            if let Some(container) = app.try_state::<Mutex<AppContainer>>() {
                if let Ok(container) = container.lock() {
                    let startup_settings = container.db_path.parent()
                        .map(|dir| crate::commands::settings::get_settings_sync(&dir.to_path_buf()))
                        .unwrap_or_default();
                    let should_auto_resume = container.is_semantic_available()
                        && startup_settings.semantic_search_enabled
                        && startup_settings.vector_indexing_mode == crate::commands::settings::VectorIndexingMode::Auto;
                    if should_auto_resume {
                        if let Ok(conn) = db::get_connection(&container.db_path) {
                            if let Ok(stats) = db::get_vector_indexing_stats(&conn) {
                                if stats.pending_chunks > 0 {
                                    tracing::info!(
                                        "Found {} pending vector chunks. Starting background indexing.",
                                        stats.pending_chunks
                                    );
                                    let embedder = container.get_embedder();
                                    let vector_index = container.get_vector_index();
                                    let vector_worker = container.get_vector_worker();
                                    let db_path = container.db_path.clone();

                                    if let (Ok(emb), Ok(vi)) = (embedder, vector_index) {
                                        if let Ok(mut worker) = vector_worker.write() {
                                            let app_handle = app.handle().clone();
                                            let _ = worker.start(
                                                db_path,
                                                emb,
                                                vi,
                                                Some(Arc::new(move |progress| {
                                                    let _ = app_handle.emit("vector-indexing-progress", &progress);
                                                })),
                                                Some(startup_settings.indexing_intensity.clone()),
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // 개발 모드에서 DevTools 열기
            #[cfg(debug_assertions)]
            if let Some(window) = app.get_webview_window("main") {
                window.open_devtools();
            }

            // 시스템 트레이 설정
            let show_item = MenuItem::with_id(app, "show", "열기", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "종료", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .tooltip("Anything")
                .on_menu_event(|app, event| {
                    match event.id.as_ref() {
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "quit" => {
                            // 벡터 워커 정리 + 인덱스 저장
                            if let Some(container) = app.try_state::<Mutex<AppContainer>>() {
                                if let Ok(container) = container.lock() {
                                    // 벡터 워커 취소 + 대기
                                    let vector_worker = container.get_vector_worker();
                                    if let Ok(mut worker) = vector_worker.write() {
                                        if worker.is_running() {
                                            tracing::info!("Stopping vector worker before exit...");
                                            worker.cancel();
                                            worker.join();
                                        }
                                    }
                                    // 벡터 인덱스 저장
                                    if let Ok(vi) = container.get_vector_index() {
                                        tracing::info!("Saving vector index before exit...");
                                        if let Err(e) = vi.save() {
                                            tracing::error!("Failed to save vector index: {}", e);
                                        }
                                    }
                                }
                            }
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            tracing::info!("System tray initialized");

            // 시작 시 최소화 처리 (--minimized 인자 또는 설정)
            let args: Vec<String> = std::env::args().collect();
            let minimized_arg = args.iter().any(|a| a == "--minimized");
            let settings = commands::settings::get_settings_sync(&app_data_dir);

            if minimized_arg || settings.start_minimized {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                    tracing::info!("Started minimized to tray");
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
                // X 버튼 클릭 시 트레이로 최소화
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    let _ = window.hide();
                    tracing::debug!("Window hidden to tray");
                }
                // 앱 종료 시 벡터 인덱스 저장
                tauri::WindowEvent::Destroyed => {
                    if let Some(container) = window.try_state::<Mutex<AppContainer>>() {
                        if let Ok(container) = container.lock() {
                            if let Ok(vi) = container.get_vector_index() {
                                if let Err(e) = vi.save() {
                                    tracing::error!("Failed to save vector index: {}", e);
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::search::search_keyword,
            commands::search::search_filename,
            commands::search::search_semantic,
            commands::search::search_hybrid,
            commands::index::add_folder,
            commands::index::remove_folder,
            commands::index::get_index_status,
            commands::index::get_folder_stats,
            commands::index::get_folders_with_info,
            commands::index::toggle_favorite,
            commands::index::cancel_indexing,
            commands::index::reindex_folder,
            commands::index::get_vector_indexing_status,
            commands::index::cancel_vector_indexing,
            commands::index::start_vector_indexing,
            commands::index::get_db_debug_info,
            commands::index::clear_all_data,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::file::open_file,
            commands::file::open_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
