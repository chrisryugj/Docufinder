mod application; // 클린 아키텍처: Application Layer
mod commands;
mod constants;
mod db;
mod domain; // 클린 아키텍처: Domain Layer
mod embedder;
mod error;
mod indexer;
mod infrastructure; // 클린 아키텍처: Infrastructure Layer
mod llm; // LLM 클라이언트 (RAG + AI 요약)
mod model_downloader; // 모델 자동 다운로드
pub mod ocr; // PaddleOCR ONNX 기반 OCR 엔진
pub mod parsers;
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
///
/// app_data_dir이 Some이면 파일 로깅도 활성화.
/// None이면 콘솔만 (app_data_dir 확보 실패 시 fallback).
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

        match RollingFileAppender::builder()
            .rotation(Rotation::DAILY)
            .filename_prefix("docufinder")
            .filename_suffix("log")
            .max_log_files(7) // 7일분만 보존, C: 누적 방지
            .build(&logs_dir)
        {
            Ok(file_appender) => {
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
            }
            Err(e) => {
                // 파일 로그 생성 실패 시 콘솔 전용으로 fallback (앱 시작은 보장)
                tracing_subscriber::registry()
                    .with(filter)
                    .with(stdout_layer)
                    .init();

                tracing::warn!("File logging disabled ({}), using console only", e);
            }
        }
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
    let e5_tokenizer = models_dir
        .join("kosimcse-roberta-multitask")
        .join("tokenizer.json");
    let embedder_available = (e5_model_int8.exists()
        || (e5_model.exists() && e5_model_data.exists()))
        && e5_tokenizer.exists();
    if !semantic_enabled || embedder_available {
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
                    || result.tokenizer_downloaded;

                if any_downloaded {
                    tracing::info!(
                        "모델 다운로드 완료: ONNX Runtime={}, Model={}, ModelData={}, Tokenizer={}",
                        result.onnx_runtime_downloaded,
                        result.model_downloaded,
                        result.model_data_downloaded,
                        result.tokenizer_downloaded,
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

/// 기존 감시 폴더들 자동 감시 복원 (콜백에서 사용)
fn resume_watchers(container: &AppContainer) {
    if let Ok(conn) = db::get_connection(&container.db_path) {
        if let Ok(folders) = db::get_watched_folders(&conn) {
            let existing_folders: Vec<String> = folders
                .into_iter()
                .filter(|folder| std::path::Path::new(folder).exists())
                .collect();
            if !existing_folders.is_empty() {
                if let Ok(wm) = container.get_watch_manager() {
                    if let Ok(mut wm) = wm.write() {
                        wm.resume_with_folders(&existing_folders);
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

    tracing::info!(
        "[VectorValidate] usearch={} ({}), map={} ({})",
        vector_file_exists,
        vector_file.display(),
        map_file_exists,
        map_file.display(),
    );

    if container.is_semantic_available() {
        if let Ok(conn) = db::get_connection(&container.db_path) {
            if let Ok(stats) = db::get_vector_indexing_stats(&conn) {
                tracing::info!(
                    "[VectorValidate] DB: total={}, vector_indexed={}, pending_chunks={}",
                    stats.total_files,
                    stats.vector_indexed_files,
                    stats.pending_chunks
                );
                if stats.vector_indexed_files > 0 && (!vector_file_exists || !map_file_exists) {
                    tracing::warn!(
                        "[VectorValidate] Index file missing → resetting {} files in DB",
                        stats.vector_indexed_files
                    );
                    if let Ok(reset_count) = db::reset_all_vector_indexed(&conn) {
                        tracing::info!(
                            "[VectorValidate] Reset vector_indexed_at for {} files",
                            reset_count
                        );
                    }
                } else if vector_file_exists && map_file_exists {
                    tracing::info!("[VectorValidate] Both files present — no reset needed");
                }
            }
        }
    }
}

// spawn_startup_sync は initialize_app → spawn_startup_sync_async (index.rs) に統合済み。
// lib.rs setup() での二重呼び出しを防止するために削除。

/// 벡터 워커 정리 + 인덱스 저장 + DB 최적화 (종료/트레이 quit 공통)
fn cleanup_vector_resources(container: &AppContainer) {
    // 주기 sync task 중단 신호 (v2.5.2) — 루프가 최대 60초 내 탈출
    container.signal_sync_shutdown();

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
    // DB 최적화: WAL 체크포인트 + 쿼리 플래너 통계 갱신
    cleanup_database(&container.db_path);
}

/// 모델 디렉토리 내 .tmp 잔여 파일 정리 (다운로드 중 크래시 시 생성됨)
fn cleanup_tmp_files(models_dir: &std::path::Path) {
    let mut cleaned = 0usize;
    // models/ 하위 2단계까지 탐색 (e.g., models/kosimcse-roberta-multitask/*.tmp)
    for entry in std::fs::read_dir(models_dir)
        .into_iter()
        .flatten()
        .flatten()
    {
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("tmp") {
            if std::fs::remove_file(&path).is_ok() {
                cleaned += 1;
            }
        } else if path.is_dir() {
            for sub in std::fs::read_dir(&path).into_iter().flatten().flatten() {
                let sub_path = sub.path();
                if sub_path.is_file()
                    && sub_path.extension().and_then(|e| e.to_str()) == Some("tmp")
                    && std::fs::remove_file(&sub_path).is_ok()
                {
                    cleaned += 1;
                }
            }
        }
    }
    if cleaned > 0 {
        tracing::info!("Cleaned up {} stale .tmp model file(s)", cleaned);
    }
}

/// 앱 종료 시 DB 정리: 풀 drain → WAL 체크포인트 + PRAGMA optimize
fn cleanup_database(db_path: &std::path::Path) {
    // 풀의 모든 커넥션을 먼저 닫아야 WAL 체크포인트가 완전히 적용됨
    // (풀 커넥션이 WAL read lock을 보유하면 TRUNCATE 모드 체크포인트 실패)
    crate::db::pool::drain_pool();

    if let Ok(conn) = crate::db::get_connection(db_path) {
        match conn.execute_batch(
            "PRAGMA wal_checkpoint(TRUNCATE);
             PRAGMA optimize;
             PRAGMA incremental_vacuum;",
        ) {
            Ok(_) => tracing::info!(
                "DB cleanup completed (WAL checkpoint + optimize + incremental vacuum)"
            ),
            Err(e) => tracing::warn!("DB cleanup partial failure: {}", e),
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

        // 파서 라이브러리의 알려진 패닉은 catch_unwind로 처리됨 → crash.log 오염 방지.
        // 해당 파일은 에러로 스킵되고 앱은 정상 동작하므로 crash 기록 불필요.
        const BENIGN_PANIC_SOURCES: &[&str] = &[
            "pdf-extract",
            "lopdf",
            "quick-xml",
            "calamine",
            "zip-",    // zip-2.x, zip-rs 등
            "ort",     // ONNX Runtime 내부 panic (세션 초기화 / DLL 로드)
            "usearch", // 벡터 인덱스 C++ 바인딩 panic (reserve / add 중)
            "lindera", // 형태소 사전 로드 panic (embedded ko-dic 압축 해제)
        ];
        if BENIGN_PANIC_SOURCES
            .iter()
            .any(|src| location.contains(src))
        {
            return;
        }

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

        // Telegram 자동 전송 (빌드 시 토큰 주입됐을 때만; 사용자 설정 체크는 생략 —
        // panic 은 치명적이므로 best-effort 로 알림).
        crate::commands::telemetry::report_panic_sync(&location, &message);

        // 긴급 로그 flush — 날짜 기반 로테이션 (최대 3개 파일 유지)
        if let Some(data_dir) = dirs::data_dir() {
            let crash_dir = data_dir.join("com.anything.app");
            let _ = std::fs::create_dir_all(&crash_dir);

            // 날짜별 crash log 파일
            let today = chrono::Local::now().format("%Y-%m-%d");
            let crash_log = crash_dir.join(format!("crash-{}.log", today));

            // 오래된 crash log 정리 (최대 3개 유지)
            const MAX_CRASH_LOGS: usize = 3;
            if let Ok(entries) = std::fs::read_dir(&crash_dir) {
                let mut crash_files: Vec<_> = entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_name().to_string_lossy().starts_with("crash-"))
                    .collect();
                crash_files.sort_by_key(|e| std::cmp::Reverse(e.file_name()));
                for old_file in crash_files.into_iter().skip(MAX_CRASH_LOGS) {
                    let _ = std::fs::remove_file(old_file.path());
                }
            }

            // 단일 파일 크기 제한 (1MB)
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
                let _ = file.sync_all(); // 전원 차단 시 유실 방지
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
        // 자동 업데이트 (GitHub Releases + ed25519 서명 검증)
        .plugin(tauri_plugin_updater::Builder::new().build())
        // relaunch() 지원 — updater가 설치 완료 후 앱 재시작
        .plugin(tauri_plugin_process::init())
        // tauri-plugin-fs: 프론트엔드에서 미사용 (capabilities 미부여)
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
            // 로깅 초기화를 위해 먼저 시도하되, 실패해도 콘솔 로깅은 확보
            let app_data_dir = match app.path().app_data_dir() {
                Ok(dir) => {
                    std::fs::create_dir_all(&dir)
                        .map_err(|e| format!("Failed to create app data dir: {}", e))?;
                    // 로깅 초기화 (콘솔 + 파일)
                    init_logging(Some(&dir));
                    dir
                }
                Err(e) => {
                    // app_data_dir 실패 시 콘솔 전용 로깅으로 fallback
                    init_logging(None);
                    tracing::error!("Failed to get app data dir: {}", e);
                    return Err(format!("Failed to get app data dir: {}", e).into());
                }
            };

            // Create models directory
            let models_dir = app_data_dir.join("models");
            std::fs::create_dir_all(&models_dir).ok();

            // 이전 다운로드 중 크래시로 남은 .tmp 파일 정리
            cleanup_tmp_files(&models_dir);

            // 번들 모델 적용: ONNX Runtime DLL + PaddleOCR 3종을 MSI 리소스에서 APPDATA/models/ 로 복사.
            // 이미 같은 해시면 skip, 다르면 덮어쓰기. 실패해도 다운로드 fallback 으로 자연 진행.
            // 회사망/방화벽 등으로 huggingface·github 차단된 환경에서도 첫 실행 즉시 OCR/시맨틱 가능.
            if let Ok(resource_dir) = app.path().resource_dir() {
                model_downloader::seed_bundled_models(&resource_dir, &models_dir);
            }

            // ONNX Runtime DLL 선제 준비 (sync, 14MB).
            // ort 2.x 는 DLL 버전 불일치 시 ort::init 단계에서 panic 을 일으키므로
            // OCR/Embedder 가 처음 DLL 을 건드리기 전에 SHA-256 검증으로 구버전을 강제 교체한다.
            // 검증만 하면 수백 ms, 다운로드가 필요하면 수초. 실패해도 앱 자체는 부팅시킨다
            // (시맨틱/OCR 기능이 비활성될 뿐 키워드 검색은 동작).
            if let Err(e) = model_downloader::ensure_onnx_runtime_dll(&models_dir) {
                tracing::error!(
                    "ONNX Runtime DLL 준비 실패: {}. 시맨틱/OCR 기능이 비활성됩니다.",
                    e
                );
            }

            // ORT_DYLIB_PATH 설정: 단일 스레드(setup) 시점에서 환경변수 설정
            // container.rs OnceCell 내부(멀티스레드 가능)에서 호출하던 것을 여기로 이동
            // SAFETY: setup()은 main 스레드에서 실행되며, ort 라이브러리 초기화 전임.
            // Rust 1.81+ deprecated이나 프로세스 초기화 시점이므로 안전함.
            {
                let dll_path = models_dir
                    .join("kosimcse-roberta-multitask")
                    .join("onnxruntime.dll");
                unsafe { std::env::set_var("ORT_DYLIB_PATH", &dll_path) };
                tracing::info!("ORT_DYLIB_PATH set to {:?}", dll_path);
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

            // DB 무결성 검사 — 대용량 DB에서 수십 초 걸릴 수 있어 백그라운드로 실행.
            // 시작 시간을 차단하지 않고, 문제 감지 시 이벤트로 프론트엔드에 경고한다.
            {
                let db_path_for_check = container.db_path.clone();
                let app_for_check = app.handle().clone();
                std::thread::spawn(move || {
                    let conn = match db::get_connection(&db_path_for_check) {
                        Ok(c) => c,
                        Err(e) => {
                            tracing::error!("DB integrity check: connection failed: {}", e);
                            return;
                        }
                    };
                    match conn.query_row("PRAGMA quick_check", [], |row| row.get::<_, String>(0)) {
                        Ok(result) if result == "ok" => {
                            tracing::info!("DB integrity check passed");
                        }
                        Ok(result) => {
                            tracing::error!("DB integrity check failed: {}", result);
                            let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)");
                            tracing::warn!("Attempted WAL recovery after integrity check failure");
                            let _ = app_for_check.emit("db-integrity-warning", "데이터베이스 무결성 검사에 실패했습니다. 데이터가 손상되었을 수 있습니다.");
                        }
                        Err(e) => {
                            tracing::error!("DB integrity check error: {}", e);
                            let _ = app_for_check.emit("db-integrity-warning", format!("데이터베이스 검사 오류: {}", e));
                        }
                    }
                });
            }

            tracing::info!("DocuFinder initialized. DB: {:?}", container.db_path);

            // 이전 세션에서 남긴 미전송 crash log 를 Telegram 으로 지연 전송
            // (네이티브 크래시/OOM kill 등 panic hook 이 실행되지 못한 경우 대비)
            commands::telemetry::spawn_flush_pending_crash_logs();

            // Check semantic search availability
            if container.is_semantic_available() {
                tracing::info!("Semantic search: enabled");
                // VectorIndex를 즉시 init하여 WatchManager의 OnceCell 공유 값을
                // pre-populate. 이렇게 해야 사용자가 검색을 한 번도 하지 않은
                // 상태에서도 파일 삭제/수정 이벤트에 벡터가 정리된다 (orphan 방지).
                // 주의: Embedder는 ONNX 로드가 무거워 lazy 유지. VectorIndex는
                // usearch mmap view라 저렴함.
                if let Err(e) = container.get_vector_index() {
                    tracing::warn!("VectorIndex pre-init 실패: {}", e);
                }
            } else {
                tracing::warn!(
                    "Semantic search: disabled (model not found at {:?})",
                    container.models_dir.join("kosimcse-roberta-multitask")
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

            // watcher가 자동 트리거한 벡터 인덱싱도 완료 시 watcher를 정상 재개해야 한다.
            {
                let app_handle = app.handle().clone();
                container.set_vector_progress_callback(Arc::new(move |progress| {
                    let _ = app_handle.emit("vector-indexing-progress", &progress);
                    if progress.is_complete {
                        if let Some(container_state) =
                            app_handle.try_state::<RwLock<AppContainer>>()
                        {
                            if let Ok(container) = container_state.read() {
                                resume_watchers(&container);
                            }
                        }
                    }
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

            // 🔄 주기 sync task 시작 (v2.5.2) — watcher 이벤트 누락 보완.
            // lib.rs setup 에서 1회만 spawn. AtomicBool shutdown 신호는
            // cleanup_vector_resources / 앱 종료 시 세팅되어 루프가 탈출한다.
            indexer::periodic_sync::spawn_periodic_sync_task(app.handle().clone());

            // 미완료 벡터 인덱싱 + startup sync 모두 initialize_app에서 처리.
            // (면책 동의 후 프론트엔드 호출 → spawn_startup_sync_async)
            // lib.rs에서 spawn_startup_sync를 별도로 호출하면
            // initialize_app의 spawn_startup_sync_async와 동시 실행되어
            // 같은 폴더에 대해 reindex가 2번 동시에 발생 → SQLITE_BUSY + 데이터 중복.

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
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    let app_data_dir = window
                        .app_handle()
                        .path()
                        .app_data_dir()
                        .unwrap_or_default();
                    let settings =
                        commands::settings::get_settings_sync(&app_data_dir);
                    if settings.close_to_tray {
                        api.prevent_close();
                        let _ = window.hide();
                        tracing::debug!("Window hidden to tray");
                    } else {
                        tracing::info!("Window closing (close_to_tray=false)");
                        // 트레이 아이콘이 프로세스를 유지시키므로 명시적 종료 필요
                        let app = window.app_handle().clone();
                        if let Some(container) = app.try_state::<RwLock<AppContainer>>() {
                            if let Ok(container) = container.read() {
                                cleanup_vector_resources(&container);
                            }
                        }
                        app.exit(0);
                    }
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
            commands::index::initialize_app,
            commands::index::start_indexing_batch,
            commands::index::get_indexing_batch,
            commands::index::cancel_indexing_batch,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::file::open_file,
            commands::file::open_url,
            commands::file::open_folder,
            commands::file::log_frontend_error,
            commands::file::get_log_dir,
            commands::file::open_log_dir,
            commands::system::get_suggested_folders,
            commands::preview::load_document_preview,
            commands::preview::load_markdown_preview,
            commands::preview::add_bookmark,
            commands::preview::remove_bookmark,
            commands::preview::update_bookmark_note,
            commands::preview::get_bookmarks,
            commands::preview::generate_summary,
            commands::export::export_csv,
            commands::export::export_markdown,
            commands::search::get_search_history_stats,
            commands::duplicate::find_duplicates,
            commands::lineage::rebuild_lineage,
            commands::lineage::get_lineage_versions,
            commands::lineage::get_lineage_health,
            commands::lineage::get_lineage_diff,
            commands::maintenance::prune_missing_files,
            commands::tags::add_file_tag,
            commands::tags::remove_file_tag,
            commands::tags::get_file_tags,
            commands::tags::get_all_tags,
            commands::tags::get_files_by_tag,
            commands::typo::suggest_correction,
            commands::ai::ask_ai,
            commands::ai::ask_ai_file,
            commands::ai::summarize_ai,
            commands::telemetry::report_error,
            commands::formula::get_formula_models_status,
            commands::formula::download_formula_models,
            indexer::periodic_sync::trigger_sync_if_stale,
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
