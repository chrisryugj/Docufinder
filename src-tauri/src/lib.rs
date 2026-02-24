mod application; // 클린 아키텍처: Application Layer
mod commands;
mod constants;
mod db;
mod domain; // 클린 아키텍처: Domain Layer
mod embedder;
mod error;
mod indexer;
mod infrastructure; // 클린 아키텍처: Infrastructure Layer
mod model_downloader; // 모델 자동 다운로드
pub mod parsers;
mod reranker; // Cross-Encoder Reranking (Phase 5)
mod search;
mod tokenizer; // 한국어 형태소 분석 (Phase 5)
mod utils; // 유틸리티 (idle_detector, disk_info)

pub use application::container::AppContainer;
pub use error::{ApiError, ApiResult};

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};
use tauri_plugin_autostart::MacosLauncher;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// 로깅 초기화 (파일 + 콘솔)
fn init_logging(app_data_dir: Option<&PathBuf>) {
    // 기본 필터: 릴리즈에서는 info, 디버그에서는 debug
    let default_filter = if cfg!(debug_assertions) {
        "docufinder=debug,tauri=info"
    } else {
        "docufinder=info,tauri=warn"
    };

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));

    // 콘솔 출력 레이어
    let stdout_layer = fmt::layer()
        .with_target(true)
        .with_level(true)
        .with_thread_ids(false);

    // 파일 로깅 (app_data_dir이 있는 경우에만)
    if let Some(data_dir) = app_data_dir {
        let logs_dir = data_dir.join("logs");
        let _ = std::fs::create_dir_all(&logs_dir);

        let file_appender = RollingFileAppender::builder()
            .rotation(Rotation::DAILY)
            .filename_prefix("docufinder")
            .filename_suffix("log")
            .max_log_files(7) // 7일분만 보존, C: 누적 방지
            .build(&logs_dir)
            .expect("Failed to create log file appender");

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
        let location = panic_info
            .location()
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
        eprintln!("Please contact the development team to report this issue.");

        // 긴급 로그 flush (append 모드로 이전 크래시 기록 보존)
        if let Some(data_dir) = dirs::data_dir() {
            let crash_dir = data_dir.join("com.anything.app");
            let _ = std::fs::create_dir_all(&crash_dir);
            let crash_log = crash_dir.join("crash.log");
            let entry = format!(
                "[{}] PANIC at {}: {}\n",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                location,
                message
            );
            // append 모드: 기존 크래시 기록 보존
            use std::io::Write;
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&crash_log)
            {
                let _ = file.write_all(entry.as_bytes());
            }
        }
    }));

    // tokenizers 병렬 처리 비활성화 (rayon과의 데드락 방지)
    // SAFETY: run() 진입 직후, main 스레드만 존재하는 단일 스레드 컨텍스트.
    // tauri::Builder 생성 전이므로 다른 스레드가 환경변수를 읽을 수 없음.
    // Rust 1.81+ deprecated이나 프로세스 초기화 시점이므로 안전함.
    unsafe { std::env::set_var("TOKENIZERS_PARALLELISM", "false") };

    // visible: false → page load 완료 후 창 표시 (검정화면 방지)
    // Dev mode: WebView2 SmartScreen 비활성화는 package.json tauri:dev 스크립트에서 설정
    let show_on_load = Arc::new(AtomicBool::new(true));
    let show_on_load_flag = show_on_load.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        // tauri-plugin-fs: 프론트엔드에서 미사용 (capabilities 미부여)
        // tauri-plugin-updater: 사내 배포용 비활성화 (외부 통신 차단)
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec!["--minimized"])))
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .setup(move |app| {
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

            // 모델이 없으면 비동기 자동 다운로드 (시맨틱 활성화 시에만, UI 블로킹 방지)
            let setup_settings = crate::commands::settings::get_settings_sync(&app_data_dir);
            let e5_model = models_dir.join("kosimcse-roberta-multitask").join("model.onnx");
            let e5_model_data = models_dir.join("kosimcse-roberta-multitask").join("model.onnx.data");
            let reranker_model = models_dir.join("ms-marco-MiniLM-L6-v2").join("model.onnx");

            if setup_settings.semantic_search_enabled && (!e5_model.exists() || !e5_model_data.exists() || !reranker_model.exists()) {
                let download_models_dir = models_dir.clone();
                let download_app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    tracing::info!("모델 파일이 없습니다. 백그라운드 다운로드를 시작합니다...");
                    let _ = download_app_handle.emit("model-download-status", "downloading");

                    match tokio::task::spawn_blocking(move || {
                        model_downloader::ensure_models(&download_models_dir)
                    }).await {
                        Ok(Ok(result)) => {
                            let any_downloaded = result.onnx_runtime_downloaded
                                || result.model_downloaded
                                || result.model_data_downloaded
                                || result.tokenizer_downloaded
                                || result.reranker_model_downloaded
                                || result.reranker_tokenizer_downloaded;

                            if any_downloaded {
                                tracing::info!(
                                    "모델 다운로드 완료: ONNX Runtime={}, Model={}, ModelData={}, Tokenizer={}, Reranker={}, RerankerTokenizer={}",
                                    result.onnx_runtime_downloaded,
                                    result.model_downloaded,
                                    result.model_data_downloaded,
                                    result.tokenizer_downloaded,
                                    result.reranker_model_downloaded,
                                    result.reranker_tokenizer_downloaded
                                );
                            }
                            let _ = download_app_handle.emit("model-download-status", "completed");
                        }
                        Ok(Err(e)) => {
                            tracing::error!("모델 다운로드 실패: {}. 일부 기능이 비활성화됩니다.", e);
                            let _ = download_app_handle.emit("model-download-status", "failed");
                        }
                        Err(e) => {
                            tracing::error!("모델 다운로드 태스크 실패: {}", e);
                            let _ = download_app_handle.emit("model-download-status", "failed");
                        }
                    }
                });
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
                    container.models_dir.join("kosimcse-roberta-multitask")
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

            // 증분 인덱싱 완료 시 프론트엔드 알림 콜백 설정
            {
                let app_handle = app.handle().clone();
                container.set_incremental_update_callback(Arc::new(move |count| {
                    tracing::info!("[WatchManager] Incremental update: {} files", count);
                    let _ = app_handle.emit("incremental-index-updated", count);
                }));
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

            // ⚡ 디스크 타입 사전 감지 (C:, D: — PowerShell 호출 1-3초를 앱 시작 시 흡수)
            tauri::async_runtime::spawn(async {
                tokio::task::spawn_blocking(|| {
                    for letter in ['C', 'D', 'E'] {
                        let path = format!("{}:\\", letter);
                        if std::path::Path::new(&path).exists() {
                            let _ = crate::utils::disk_info::detect_disk_type(std::path::Path::new(&path));
                        }
                    }
                    tracing::debug!("Disk type pre-detection completed");
                }).await.ok();
            });

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
            app.manage(RwLock::new(container));

            // 미완료 벡터 인덱싱 자동 재개 (시맨틱 활성화 + 자동 모드일 때만)
            if let Some(container) = app.try_state::<RwLock<AppContainer>>() {
                if let Ok(container) = container.read() {
                    let startup_settings = container.get_settings();
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

            // 앱 시작 시 완료된 폴더 자동 동기화 (오프라인 변경 감지)
            {
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    // 앱 초기화 완료 대기 (UI 렌더링 우선, 1초면 충분)
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

                    let (folders_to_sync, service, include_subfolders, max_file_size_mb, db_path) = {
                        let container_state = match app_handle.try_state::<RwLock<AppContainer>>() {
                            Some(c) => c,
                            None => return,
                        };
                        let container = match container_state.read() {
                            Ok(c) => c,
                            Err(_) => return,
                        };
                        let conn = match db::get_connection(&container.db_path) {
                            Ok(c) => c,
                            Err(_) => return,
                        };
                        // 완료된 폴더만 (미완료/취소는 FolderTree가 resume_indexing으로 처리)
                        let folder_infos = db::get_watched_folders_with_info(&conn).unwrap_or_default();
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs() as i64)
                            .unwrap_or(0);
                        // 최근 5분 이내 동기화된 폴더는 스킵 (HDD 부하 방지)
                        const SYNC_SKIP_SECS: i64 = 300;
                        let completed: Vec<String> = folder_infos.into_iter()
                            .filter(|f| {
                                if f.indexing_status != "completed" { return false; }
                                match f.last_synced_at {
                                    Some(ts) if (now - ts) < SYNC_SKIP_SECS => {
                                        tracing::debug!("[Startup Sync] Skipping {} (synced {}s ago)", f.path, now - ts);
                                        false
                                    }
                                    _ => true,
                                }
                            })
                            .map(|f| f.path)
                            .collect();

                        if completed.is_empty() { return; }

                        let settings = container.get_settings();
                        (
                            completed,
                            container.index_service(),
                            settings.include_subfolders,
                            settings.max_file_size_mb,
                            container.db_path.clone(),
                        )
                    };

                    tracing::info!("[Startup Sync] Checking {} completed folders for offline changes...", folders_to_sync.len());

                    let mut total_added = 0usize;
                    let mut total_deleted = 0usize;

                    for folder in &folders_to_sync {
                        let path = std::path::Path::new(folder);
                        if !path.exists() { continue; }

                        let ah = app_handle.clone();
                        let progress_cb: Box<dyn Fn(crate::indexer::pipeline::FtsIndexingProgress) + Send + Sync> =
                            Box::new(move |p: crate::indexer::pipeline::FtsIndexingProgress| {
                                #[derive(serde::Serialize)]
                                struct ProgressEvent {
                                    phase: String,
                                    total_files: usize,
                                    processed_files: usize,
                                    current_file: Option<String>,
                                    folder_path: String,
                                    error: Option<String>,
                                }
                                let _ = ah.emit("indexing-progress", &ProgressEvent {
                                    phase: p.phase,
                                    total_files: p.total_files,
                                    processed_files: p.processed_files,
                                    current_file: p.current_file,
                                    folder_path: p.folder_path,
                                    error: None,
                                });
                            });

                        match service.sync_folder(path, include_subfolders, Some(progress_cb), max_file_size_mb).await {
                            Ok(result) => {
                                total_added += result.added;
                                total_deleted += result.deleted;
                                // 동기화 완료 시각 기록 (다음 시작 시 스킵 판단용)
                                if let Ok(conn) = db::get_connection(&db_path) {
                                    let _ = db::update_last_synced_at(&conn, folder);
                                }
                                if result.added > 0 || result.deleted > 0 {
                                    tracing::info!(
                                        "[Startup Sync] {}: +{} added, -{} deleted, {} unchanged",
                                        folder, result.added, result.deleted, result.unchanged
                                    );
                                }
                            }
                            Err(e) => {
                                tracing::warn!("[Startup Sync] Failed to sync {}: {}", folder, e);
                            }
                        }
                    }

                    // 변경이 있으면 FilenameCache 갱신
                    if total_added > 0 || total_deleted > 0 {
                        if let Some(cs) = app_handle.try_state::<RwLock<AppContainer>>() {
                            if let Ok(c) = cs.read() {
                                let _ = c.load_filename_cache();
                            }
                        }
                        tracing::info!("[Startup Sync] Complete: {} added, {} deleted", total_added, total_deleted);
                    } else {
                        tracing::info!("[Startup Sync] No offline changes detected");
                    }
                });
            }

            // 개발 모드에서 DevTools 열기 (DEVTOOLS=1 환경변수로 제어)
            #[cfg(debug_assertions)]
            if std::env::var("DEVTOOLS").unwrap_or_default() == "1" {
                if let Some(window) = app.get_webview_window("main") {
                    window.open_devtools();
                }
            }

            // 시스템 트레이 설정
            let show_item = MenuItem::with_id(app, "show", "열기", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "종료", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon()
                    .cloned()
                    .unwrap_or_else(|| {
                        tracing::warn!("Default window icon not found, tray icon may not display correctly");
                        tauri::image::Image::new(&[], 0, 0)
                    }))
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
                            if let Some(container) = app.try_state::<RwLock<AppContainer>>() {
                                if let Ok(container) = container.read() {
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
                // visible: false 상태이므로 on_page_load에서 show하지 않도록 플래그 설정
                show_on_load.store(false, Ordering::Relaxed);
                tracing::info!("Started minimized to tray");
            }

            Ok(())
        })
        .on_page_load(move |webview, payload| {
            tracing::info!("[PERF] on_page_load: url={}, event={:?}", payload.url(), payload.event());
            // page load 완료 시 창 표시 (visible: false → show)
            if matches!(payload.event(), tauri::webview::PageLoadEvent::Finished)
                && show_on_load_flag.load(Ordering::Relaxed)
            {
                if let Some(window) = webview.app_handle().get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                    tracing::info!("[PERF] Window shown after page load");
                }
            }
        })
        .on_window_event(|window, event| {
            match event {
                // X 버튼 클릭 시 트레이로 최소화
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    let _ = window.hide();
                    tracing::debug!("Window hidden to tray");
                }
                // 앱 종료 시 벡터 워커 정리 + 인덱스 저장
                tauri::WindowEvent::Destroyed => {
                    if let Some(container) = window.try_state::<RwLock<AppContainer>>() {
                        if let Ok(container) = container.read() {
                            // 벡터 워커 취소 + 대기 (quit 핸들러와 동일)
                            let vector_worker = container.get_vector_worker();
                            if let Ok(mut worker) = vector_worker.write() {
                                if worker.is_running() {
                                    tracing::info!("Stopping vector worker on window destroy...");
                                    worker.cancel();
                                    worker.join();
                                }
                            }
                            // 벡터 인덱스 저장
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
            commands::index::resume_indexing,
            commands::index::get_vector_indexing_status,
            commands::index::cancel_vector_indexing,
            commands::index::start_vector_indexing,
            commands::index::get_db_debug_info,
            commands::index::clear_all_data,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::file::open_file,
            commands::file::open_folder,
            commands::file::log_frontend_error,
            commands::file::get_log_dir,
            commands::file::open_log_dir,
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            eprintln!("Fatal: Tauri failed to start: {}", e);
            // 크래시 로그에도 기록 (append 모드: 이전 기록 보존)
            if let Some(data_dir) = dirs::data_dir() {
                let crash_dir = data_dir.join("com.anything.app");
                let _ = std::fs::create_dir_all(&crash_dir);
                let crash_log = crash_dir.join("crash.log");
                let entry = format!(
                    "[{}] FATAL: Tauri failed to start: {}\n",
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                    e
                );
                use std::io::Write;
                if let Ok(mut file) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&crash_log)
                {
                    let _ = file.write_all(entry.as_bytes());
                }
            }
            std::process::exit(1);
        });
}
