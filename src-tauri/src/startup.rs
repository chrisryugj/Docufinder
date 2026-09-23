//! 앱 시작(setup) 단계 작업: 로깅 초기화, 모델 준비, 백그라운드 점검·캐시 로드, 시스템 트레이.

use crate::db;
#[cfg(feature = "online")]
use crate::model_downloader;
use crate::parsers;
use crate::shutdown::graceful_shutdown;
use crate::AppContainer;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

pub(crate) fn init_logging(app_data_dir: Option<&PathBuf>) {
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
#[cfg(feature = "online")]
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
#[cfg(feature = "online")]
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
pub(crate) fn resume_watchers(container: &AppContainer) {
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
///
/// get_vector_indexing_stats 의 chunks JOIN files COUNT 풀스캔이 HDD 대용량 DB에서
/// 수 초 걸릴 수 있어 setup() 에서는 백그라운드 스레드로 호출한다 (quick_check 와 동일 패턴).
/// 벡터 검색은 lazy 초기화라 검증/복구(reset_all_vector_indexed)가 수 초 늦게 끝나도 무해.
fn validate_vector_index(
    vector_index_path: &std::path::Path,
    db_path: &std::path::Path,
    semantic_available: bool,
) {
    let vector_file = vector_index_path;
    let map_file = vector_index_path.with_extension("map");
    let vector_file_exists = vector_file.exists();
    let map_file_exists = map_file.exists();

    tracing::info!(
        "[VectorValidate] usearch={} ({}), map={} ({})",
        vector_file_exists,
        vector_file.display(),
        map_file_exists,
        map_file.display(),
    );

    if semantic_available {
        if let Ok(conn) = db::get_connection(db_path) {
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

/// 모델 디렉토리 내 .tmp 잔여 파일 정리 (다운로드 중 크래시 시 생성됨)
#[cfg(feature = "online")]
pub(crate) fn cleanup_tmp_files(models_dir: &std::path::Path) {
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

// macOS: ad-hoc 서명 + dmg 다운로드 시 .app 내부 sub-binary(node, *.node, dylib)에
// `com.apple.quarantine` xattr 가 상속되어 spawn 시 Gatekeeper 가 차단 → kordoc CLI
// 가 실행 안 됨 → HWP5 파싱 전수 실패(이슈 #22). 사용자가 직접 `xattr -dr` 하기 전엔
// 발현되므로 startup 1회로 자동 제거한다. xattr 실행 자체는 quarantine 영향 안 받음.
#[cfg(target_os = "macos")]
pub(crate) fn remove_sidecar_quarantine(resource_dir: &Option<PathBuf>) {
    if let Some(resource_dir) = resource_dir.as_ref() {
        let sidecar_root = resource_dir.join("resources");
        if sidecar_root.exists() {
            match std::process::Command::new("/usr/bin/xattr")
                .args(["-rd", "com.apple.quarantine"])
                .arg(&sidecar_root)
                .output()
            {
                Ok(out) if out.status.success() => {
                    tracing::info!("macOS 사이드카 quarantine 제거: {}", sidecar_root.display());
                }
                Ok(out) => {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    tracing::warn!("xattr 종료코드 {}: {}", out.status, stderr.trim());
                }
                Err(e) => tracing::warn!("xattr 실행 실패: {}", e),
            }
        }
    }
}

// 번들 모델 seed + ONNX Runtime DLL 검증 + 모델 자동 다운로드 — 백그라운드 실행.
// 기존에는 setup() 동기 실행이라 ~43MB SHA-256 해싱(DLL+OCR 3종)이 콜드 스타트
// (AV 상주 PC)에서 첫 창 표시를 1초+ 지연시켰다. 기존 실행 순서(seed → DLL 검증 →
// 모델/OCR 다운로드 체크)는 같은 스레드에서 순차 실행으로 그대로 유지 — 번들 적용
// 전에 다운로드 체크가 돌아 같은 파일을 동시에 쓰는 레이스를 막는다.
// Embedder/OCR 는 lazy(OnceCell) 초기화로 첫 사용 시점(빠르면 initialize_app 1초 후
// startup sync)이 DLL 검증 완료(통상 수백 ms)보다 늦어 ort panic 방지 목적은 유지된다.
//
// lite(내부망) 빌드에는 이 스레드가 통째로 없다. seed 는 "실행 중인 프로세스가
// AppData 에 PE(onnxruntime.dll·pdfium.dll)를 쓰고 그걸 로드"하는 상관을 만들어
// ZombieZERO 계열이 dropper 로 격리했던 바로 그 동작이고(이슈 #35), 나머지는 전부
// 런타임 다운로드다. lite 는 두 기능(시맨틱·OCR)을 제공하지 않으므로 손실이 없다.
#[cfg(feature = "online")]
pub(crate) fn spawn_model_preparation(
    app_handle_bg: tauri::AppHandle,
    models_dir_bg: PathBuf,
    resource_dir: Option<PathBuf>,
    semantic_enabled: bool,
    ocr_enabled: bool,
) {
    std::thread::spawn(move || {
        // 번들 모델 적용: ONNX Runtime DLL + PaddleOCR 3종을 MSI 리소스에서
        // APPDATA/models/ 로 복사. 이미 같은 해시면 skip, 다르면 덮어쓰기.
        // 실패해도 다운로드 fallback 으로 자연 진행. 회사망/방화벽 등으로
        // huggingface·github 차단된 환경에서도 첫 실행 즉시 OCR/시맨틱 가능.
        if let Some(resource_dir) = resource_dir {
            model_downloader::seed_bundled_models(&resource_dir, &models_dir_bg);
        }

        // ONNX Runtime DLL 선제 준비 (14MB).
        // ort 2.x 는 DLL 버전 불일치 시 ort::init 단계에서 panic 을 일으키므로
        // OCR/Embedder 가 처음 DLL 을 건드리기 전에 SHA-256 검증으로 구버전을 강제 교체한다.
        // 검증만 하면 수백 ms, 다운로드가 필요하면 수초. 실패해도 앱 자체는 부팅시킨다
        // (시맨틱/OCR 기능이 비활성될 뿐 키워드 검색은 동작).
        if let Err(e) = model_downloader::ensure_onnx_runtime_dll(&models_dir_bg) {
            tracing::error!(
                "ONNX Runtime DLL 준비 실패: {}. 시맨틱/OCR 기능이 비활성됩니다.",
                e
            );
        }

        // 모델 자동 다운로드 — seed/DLL 검증 후에 존재 여부를 검사해야
        // 번들로 채워진 파일을 재다운로드하거나 동시에 쓰지 않는다.
        maybe_download_models(
            app_handle_bg.clone(),
            models_dir_bg.clone(),
            semantic_enabled,
        );

        // pdfium 준비 (스캔/이미지 PDF 페이지 래스터화 fallback) — OCR 활성 시에만.
        // best-effort: 실패해도 born-digital/JPEG 스캔 경로는 그대로 동작한다.
        if ocr_enabled {
            if let Err(e) = model_downloader::ensure_pdfium(&models_dir_bg) {
                tracing::warn!("pdfium 준비 실패 (스캔 PDF 래스터화 OCR 비활성): {}", e);
            }
        }

        maybe_download_ocr_models(app_handle_bg.clone(), models_dir_bg, ocr_enabled);

        // OCR 엔진 워밍업 — **반드시 seed 이후**. seed_one 은 해시가 다르면
        // dict.txt 를 지웠다가 다시 복사하는데, 그 창에 OcrEngine::new 가 읽으면
        // "Dictionary not found" 로 실패하고 그 세션은 OCR 이 죽는다(이슈 #35).
        // AppContainer 는 setup 본류에서 manage 되므로 잠깐 기다렸다 잡는다.
        if ocr_enabled || semantic_enabled {
            for _ in 0..50 {
                if let Some(state) = app_handle_bg.try_state::<RwLock<AppContainer>>() {
                    if let Ok(container) = state.read() {
                        if ocr_enabled {
                            container.spawn_ocr_warmup();
                        }
                        // 검색 경로는 더 이상 임베더를 블로킹 초기화하지 않으므로
                        // (#44) 시맨틱이 켜진 세션은 부팅 때 미리 준비해 첫
                        // 시맨틱 검색부터 동작하게 한다.
                        if semantic_enabled {
                            container.spawn_embedder_warmup();
                        }
                    }
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    });
}

/// 시작 단계 경고 (DB 무결성 등). 검사가 프론트 리스너 등록보다 먼저 끝나면 emit 이 유실되므로
/// 여기 보관해 두고 프론트가 마운트 때 `get_startup_warnings` 로 가져간다(늦게 끝나면 emit 으로 받는다).
static STARTUP_WARNINGS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

pub(crate) fn startup_warnings() -> Vec<String> {
    STARTUP_WARNINGS
        .lock()
        .map(|w| w.clone())
        .unwrap_or_default()
}

fn push_startup_warning(app: &tauri::AppHandle, message: String) {
    if let Ok(mut w) = STARTUP_WARNINGS.lock() {
        w.push(message.clone());
    }
    let _ = app.emit("db-integrity-warning", message);
}

// DB 무결성 검사 — 대용량 DB에서 수십 초 걸릴 수 있어 백그라운드로 실행.
// 시작 시간을 차단하지 않고, 문제 감지 시 프론트엔드에 경고한다.
pub(crate) fn spawn_db_integrity_check(app: &tauri::App, container: &AppContainer) {
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
                push_startup_warning(
                    &app_for_check,
                    "데이터베이스 무결성 검사에 실패했습니다. 데이터가 손상되었을 수 있습니다."
                        .to_string(),
                );
            }
            Err(e) => {
                tracing::error!("DB integrity check error: {}", e);
                push_startup_warning(&app_for_check, format!("데이터베이스 검사 오류: {}", e));
            }
        }
    });
}

// kordoc 사이드카 가용성 진단 — HWP5 는 Rust fallback 이 없어 kordoc 미가용 시 전수 실패한다.
// 미가용이면 frontend 에 즉시 알려 인덱싱 시작 전에 사용자가 인지할 수 있게 한다 (이슈 #22).
pub(crate) fn report_kordoc_availability(app: &tauri::App) {
    let kordoc_ok = parsers::kordoc::is_available();
    if kordoc_ok {
        tracing::info!("kordoc 사이드카 가용 — hwp/hwpx/docx/pdf 변환 활성");
    } else {
        tracing::error!(
            "kordoc 사이드카 미가용 — HWP 파일 인덱싱 불가. \
             번들 node / kordoc CLI 가 .app 내부에 누락되었거나 실행 권한이 없습니다."
        );
    }
    let _ = app.handle().emit("kordoc-availability", kordoc_ok);
}

/// 컨테이너 인덱싱 콜백 등록: 증분 인덱싱 알림, 벡터 인덱싱 진행률(완료 시 감시 재개).
pub(crate) fn set_indexing_callbacks(app: &tauri::App, container: &AppContainer) {
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
                if let Some(container_state) = app_handle.try_state::<RwLock<AppContainer>>() {
                    if let Ok(container) = container_state.read() {
                        resume_watchers(&container);
                    }
                }
            }
        }));
    }
}

// ⚡ 디스크 타입 사전 감지 (C:, D: — PowerShell 호출 1-3초를 앱 시작 시 흡수)
pub(crate) fn spawn_disk_type_detection() {
    tauri::async_runtime::spawn(async {
        tokio::task::spawn_blocking(|| {
            for letter in ['C', 'D', 'E'] {
                let path = format!("{}:\\", letter);
                if std::path::Path::new(&path).exists() {
                    let _ = crate::utils::disk_info::detect_disk_type(std::path::Path::new(&path));
                }
            }
            tracing::debug!("Disk type pre-detection completed");
        })
        .await
        .ok();
    });
}

// ⚡ 파일명 캐시 로드 (Everything 스타일 빠른 검색) + 벡터 인덱스 ↔ DB 정합성 검증
// — 백그라운드 실행. 캐시 DB 전체 SELECT 는 HDD 5-10초(filename_cache.rs 주석),
// 정합성 검증은 chunks JOIN files COUNT 풀스캔 2회로 역시 수 초 걸릴 수 있어
// setup() 동기 실행 시 첫 창 표시를 그만큼 지연시킨다. 캐시 로드 완료 전 파일명
// 검색은 기존 DB LIKE 폴백이 처리하고(search_service/keyword.rs use_cache 게이트:
// !is_empty && !is_truncated), 두 작업은 기존 순서대로 같은 스레드에서 순차 실행해
// HDD 디스크 경합을 피한다. container 는 아래 app.manage 로 move 되므로
// Arc/PathBuf 만 복제해 넘긴다.
pub(crate) fn spawn_filename_cache_load(container: &AppContainer) {
    let filename_cache = container.get_filename_cache();
    let db_path = container.db_path.clone();
    let vector_index_path = container.vector_index_path.clone();
    let semantic_available = container.is_semantic_available();
    std::thread::spawn(move || {
        match db::get_connection(&db_path) {
            Ok(conn) => match filename_cache.load_from_db(&conn) {
                Ok(count) => {
                    tracing::info!("FilenameCache loaded: {} files", count);
                }
                Err(e) => {
                    tracing::warn!("Failed to load filename cache: {}", e);
                }
            },
            Err(e) => {
                tracing::warn!("Failed to load filename cache: {}", e);
            }
        }

        validate_vector_index(&vector_index_path, &db_path, semantic_available);
    });
}

/// 감시 폴더 자동 복원을 백그라운드 스레드에서 실행한다 (app.manage 이후 호출).
pub(crate) fn spawn_resume_watchers(app: &tauri::App) {
    let app_handle = app.handle().clone();
    std::thread::spawn(move || {
        if let Some(container_state) = app_handle.try_state::<RwLock<AppContainer>>() {
            if let Ok(container) = container_state.read() {
                resume_watchers(&container);
            }
        }
    });
}

// 시스템 트레이 설정
pub(crate) fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
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
                    tracing::warn!("Failed to load tray icon: {e}, falling back to default");
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
                graceful_shutdown(app);
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
    Ok(())
}
