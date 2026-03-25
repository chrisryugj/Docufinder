mod ai; // Gemini RAG (v2.6)
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
pub mod ocr; // PaddleOCR ONNX 기반 OCR 엔진
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

/// 모델 파일이 없으면 비동기 자동 다운로드 시작
fn maybe_download_models(
    app_handle: tauri::AppHandle,
    models_dir: PathBuf,
    semantic_enabled: bool,
) {
    let e5_model_int8 = models_dir
        .join("kosimcse-roberta-multitask")
        .join("model_int8.onnx");
    let e5_model = models_dir
        .join("kosimcse-roberta-multitask")
        .join("model.onnx");
    let e5_model_data = models_dir
        .join("kosimcse-roberta-multitask")
        .join("model.onnx.data");
    let reranker_model = models_dir.join("ms-marco-MiniLM-L6-v2").join("model.onnx");

    let embedder_available =
        e5_model_int8.exists() || (e5_model.exists() && e5_model_data.exists());
    if !semantic_enabled || (embedder_available && reranker_model.exists()) {
        return;
    }

    tauri::async_runtime::spawn(async move {
        tracing::info!("모델 파일이 없습니다. 백그라운드 다운로드를 시작합니다...");
        let _ = app_handle.emit("model-download-status", "downloading");

        match tokio::task::spawn_blocking(move || model_downloader::ensure_models(&models_dir))
            .await
        {
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
                let _ = app_handle.emit("model-download-status", "completed");
            }
            Ok(Err(e)) => {
                tracing::error!("모델 다운로드 실패: {}. 일부 기능이 비활성화됩니다.", e);
                let _ = app_handle.emit("model-download-status", "failed");
            }
            Err(e) => {
                tracing::error!("모델 다운로드 태스크 실패: {}", e);
                let _ = app_handle.emit("model-download-status", "failed");
            }
        }
    });
}

/// OCR 모델 파일이 없으면 비동기 자동 다운로드 시작
fn maybe_download_ocr_models(app_handle: tauri::AppHandle, models_dir: PathBuf, ocr_enabled: bool) {
    if !ocr_enabled {
        return;
    }

    let ocr_dir = models_dir.join("paddleocr");
    let det_exists = ocr_dir.join("det.onnx").exists();
    let rec_exists = ocr_dir.join("rec.onnx").exists();
    let dict_exists = ocr_dir.join("dict.txt").exists();

    if det_exists && rec_exists && dict_exists {
        return;
    }

    tauri::async_runtime::spawn(async move {
        tracing::info!("OCR 모델 파일이 없습니다. 백그라운드 다운로드를 시작합니다...");
        let _ = app_handle.emit("model-download-status", "downloading-ocr");

        match tokio::task::spawn_blocking(move || model_downloader::ensure_ocr_models(&models_dir))
            .await
        {
            Ok(Ok((det, rec, dict))) => {
                if det || rec || dict {
                    tracing::info!(
                        "OCR 모델 다운로드 완료: det={}, rec={}, dict={}",
                        det,
                        rec,
                        dict
                    );
                }
                let _ = app_handle.emit("model-download-status", "completed-ocr");
            }
            Ok(Err(e)) => {
                tracing::error!("OCR 모델 다운로드 실패: {}", e);
                let _ = app_handle.emit("model-download-status", "failed-ocr");
            }
            Err(e) => {
                tracing::error!("OCR 모델 다운로드 태스크 실패: {}", e);
                let _ = app_handle.emit("model-download-status", "failed-ocr");
            }
        }
    });
}

/// 기존 감시 폴더들 자동 감시 복원
fn resume_watchers(container: &AppContainer) {
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
}

/// 벡터 인덱스 파일 ↔ DB 정합성 검증
fn validate_vector_index(container: &AppContainer) {
    let vector_file = container.vector_index_path.clone();
    let map_file = container.vector_index_path.with_extension("map");
    let vector_file_exists = vector_file.exists();
    let map_file_exists = map_file.exists();

    if container.is_semantic_available() {
        if let Ok(conn) = db::get_connection(&container.db_path) {
            if let Ok(stats) = db::get_vector_indexing_stats(&conn) {
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
}

/// 미완료 벡터 인덱싱 자동 재개
fn auto_resume_vector_indexing(app: &tauri::App) {
    let Some(container_state) = app.try_state::<RwLock<AppContainer>>() else {
        return;
    };
    let Ok(container) = container_state.read() else {
        return;
    };

    let startup_settings = container.get_settings();
    let should_auto_resume = container.is_semantic_available()
        && startup_settings.semantic_search_enabled
        && startup_settings.vector_indexing_mode
            == crate::commands::settings::VectorIndexingMode::Auto;
    if !should_auto_resume {
        return;
    }

    let Ok(conn) = db::get_connection(&container.db_path) else {
        return;
    };
    let Ok(stats) = db::get_vector_indexing_stats(&conn) else {
        return;
    };
    if stats.pending_chunks == 0 {
        return;
    }

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

/// 앱 시작 시 완료된 폴더 자동 동기화 (오프라인 변경 감지)
fn spawn_startup_sync(app_handle: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        let (folders_to_sync, service, include_subfolders, max_file_size_mb, db_path, exclude_dirs) = {
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
            let folder_infos = db::get_watched_folders_with_info(&conn).unwrap_or_default();
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            const SYNC_SKIP_SECS: i64 = 300;
            let completed: Vec<String> = folder_infos
                .into_iter()
                .filter(|f| {
                    if f.indexing_status != "completed" {
                        return false;
                    }
                    match f.last_synced_at {
                        Some(ts) if (now - ts) < SYNC_SKIP_SECS => {
                            tracing::debug!(
                                "[Startup Sync] Skipping {} (synced {}s ago)",
                                f.path,
                                now - ts
                            );
                            false
                        }
                        _ => true,
                    }
                })
                .map(|f| f.path)
                .collect();

            if completed.is_empty() {
                return;
            }

            let settings = container.get_settings();
            let mut dirs: Vec<String> = crate::constants::DEFAULT_EXCLUDED_DIRS
                .iter()
                .map(|s| s.to_string())
                .collect();
            dirs.extend(settings.exclude_dirs.clone());
            (
                completed,
                container.index_service(),
                settings.include_subfolders,
                settings.max_file_size_mb,
                container.db_path.clone(),
                dirs,
            )
        };

        tracing::info!(
            "[Startup Sync] Checking {} completed folders for offline changes...",
            folders_to_sync.len()
        );

        let mut total_added = 0usize;
        let mut total_deleted = 0usize;

        for folder in &folders_to_sync {
            let path = std::path::Path::new(folder);
            if !path.exists() {
                continue;
            }

            let ah = app_handle.clone();
            let progress_cb: Box<
                dyn Fn(crate::indexer::pipeline::FtsIndexingProgress) + Send + Sync,
            > = Box::new(move |p: crate::indexer::pipeline::FtsIndexingProgress| {
                #[derive(serde::Serialize)]
                struct ProgressEvent {
                    phase: String,
                    total_files: usize,
                    processed_files: usize,
                    current_file: Option<String>,
                    folder_path: String,
                    error: Option<String>,
                }
                let _ = ah.emit(
                    "indexing-progress",
                    &ProgressEvent {
                        phase: p.phase,
                        total_files: p.total_files,
                        processed_files: p.processed_files,
                        current_file: p.current_file,
                        folder_path: p.folder_path,
                        error: None,
                    },
                );
            });

            match service
                .sync_folder(
                    path,
                    include_subfolders,
                    Some(progress_cb),
                    max_file_size_mb,
                    exclude_dirs.clone(),
                )
                .await
            {
                Ok(result) => {
                    total_added += result.added;
                    total_deleted += result.deleted;
                    if let Ok(conn) = db::get_connection(&db_path) {
                        let _ = db::update_last_synced_at(&conn, folder);
                    }
                    if result.added > 0 || result.deleted > 0 {
                        tracing::info!(
                            "[Startup Sync] {}: +{} added, -{} deleted, {} unchanged",
                            folder,
                            result.added,
                            result.deleted,
                            result.unchanged
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!("[Startup Sync] Failed to sync {}: {}", folder, e);
                }
            }
        }

        if total_added > 0 || total_deleted > 0 {
            if let Some(cs) = app_handle.try_state::<RwLock<AppContainer>>() {
                if let Ok(c) = cs.read() {
                    let _ = c.load_filename_cache();
                }
            }
            tracing::info!(
                "[Startup Sync] Complete: {} added, {} deleted",
                total_added,
                total_deleted
            );
        } else {
            tracing::info!("[Startup Sync] No offline changes detected");
        }
    });
}

/// 벡터 워커 정리 + 인덱스 저장 (종료/트레이 quit 공통)
fn cleanup_vector_resources(container: &AppContainer) {
    let vector_worker = container.get_vector_worker();
    if let Ok(mut worker) = vector_worker.write() {
        if worker.is_running() {
            tracing::info!("Stopping vector worker...");
            worker.cancel();
            worker.join();
        }
    }
    if let Ok(vi) = container.get_vector_index() {
        if let Err(e) = vi.save() {
            tracing::error!("Failed to save vector index: {}", e);
        }
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

            // 크기 제한: 1MB 초과 시 truncate (디스크 무한 증가 방지)
            const MAX_CRASH_LOG_SIZE: u64 = 1024 * 1024;
            if let Ok(meta) = std::fs::metadata(&crash_log) {
                if meta.len() > MAX_CRASH_LOG_SIZE {
                    let _ = std::fs::remove_file(&crash_log);
                }
            }

            let entry = format!(
                "[{}] PANIC at {}: {}\n",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                location,
                message
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
        // 싱글 인스턴스: 중복 실행 시 기존 창 포커스 (가장 먼저 등록해야 함)
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        // tauri-plugin-fs: 프론트엔드에서 미사용 (capabilities 미부여)
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .plugin(
            tauri_plugin_window_state::Builder::new()
                // VISIBLE 복원 제외: start_minimized 설정을 무시하고 창을 띄우는 문제 방지
                .with_state_flags(
                    tauri_plugin_window_state::StateFlags::all()
                        & !tauri_plugin_window_state::StateFlags::VISIBLE,
                )
                .build(),
        )
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

            // ORT_DYLIB_PATH 설정: 단일 스레드(setup) 시점에서 환경변수 설정
            // container.rs OnceCell 내부(멀티스레드 가능)에서 호출하던 것을 여기로 이동
            // SAFETY: setup()은 main 스레드에서 실행되며, ort 라이브러리 초기화 전임.
            // Rust 1.81+ deprecated이나 프로세스 초기화 시점이므로 안전함.
            {
                let dll_path = models_dir
                    .join("kosimcse-roberta-multitask")
                    .join("onnxruntime.dll");
                if dll_path.exists() {
                    unsafe { std::env::set_var("ORT_DYLIB_PATH", &dll_path) };
                    tracing::info!("ORT_DYLIB_PATH set to {:?}", dll_path);
                }
            }

            // 모델 자동 다운로드 (백그라운드)
            let setup_settings = crate::commands::settings::get_settings_sync(&app_data_dir);
            maybe_download_models(
                app.handle().clone(),
                models_dir.clone(),
                setup_settings.semantic_search_enabled,
            );
            maybe_download_ocr_models(
                app.handle().clone(),
                models_dir.clone(),
                setup_settings.ocr_enabled,
            );

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

            // Check OCR availability
            if container.is_ocr_available() && setup_settings.ocr_enabled {
                tracing::info!("OCR: enabled (PaddleOCR ONNX)");
            } else if setup_settings.ocr_enabled {
                tracing::warn!("OCR: enabled but model not found (downloading...)");
            } else {
                tracing::info!("OCR: disabled");
            }

            // 증분 인덱싱 완료 시 프론트엔드 알림 콜백 설정
            {
                let app_handle = app.handle().clone();
                container.set_incremental_update_callback(Arc::new(move |count| {
                    tracing::info!("[WatchManager] Incremental update: {} files", count);
                    let _ = app_handle.emit("incremental-index-updated", count);
                }));
            }

            // 증분 인덱싱 시 HWP 파일 감지 콜백 설정
            {
                let app_handle = app.handle().clone();
                container.set_hwp_detected_callback(Arc::new(move |paths| {
                    tracing::info!("[WatchManager] HWP files detected: {} files", paths.len());
                    let _ = app_handle.emit("hwp-files-detected", paths);
                }));
            }

            // 기존 감시 폴더들 자동 감시 복원
            resume_watchers(&container);

            // ⚡ 디스크 타입 사전 감지 (C:, D: — PowerShell 호출 1-3초를 앱 시작 시 흡수)
            tauri::async_runtime::spawn(async {
                tokio::task::spawn_blocking(|| {
                    for letter in ['C', 'D', 'E'] {
                        let path = format!("{}:\\", letter);
                        if std::path::Path::new(&path).exists() {
                            let _ = crate::utils::disk_info::detect_disk_type(
                                std::path::Path::new(&path),
                            );
                        }
                    }
                    tracing::debug!("Disk type pre-detection completed");
                })
                .await
                .ok();
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

            // 벡터 인덱스 ↔ DB 정합성 검증
            validate_vector_index(&container);

            // Store app container
            app.manage(RwLock::new(container));

            // 미완료 벡터 인덱싱 자동 재개
            auto_resume_vector_indexing(app);

            // 완료된 폴더 자동 동기화 (오프라인 변경 감지)
            spawn_startup_sync(app.handle().clone());

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

            // 트레이 전용 아이콘 로드 (anything-l.png), 실패 시 기본 아이콘 fallback
            let tray_icon = {
                let tray_icon_path = app
                    .path()
                    .resource_dir()
                    .ok()
                    .map(|d| d.join("icons").join("tray-icon.png"))
                    .unwrap_or_default();
                if tray_icon_path.exists() {
                    match tauri::image::Image::from_path(&tray_icon_path) {
                        Ok(img) => {
                            tracing::info!("Loaded tray icon from {:?}", tray_icon_path);
                            img
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Failed to load tray icon: {e}, falling back to default"
                            );
                            app.default_window_icon()
                                .cloned()
                                .unwrap_or_else(|| tauri::image::Image::new(&[], 0, 0))
                        }
                    }
                } else {
                    tracing::debug!(
                        "Tray icon file not found at {:?}, using default",
                        tray_icon_path
                    );
                    app.default_window_icon()
                        .cloned()
                        .unwrap_or_else(|| tauri::image::Image::new(&[], 0, 0))
                }
            };
            let _tray = TrayIconBuilder::new()
                .icon(tray_icon)
                .menu(&menu)
                .show_menu_on_left_click(false)
                .tooltip("Anything")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => {
                        if let Some(container) = app.try_state::<RwLock<AppContainer>>() {
                            if let Ok(container) = container.read() {
                                cleanup_vector_resources(&container);
                            }
                        }
                        app.exit(0);
                    }
                    _ => {}
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
                // on_page_load에서 show하지 않도록 플래그 설정
                show_on_load.store(false, Ordering::Relaxed);
                // setup 시점에도 명시적으로 숨김 (window-state나 Tauri 내부에서 show될 수 있으므로)
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
                tracing::info!("Started minimized to tray");
            }

            Ok(())
        })
        .on_page_load(move |webview, payload| {
            let event = payload.event();
            tracing::info!(
                "[PERF] on_page_load: url={}, event={:?}",
                payload.url(),
                event
            );

            if let Some(window) = webview.app_handle().get_webview_window("main") {
                if show_on_load_flag.load(Ordering::Relaxed) {
                    // 일반 시작: Finished 이벤트에서 창 표시 (검정화면 방지)
                    if matches!(event, tauri::webview::PageLoadEvent::Finished) {
                        let _ = window.show();
                        let _ = window.set_focus();
                        tracing::info!("[PERF] Window shown after page load");
                    }
                } else {
                    // start_minimized: Started/Finished 이벤트 모두에서 즉시 숨김
                    let _ = window.hide();
                    tracing::info!("[PERF] Window hidden (start minimized, event={:?})", event);
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
                tauri::WindowEvent::Destroyed => {
                    if let Some(container) = window.try_state::<RwLock<AppContainer>>() {
                        if let Ok(container) = container.read() {
                            cleanup_vector_resources(&container);
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
            commands::search::search_smart,
            commands::search::find_similar_documents,
            commands::search::classify_document,
            commands::search::get_suggestions,
            commands::search::save_search_query,
            commands::search::get_document_statistics,
            commands::index::add_folder,
            commands::index::remove_folder,
            commands::index::get_index_status,
            commands::index::get_folder_stats,
            commands::index::get_all_folder_stats,
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
            commands::index::convert_hwp_to_hwpx,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::verify_admin_code,
            commands::file::open_file,
            commands::file::open_url,
            commands::file::open_folder,
            commands::file::log_frontend_error,
            commands::file::get_log_dir,
            commands::file::open_log_dir,
            commands::system::get_suggested_folders,
            commands::preview::load_document_preview,
            commands::preview::add_bookmark,
            commands::preview::remove_bookmark,
            commands::preview::update_bookmark_note,
            commands::preview::get_bookmarks,
            commands::preview::generate_summary,
            commands::ai::ask_ai,
            commands::export::export_csv,
            commands::export::export_xlsx,
            commands::export::package_zip,
            commands::duplicate::find_duplicates,
            commands::expiry::scan_expiry_dates,
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            eprintln!("Fatal: Tauri failed to start: {}", e);
            // 크래시 로그에도 기록 (append 모드: 이전 기록 보존)
            if let Some(data_dir) = dirs::data_dir() {
                let crash_dir = data_dir.join("com.anything.app");
                let _ = std::fs::create_dir_all(&crash_dir);
                let crash_log = crash_dir.join("crash.log");
                // 크기 제한: 1MB 초과 시 truncate
                const MAX_CRASH_LOG_SIZE: u64 = 1024 * 1024;
                if let Ok(meta) = std::fs::metadata(&crash_log) {
                    if meta.len() > MAX_CRASH_LOG_SIZE {
                        let _ = std::fs::remove_file(&crash_log);
                    }
                }
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
