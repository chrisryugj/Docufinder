mod application; // 클린 아키텍처: Application Layer
pub mod breadcrumb; // 현재 처리 중 파일/단계 추적 (panic hook 에서 읽음)
mod commands;
mod constants;
mod crash; // 패닉 훅, 시작 실패 crash log
mod db;
mod embedder;
mod error;
mod indexer;
// LLM 클라이언트 · 모델 자동 다운로드 — 둘 다 런타임 네트워크 경로라 lite 빌드에선 통째로 뺀다.
#[cfg(feature = "online")]
mod llm; // LLM 클라이언트 (RAG + AI 요약)
#[cfg(feature = "online")]
mod model_downloader; // 모델 자동 다운로드
pub mod ocr; // PaddleOCR ONNX 기반 OCR 엔진
pub mod panic_filter; // crash.log BENIGN 필터 (panic hook + deferred flush 공유)
pub mod parsers;
mod search;
mod shutdown; // 종료 절차 (벡터 워커 정리, DB 정리)
mod startup; // setup 단계 초기화 (로깅, 모델 준비, 백그라운드 작업, 트레이)
mod tokenizer; // 한국어 형태소 분석 (Phase 5)
mod utils; // 유틸리티 (idle_detector, disk_info)

#[cfg(test)]
mod perf_bench; // 성능 계측 하니스 (릴리스 미포함, cargo test --release perf_)

// Windows: fixed-version WebView2 Runtime detection + environment injection
// (이슈 #24 LTSC 1809 + admin 없음 + GPO 차단 환경 대응)
#[cfg(target_os = "windows")]
mod webview2_runtime;

pub use application::container::AppContainer;
pub use error::{ApiError, ApiResult};

/// `tauri.conf.json` 의 `identifier` 와 반드시 같아야 하는 값.
///
/// panic hook 은 Tauri 앱이 만들어지기 전에 설치되므로 `app.path().app_data_dir()` 를 쓸 수
/// 없고 `dirs::data_dir()` 아래 경로를 직접 조립한다. lite 빌드는 일반 배포판과 설정/DB 를
/// 섞지 않으려고 별도 identifier 를 쓰므로(그래야 lite 의 기능 강제 off 가 일반 설치본의
/// 설정을 덮어쓰지 않는다) 여기서도 갈라줘야 크래시 로그가 엉뚱한 폴더로 가지 않는다.
#[cfg(feature = "online")]
pub const APP_IDENTIFIER: &str = "com.anything.app";
#[cfg(not(feature = "online"))]
pub const APP_IDENTIFIER: &str = "com.anything.lite";

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

use tauri::Manager;
use tauri_plugin_autostart::MacosLauncher;

use shutdown::{cleanup_vector_resources, graceful_shutdown};
#[cfg(feature = "online")]
use startup::cleanup_tmp_files;
use startup::init_logging;

/// 로깅 초기화 (파일 + 콘솔)
///
/// app_data_dir이 Some이면 파일 로깅도 활성화.
/// None이면 콘솔만 (app_data_dir 확보 실패 시 fallback).
/// 네이티브 드래그아웃 프리뷰 아이콘 경로 — 내장 PNG 를 앱 캐시에 1회 기록하고 절대경로 반환.
/// @crabnebula/tauri-plugin-drag 의 startDrag(icon) 는 디스크상 실존 이미지 경로를 요구한다.
#[tauri::command]
fn drag_preview_icon(app: tauri::AppHandle) -> Result<String, String> {
    use std::io::Write;
    let dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| format!("cache dir: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("create cache dir: {e}"))?;
    let path = dir.join("drag-preview.png");
    if !path.exists() {
        let bytes = include_bytes!("../icons/32x32.png");
        std::fs::File::create(&path)
            .and_then(|mut f| f.write_all(bytes))
            .map_err(|e| format!("write drag icon: {e}"))?;
    }
    Ok(path.to_string_lossy().into_owned())
}

// spawn_startup_sync は initialize_app → spawn_startup_sync_async (index.rs) に統合済み。
// lib.rs setup() での二重呼び出しを防止するために削除。

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 크래시 핸들러 설정 (패닉 발생 시 로그 기록)
    crash::install_panic_hook();

    // tokenizers 병렬 처리 비활성화 (rayon과의 데드락 방지)
    // SAFETY: run() 진입 직후, main 스레드만 존재하는 단일 스레드 컨텍스트.
    // tauri::Builder 생성 전이므로 다른 스레드가 환경변수를 읽을 수 없음.
    // Rust 1.81+ deprecated이나 프로세스 초기화 시점이므로 안전함.
    unsafe { std::env::set_var("TOKENIZERS_PARALLELISM", "false") };

    // visible: false → page load 완료 후 창 표시 (검정화면 방지)
    // Dev mode: WebView2 SmartScreen 비활성화는 package.json tauri:dev 스크립트에서 설정
    let show_on_load = Arc::new(AtomicBool::new(true));
    let show_on_load_flag = show_on_load.clone();

    let builder = tauri::Builder::default()
        // 싱글 인스턴스: 중복 실행 시 기존 창 포커스 (가장 먼저 등록해야 함)
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init());

    // 자동 업데이트 — lite(내부망) 빌드는 등록하지 않는다.
    // `dialog:false` + `installMode:passive` 로 서명 없는 설치본을 받아 조용히 실행하는 흐름은
    // 보안솔루션이 dropper 휴리스틱으로 잡는 대표 동작이고, 폐쇄망에서는 6시간마다 실패하는
    // 아웃바운드 시도가 IDS 로그에 반복 알람으로 쌓이기만 한다.
    #[cfg(feature = "online")]
    let builder = builder
        // 자동 업데이트 (GitHub Releases + ed25519 서명 검증)
        .plugin(tauri_plugin_updater::Builder::new().build())
        // relaunch() 지원 — updater가 설치 완료 후 앱 재시작
        .plugin(tauri_plugin_process::init());

    builder
        // 네이티브 드래그아웃 — 검색 결과 파일을 다른 앱/웹페이지로 끌어다 놓기
        .plugin(tauri_plugin_drag::init())
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

            // [v2.6.3] main window 직접 build.
            // conf.json 의 main window 는 `create:false` 라 Tauri 가 자동 생성하지 않는다.
            // Windows 에서 `<exe_dir>/EBWebView` 또는 `<exe_dir>/*/EBWebView` 가 발견되면
            // 그 경로로 `ICoreWebView2Environment` 를 만들어 `with_environment` 로 주입한다.
            // registry 에 WebView2 가 등록 안 된 환경(LTSC 1809 / GPO 차단 / 다른 사용자 계정에만
            // 설치된 케이스 — 이슈 #24)에서도 EBWebView 폴더만 풀어두면 동작.
            {
                let main_config = app
                    .config()
                    .app
                    .windows
                    .iter()
                    .find(|w| w.label == "main")
                    .cloned()
                    .ok_or_else(|| {
                        "main window config missing in tauri.conf.json".to_string()
                    })?;

                #[allow(unused_mut)]
                let mut builder = tauri::WebviewWindowBuilder::from_config(
                    app.handle(),
                    &main_config,
                )
                .map_err(|e| format!("WebviewWindowBuilder::from_config failed: {e}"))?;

                // Windows: LTSC installer 에 fixed-runtime 폴더가 있어도 시스템
                // WebView2 Runtime 이 정상 등록된 PC에서는 시스템 런타임을 우선 사용한다.
                // 집 PC에서 v2.6.1/2.6.2 가 됐다가 LTSC 설치본에서만 실패한 이유가 이
                // 차이다. 시스템 런타임이 없거나 강제 플래그가 있을 때만 직접 만든
                // ICoreWebView2Environment 를 with_environment 로 wry 에 주입한다.
                //
                // Tauri 의 `webviewInstallMode:fixedRuntime` 는 프로세스 시작 전
                // WEBVIEW2_BROWSER_EXECUTABLE_FOLDER 를 설정하지만, 설정 경로가
                // webview2-runtime/ 부모 폴더라 현재 CAB 레이아웃의 실제
                // msedgewebview2.exe 위치(EBWebView/x64)와 맞지 않는다. 그래서
                // fixed-runtime fallback 에서는 런타임 경로와 UDF를 직접 지정한다.
                //
                // LTSC installer 의 풀린 위치 `<exe_dir>/webview2-runtime/EBWebView/x64/`
                // 는 detect_fixed_runtime_dir 의 우선순위 1 후보로 등록되어 있다.
                // v2.6.17: Windows 10 + Fixed Version Runtime v120+ 는 renderer 가
                // App Container sandbox 에서 실행되므로 runtime 폴더에 App Container
                // 읽기 권한이 필수다 (이슈 #23). NSIS 가 푼 폴더엔 없으므로 startup
                // 에서 보강한다.
                //
                // v2.6.18: watchdog 다이얼로그 본문을 fixed runtime 감지 여부로 분기.
                // 미감지(일반 설치본 + system WebView2)면 LTSC 설치본 안내가 우선이고,
                // 감지(LTSC 설치본, runtime 정상)인데도 hang 이면 보안 솔루션 차단이
                // 유력하다 — 두 경우를 같은 문구로 묶으면 원인을 오도한다 (이슈 #23/#24).
                #[cfg(target_os = "windows")]
                let webview2_watchdog_body: String = {
                    if let Some(runtime_dir) =
                        crate::webview2_runtime::detect_fixed_runtime_dir()
                    {
                        let force_fixed =
                            std::env::var_os("DOCUFINDER_FORCE_FIXED_WEBVIEW2").is_some();
                        let fixed_version =
                            crate::webview2_runtime::fixed_runtime_version(&runtime_dir);
                        let system_status = if force_fixed {
                            Err("DOCUFINDER_FORCE_FIXED_WEBVIEW2=1".to_string())
                        } else {
                            crate::webview2_runtime::detect_system_runtime_version_ignoring_overrides()
                        };

                        let system_version_to_use = match &system_status {
                            Ok(version)
                                if crate::webview2_runtime::should_prefer_system_runtime(
                                    version,
                                    fixed_version.as_deref(),
                                ) =>
                            {
                                Some(version.clone())
                            }
                            _ => None,
                        };

                        if let Some(version) = system_version_to_use {
                            {
                                crate::webview2_runtime::clear_process_overrides();
                                let override_after =
                                    crate::webview2_runtime::process_override_diagnostics();
                                tracing::info!(
                                    "System WebView2 Runtime available ({version}) and newer than bundled fixed runtime ({}) — using system runtime instead of bundled fixed runtime at {}",
                                    fixed_version.as_deref().unwrap_or("unknown"),
                                    runtime_dir.display(),
                                );
                                tracing::info!(
                                    "WebView2 process override after system runtime selection:\n{override_after}"
                                );

                                format!(
                                    "WebView2 초기화가 응답하지 않습니다.\n\n\
                                     시스템 WebView2 Runtime({version})이 정상 등록되어 있어\n\
                                     LTSC 포함 런타임 대신 시스템 런타임으로 시작했습니다.\n\
                                     그래도 controller 생성이 끝나지 않았으므로 WebView2\n\
                                     프로세스 생성/프로필 쓰기/보안 정책 쪽을 확인해야 합니다.\n\n\
                                     [진단 정보]\nSystem WebView2 Runtime: {version}\n\
                                     Bundled fixed runtime: {}\n\
                                     Process overrides:\n{override_after}\n\n\
                                     위 내용을 캡처해 개발자에게 전달해 주세요. 앱을 종료합니다.",
                                    runtime_dir.display()
                                )
                            }
                        } else {
                            {
                                let system_reason = match system_status {
                                    Ok(version) => format!(
                                        "System WebView2 Runtime available ({version}) but not newer than bundled fixed runtime ({})",
                                        fixed_version.as_deref().unwrap_or("unknown")
                                    ),
                                    Err(reason) => reason,
                                };

                                if force_fixed {
                                    tracing::info!(
                                        "System WebView2 Runtime bypassed ({system_reason}) — using bundled fixed runtime at {}",
                                        runtime_dir.display()
                                    );
                                } else if system_reason.contains("not newer than bundled") {
                                    tracing::info!(
                                        "{system_reason} — using bundled fixed runtime at {}",
                                        runtime_dir.display()
                                    );
                                } else {
                                    tracing::info!(
                                        "System WebView2 Runtime unavailable ({system_reason}) — using bundled fixed runtime at {}",
                                        runtime_dir.display()
                                    );
                                }

                                let webview2_user_data_dir = match app.path().app_local_data_dir() {
                                    Ok(dir) => dir.join("webview2-user-data"),
                                    Err(e) => {
                                        tracing::warn!(
                                            "app_local_data_dir 확보 실패, app_data_dir로 fallback: {e}"
                                        );
                                        app_data_dir.join("webview2-user-data")
                                    }
                                };
                                let udf_ok = crate::webview2_runtime::prepare_user_data_dir(
                                    &webview2_user_data_dir,
                                );
                                builder = builder.data_directory(webview2_user_data_dir.clone());

                                let override_before =
                                    crate::webview2_runtime::process_override_diagnostics();
                                crate::webview2_runtime::force_process_overrides(
                                    &runtime_dir,
                                    &webview2_user_data_dir,
                                );
                                let override_after =
                                    crate::webview2_runtime::process_override_diagnostics();
                                tracing::info!(
                                    "WebView2 process override before:\n{override_before}\nWebView2 process override after:\n{override_after}"
                                );

                                let runtime_acl_ok =
                                    crate::webview2_runtime::grant_app_container_access(
                                        &runtime_dir,
                                    );
                                let runtime_diag =
                                    crate::webview2_runtime::runtime_diagnostics(&runtime_dir);
                                let udf_diag = crate::webview2_runtime::user_data_diagnostics(
                                    &webview2_user_data_dir,
                                );
                                tracing::info!("WebView2 fixed runtime 진단:\n{runtime_diag}");
                                tracing::info!("WebView2 user data dir 진단:\n{udf_diag}");

                                let mut env_injected = false;
                                match crate::webview2_runtime::create_environment(
                                    &runtime_dir,
                                    &webview2_user_data_dir,
                                ) {
                                    Ok(env) => {
                                        tracing::info!(
                                            "WebView2 fixed runtime detected at {} — environment injected",
                                            runtime_dir.display()
                                        );
                                        builder = builder.with_environment(env);
                                        env_injected = true;
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            "WebView2 fixed runtime present at {} but environment creation failed: {}",
                                            runtime_dir.display(),
                                            e
                                        );
                                        if !force_fixed {
                                            crate::webview2_runtime::clear_process_overrides();
                                        }
                                    }
                                }

                                // fixed runtime 정상 감지 — hang 이면 controller 생성 단계에서
                                // msedgewebview2 자식 프로세스 실행, runtime 읽기 또는 UDF
                                // 쓰기가 막힌 상태다.
                                format!(
                                    "WebView2 초기화가 응답하지 않습니다.\n\n\
                                     LTSC 포함 런타임으로 시작했습니다. 시스템 WebView2가\n\
                                     없거나, 포함 런타임보다 새 버전이 아니라서 이 경로를\n\
                                     선택했습니다. 포함 런타임과 User Data Folder 권한을\n\
                                     보강했지만 controller 생성 단계가 끝나지 않았습니다.\n\
                                     Windows Defender 차단 기록, AppLocker, EDR, Controlled\n\
                                     Folder Access 또는 App Container 권한 문제를 확인해야 합니다.\n\
                                     아래 실행 파일을 허용 목록에 추가해 주세요:\n\
                                     - docufinder.exe\n\
                                     - msedgewebview2.exe (WebView2 브라우저 프로세스)\n\n\
                                     [진단 정보]\nSystem WebView2 Runtime: {system_reason}\n\
                                     Fixed environment injected: {env_injected}\n\
                                     Runtime ACL: {}\nUDF ACL: {}\n\
                                     {runtime_diag}\n\n{udf_diag}\n\
                                     Process overrides:\n{override_after}\n\n\
                                     위 내용을 캡처해 개발자에게 전달해 주세요. 앱을 종료합니다.",
                                    if runtime_acl_ok { "OK" } else { "FAILED" },
                                    if udf_ok { "OK" } else { "FAILED" }
                                )
                            }
                        }
                    } else {
                        crate::webview2_runtime::clear_process_overrides();
                        let override_after =
                            crate::webview2_runtime::process_override_diagnostics();
                        tracing::info!(
                            "no fixed-runtime detected near exe — relying on system WebView2 (registry detection)"
                        );
                        // fixed runtime 미감지 = 일반 설치본. system WebView2 의존.
                        // 회사 / 오프라인 PC 면 LTSC 설치본이 맞다.
                        format!(
                            "WebView2 초기화가 응답하지 않습니다.\n\n\
                             이 설치본은 WebView2 런타임을 자체 포함하지 않아 시스템에 깔린\n\
                             WebView2 에 의존합니다. 회사 / 관공서 / 오프라인 PC 에서 화면이\n\
                             안 뜨면, 릴리스 페이지에서 파일명에 'ltsc' 가 붙은 설치본\n\
                             (Anything_x.x.x_x64-ltsc-setup.exe — WebView2 를 자체 포함한\n\
                             오프라인 전용 빌드) 을 받아 다시 설치해 주세요.\n\n\
                             [진단 정보] fixed runtime 미감지 — system WebView2 경로\n\
                             Process overrides:\n{override_after}\n\n\
                             위 내용을 캡처해 개발자에게 전달해 주세요. 앱을 종료합니다."
                        )
                    }
                };

                // builder.build() 는 wry 가 WebView2 controller 를 생성하는 단계로,
                // controller 완료 callback 무한 대기(wait_with_pump)로 hang 할 수 있다
                // (이슈 #23 v2.6.16). watchdog 으로 60초 후 진단 다이얼로그 + 종료.
                #[cfg(target_os = "windows")]
                let build_watchdog = crate::webview2_runtime::spawn_build_watchdog(
                    std::time::Duration::from_secs(60),
                    webview2_watchdog_body,
                );

                let build_result = builder.build();

                #[cfg(target_os = "windows")]
                build_watchdog.disarm();

                build_result.map_err(|e| format!("main window build failed: {e}"))?;
            }

            // Create models directory
            let models_dir = app_data_dir.join("models");
            std::fs::create_dir_all(&models_dir).ok();

            // 이전 다운로드 중 크래시로 남은 .tmp 파일 정리
            #[cfg(feature = "online")]
            cleanup_tmp_files(&models_dir);

            #[cfg_attr(not(feature = "online"), allow(unused_variables))]
            let resource_dir = app.path().resource_dir().ok();

            // sync 유지 — 아래 kordoc::is_available 진단이 quarantine 제거 후에 실행돼야 한다.
            #[cfg(target_os = "macos")]
            startup::remove_sidecar_quarantine(&resource_dir);

            // ORT_DYLIB_PATH 설정: 단일 스레드(setup) 시점에서 환경변수 설정
            // (아래 모델 준비 스레드 spawn 전에 실행 — set_var 단일 스레드 안전 논거 유지)
            // container.rs OnceCell 내부(멀티스레드 가능)에서 호출하던 것을 여기로 이동
            // SAFETY: setup()은 main 스레드에서 실행되며, ort 라이브러리 초기화 전임.
            // Rust 1.81+ deprecated이나 프로세스 초기화 시점이므로 안전함.
            // DLL(onnxruntime·pdfium) 은 **번들(설치본) 경로가 있으면 거기서 직접 로드**한다.
            // AppData 복사본을 로드하면 내부망 EDR(ZombieZERO 등) 이 "이 프로세스가 런타임에 쓴
            // PE 를 로드" 하는 상관을 dropper 로 오탐해 프로세스를 격리한다(이슈 #35). 설치
            // 프로그램이 놓은 서명된 번들 파일에서 로드하면 그 상관이 끊겨 오탐이 사라진다.
            // 번들이 없으면(dev·미번들 플랫폼) 기존 AppData seed/다운로드 경로로 fallback.
            //
            // lite(내부망) 빌드는 onnxruntime/pdfium 을 아예 번들하지 않고 시맨틱·OCR 도
            // 강제 off 라, 두 경로를 설정할 일이 없다. 설정하지 않으면 `ort`/`pdfium-render`
            // 가 dlopen 을 시도하는 순간 자체가 사라져 "런타임 DLL 로드" 신호가 0 이 된다.
            #[cfg(feature = "online")]
            {
                let ort_lib = model_downloader::dylib_filename();
                let bundled_ort = resource_dir
                    .as_ref()
                    .map(|r| r.join("resources").join("onnxruntime").join(ort_lib));
                let dll_path = match bundled_ort {
                    Some(p) if p.exists() => p,
                    _ => models_dir
                        .join("kosimcse-roberta-multitask")
                        .join(ort_lib),
                };
                unsafe { std::env::set_var("ORT_DYLIB_PATH", &dll_path) };
                tracing::info!("ORT_DYLIB_PATH set to {:?}", dll_path);
            }

            // PDFIUM_DYLIB_PATH 설정: 스캔/이미지 PDF 페이지 래스터화 fallback 용 (parsers/pdf.rs).
            // ORT_DYLIB_PATH 와 동일하게 번들 경로 우선, 없으면 models/pdfium/<lib> fallback.
            // 외부에서 이미 명시 설정돼 있으면 그 값을 존중한다. 파일이 없으면 pdf.rs 가 조용히
            // 기능 비활성한다(크래시 없음 — 기존 born-digital/JPEG 스캔 경로는 그대로 동작).
            #[cfg(feature = "online")]
            if std::env::var_os("PDFIUM_DYLIB_PATH").is_none() {
                let pdfium_lib = model_downloader::pdfium_lib_filename();
                let bundled_pdfium = resource_dir
                    .as_ref()
                    .map(|r| r.join("resources").join("pdfium").join(pdfium_lib));
                let pdfium_path = match bundled_pdfium {
                    Some(p) if p.exists() => p,
                    _ => models_dir.join("pdfium").join(pdfium_lib),
                };
                unsafe { std::env::set_var("PDFIUM_DYLIB_PATH", &pdfium_path) };
                tracing::info!("PDFIUM_DYLIB_PATH set to {:?}", pdfium_path);
            }

            let setup_settings = crate::commands::settings::get_settings_sync(&app_data_dir);

            #[cfg(feature = "online")]
            startup::spawn_model_preparation(
                app.handle().clone(),
                models_dir.clone(),
                resource_dir,
                setup_settings.semantic_search_enabled,
                setup_settings.ocr_enabled,
            );

            // Initialize database with AppContainer
            let container = AppContainer::new(&app_data_dir);
            db::init_database(&container.db_path)
                .map_err(|e| format!("Failed to initialize database: {}", e))?;

            startup::spawn_db_integrity_check(app, &container);

            tracing::info!("DocuFinder initialized. DB: {:?}", container.db_path);

            // 이슈 #29: 기존에 매핑 드라이브(`Y:\`)로 등록된 감시 폴더·파일 경로를 UNC
            // (`\\server\share`)로 마이그레이션. UAC elevated 실행 시 일반 세션의 매핑
            // 드라이브가 안 보여 sync/재인덱싱이 os error 5 로 막히던 것을 자동 치유한다.
            // 드라이브 매핑이 살아 있어야 resolve 되므로 매 시작마다 best-effort 재시도(멱등).
            #[cfg(windows)]
            if let Ok(conn) = db::get_connection(&container.db_path) {
                if let Ok(folders) = db::get_watched_folders(&conn) {
                    let mut seen = std::collections::HashSet::new();
                    for f in &folders {
                        let b = f.as_bytes();
                        if b.len() < 2 || b[1] != b':' || !b[0].is_ascii_alphabetic() {
                            continue; // 드라이브 경로 아님(UNC/posix)
                        }
                        let letter = (b[0] as char).to_ascii_uppercase();
                        if !seen.insert(letter) {
                            continue; // 드라이브당 1회
                        }
                        let root = format!("{letter}:\\");
                        if let Some(unc) = crate::utils::network_path::resolve_mapped_drive_to_unc(
                            std::path::Path::new(&root),
                        ) {
                            let base = unc.to_string_lossy();
                            match db::remap_drive_prefix(&conn, letter, &base) {
                                Ok((nf, nd)) if nf + nd > 0 => tracing::info!(
                                    "[#29] {letter}:\\ → {base} 마이그레이션: files {nf}, folders {nd}"
                                ),
                                Ok(_) => {}
                                Err(e) => tracing::warn!("[#29] {letter}: 마이그레이션 실패: {e}"),
                            }
                        }
                    }
                }

                // 이슈 #46: v3.8.5 까지 UNC 폴더가 `\\?\UNC\srv\share\…` verbatim 형태로 저장돼
                // 프론트의 `\\?\` 제거가 `UNC\srv\…` 를 만들었다. 저장 형식을 `\\srv\share\…` 로
                // 복원한다(멱등, 매핑 정보 불필요). #29 치환 뒤에 돌려야 `Y:\` → UNC 로 바뀐 행도 포함된다.
                match db::remap_unc_verbatim_prefix(&conn) {
                    Ok((nf, nd)) if nf + nd > 0 => tracing::info!(
                        "[#46] \\\\?\\UNC\\ → \\\\ 마이그레이션: files {nf}, folders {nd}"
                    ),
                    Ok(_) => {}
                    Err(e) => tracing::warn!("[#46] UNC verbatim 마이그레이션 실패: {e}"),
                }
            }

            // 이전 세션에서 남긴 미전송 crash log 를 Telegram 으로 지연 전송
            // (네이티브 크래시/OOM kill 등 panic hook 이 실행되지 못한 경우 대비)
            // 사용자 설정 백엔드 게이트는 함수 내부에서 처리한다.
            commands::telemetry::spawn_flush_pending_crash_logs(container.app_data_dir.clone());

            startup::report_kordoc_availability(app);

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

            startup::set_indexing_callbacks(app, &container);

            // 기존 감시 폴더들 자동 감시 복원 — app.manage 이후 백그라운드 스레드로 실행한다.
            // v3.4.5 이전에는 resume_watchers 가 get_watch_manager 를 통해 OCR 엔진(ort 세션)을
            // 즉시 빌드해서, 내부망 EDR 이 그 로드를 격리하거나 ort 로드가 지연되면 setup 메인
            // 스레드가 물려 창 표시 자체가 막혔다(이슈 #35). 지금은 OCR 셀을 공유만 하지만,
            // WatchManager 생성은 감시 폴더 수만큼 파일시스템을 훑으므로 백그라운드가 맞다.
            // (아래로 이동됨)

            startup::spawn_disk_type_detection();

            startup::spawn_filename_cache_load(&container);

            // Store app container
            app.manage(RwLock::new(container));

            // OCR 워밍업은 위 seed 스레드 끝(모델 준비 완료 후)에서 건다 — 여기서 걸면
            // 번들 seed 의 dict.txt 재복사와 레이스가 난다(이슈 #35).

            // 감시 폴더 자동 복원 (위에서 이동) — OCR 엔진 빌드가 창 표시를 막지 않도록 분리.
            startup::spawn_resume_watchers(app);

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

            startup::setup_tray(app)?;

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
                        graceful_shutdown(window.app_handle());
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
            drag_preview_icon,
            commands::search::search_keyword,
            commands::search::search_filename,
            commands::search::search_semantic,
            commands::search::search_hybrid,
            commands::search::search_smart,
            commands::search::find_similar_documents,
            commands::search::classify_documents,
            commands::search::save_search_query,
            commands::search::get_document_statistics,
            commands::search::get_recently_opened_documents,
            commands::search::remove_recently_opened_document,
            commands::index::add_folder,
            commands::index::classify_folder,
            commands::index::remove_folder,
            commands::index::get_index_status,
            commands::index::get_all_folder_stats,
            commands::index::get_folders_with_info,
            commands::index::toggle_favorite,
            commands::index::cancel_indexing,
            commands::index::reindex_folder,
            commands::index::reindex_file,
            commands::index::resume_indexing,
            commands::index::reset_folder_indexing,
            commands::index::get_vector_indexing_status,
            commands::index::cancel_vector_indexing,
            commands::index::start_vector_indexing,
            commands::index::clear_all_data,
            commands::index::initialize_app,
            commands::index::start_indexing_batch,
            commands::index::get_indexing_batch,
            commands::index::cancel_indexing_batch,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::count_ocr_reindex_candidates,
            commands::file::open_file,
            commands::file::open_url,
            commands::file::check_github_release,
            commands::file::open_folder,
            commands::file::log_frontend_error,
            commands::file::open_log_dir,
            commands::system::get_suggested_folders,
            commands::preview::load_markdown_preview,
            commands::preview::render_layout_svg,
            commands::preview::render_pdf_page,
            commands::preview::add_bookmark,
            commands::preview::remove_bookmark,
            commands::preview::update_bookmark_note,
            commands::preview::get_bookmarks,
            commands::export::export_csv,
            commands::export::export_markdown,
            commands::search::get_search_history_stats,
            commands::duplicate::find_duplicates,
            commands::lineage::rebuild_lineage,
            commands::lineage::get_lineage_versions,
            commands::lineage::get_lineage_health,
            commands::lineage::get_lineage_diff,
            commands::maintenance::prune_missing_files,
            commands::maintenance::probe_kordoc_runtime,
            commands::maintenance::get_startup_warnings,
            commands::tags::add_file_tag,
            commands::tags::remove_file_tag,
            commands::tags::get_file_tags,
            commands::tags::get_all_tags,
            commands::typo::suggest_correction,
            commands::ai::ask_ai,
            commands::ai::ask_ai_file,
            commands::ai::summarize_ai,
            commands::telemetry::report_error,
            commands::formula::get_formula_models_status,
            commands::formula::download_formula_models,
            indexer::periodic_sync::trigger_sync_if_stale,
        ])
        .build(tauri::generate_context!())
        .map(|app| {
            app.run(|app_handle, event| {
                // macOS dock 아이콘 클릭(Reopen) 시 트레이로 숨겨진 윈도우 다시 표시.
                // close_to_tray 로 hide() 된 상태에서 dock 클릭하면 자동 복귀.
                #[cfg(target_os = "macos")]
                if let tauri::RunEvent::Reopen { has_visible_windows, .. } = event {
                    if !has_visible_windows {
                        if let Some(window) = app_handle.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.unminimize();
                            let _ = window.set_focus();
                        }
                    }
                }
                #[cfg(not(target_os = "macos"))]
                {
                    let _ = (app_handle, event);
                }
            });
            Ok::<(), tauri::Error>(())
        })
        .and_then(|r| r)
        .unwrap_or_else(crash::exit_on_start_failure);
}
