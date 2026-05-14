//! Fixed-version WebView2 runtime detection + environment construction (Windows only).
//!
//! WHY: wry 0.54 는 `CreateCoreWebView2EnvironmentWithOptions` 의 첫 인자
//! (browserExecutableFolder) 에 항상 null 을 넘긴다. 따라서 system-installed
//! WebView2 Runtime 이 registry (HKLM 또는 HKCU) 에 등록되어 있지 않으면 wry 가
//! environment 를 만들지 못해 앱이 시작되지 않는다.
//!
//! LTSC 1809 + admin 권한 없음 + 회사 GPO 차단 환경 (이슈 #24 JS190-prog) 에서는
//! Microsoft standalone installer 도 HKLM 에 못 박히고, 다른 사용자 계정 HKCU 만
//! 등록되어 본인 계정 wry detection 이 실패한다.
//!
//! 해결: 사용자가 EBWebView 폴더(Fixed Version Runtime) 를 앱 설치 경로에 직접
//! 풀어두면, 본 모듈이 그것을 감지해 `ICoreWebView2Environment` 를 명시적으로
//! 생성하고, Tauri 의 `WebviewWindowBuilder::with_environment` 로 inject 한다.
//! registry / installer scope 와 무관하게 동작.

use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use webview2_com::CreateCoreWebView2EnvironmentCompletedHandler;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    CreateCoreWebView2EnvironmentWithOptions, ICoreWebView2Environment,
};
use windows::core::PCWSTR;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE, WM_QUIT,
};

/// 환경 생성 대기 timeout. callback 은 일반적으로 수백 ms 안에 호출되므로 5초면
/// 충분. 초과 시 inject 포기하고 fallback (system registry detection) 진행.
const CREATE_ENV_TIMEOUT: Duration = Duration::from_secs(5);

/// `msedgewebview2.exe` 가 들어있는 EBWebView 폴더를 exe 디렉토리 기준으로 검색한다.
///
/// 지원 레이아웃 (우선순위 순):
/// - `<exe_dir>/resources/EBWebView/msedgewebview2.exe`     ← LTSC installer 기본 위치
///   (Tauri `bundle.resources` 는 `src-tauri/resources/EBWebView/**/*` → `<install_dir>/resources/EBWebView/...`)
/// - `<exe_dir>/EBWebView/msedgewebview2.exe`               ← 평면 풀기
/// - `<exe_dir>/<any>/EBWebView/msedgewebview2.exe`         ← Microsoft Fixed Version
///   Runtime zip 의 표준 레이아웃 (`Microsoft.WebView2.FixedVersionRuntime.{ver}.x64/EBWebView/`)
pub fn detect_fixed_runtime_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let exe_dir = exe.parent()?;

    let mut candidates: Vec<PathBuf> = Vec::with_capacity(8);
    // 우선순위 1: LTSC installer 가 풀어두는 위치 — 명시적 확인으로 빠른 hit.
    candidates.push(exe_dir.join("resources").join("EBWebView"));
    // 우선순위 2: 사용자가 zip 을 풀어 평면으로 둔 경우.
    candidates.push(exe_dir.join("EBWebView"));
    // 우선순위 3: 사용자가 zip 을 풀어 sub dir 그대로 둔 경우 (한 단계만 검색).
    if let Ok(entries) = std::fs::read_dir(exe_dir) {
        for entry in entries.flatten() {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                let name = entry.file_name();
                // resources 는 이미 우선순위 1 에서 검사됨 → 중복 skip
                if name == "resources" {
                    continue;
                }
                candidates.push(entry.path().join("EBWebView"));
            }
        }
    }

    candidates
        .into_iter()
        .find(|c| c.join("msedgewebview2.exe").is_file())
}

/// Fixed-runtime 경로로 `ICoreWebView2Environment` 를 동기적으로 생성한다.
///
/// 직접 `PeekMessageW` 로 Win32 message loop 를 돌리면서 비동기 callback 결과를
/// 기다리되, `CREATE_ENV_TIMEOUT` 안에 안 오면 포기하고 Err 를 반환한다.
/// (`webview2_com::wait_with_pump` 는 무한 대기라 환경 생성 실패 시 setup() 자체가
/// hang → 앱 시작 회귀. 우리는 timeout fallback 으로 안전 확보.)
///
/// setup() 콜백은 Tauri 가 main thread 에서 호출하며, 우리가 직접 펌프를 돌리므로
/// winit/tao event loop 시작 여부와 무관하게 동작.
pub fn create_environment(browser_dir: &Path) -> Result<ICoreWebView2Environment, String> {
    let browser_wide: Vec<u16> = browser_dir
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let (tx, rx) = mpsc::channel::<Result<ICoreWebView2Environment, String>>();

    // hr 의 정확한 타입은 webview2-com macros 가 생성하므로 (HRESULT / Result<()>
        //  / windows-core 버전에 따라 변동) 의존 없이 env 만 보고 판단한다.
    // 실패 시엔 hr 를 Debug 로 남겨 진단.
    let handler =
        CreateCoreWebView2EnvironmentCompletedHandler::create(Box::new(move |hr, env| {
            let result = if let Some(env) = env {
                Ok(env)
            } else {
                Err(format!("WebView2 environment creation failed (hr={hr:?})"))
            };
            let _ = tx.send(result);
            Ok(())
        }));

    unsafe {
        CreateCoreWebView2EnvironmentWithOptions(
            PCWSTR(browser_wide.as_ptr()),
            PCWSTR::null(),
            None,
            &handler,
        )
        .map_err(|e| format!("CreateCoreWebView2EnvironmentWithOptions HRESULT: {e}"))?;
    }

    pump_until_recv(&rx, CREATE_ENV_TIMEOUT)?
}

/// Non-blocking Win32 message pump + bounded `recv_timeout` 조합. callback 이
/// `PostMessage` 로 도착하므로 매 iteration 마다 `PeekMessageW` 로 메시지를 비우고
/// 짧게 (50ms) 채널을 polling.
fn pump_until_recv<T>(rx: &mpsc::Receiver<T>, timeout: Duration) -> Result<T, String> {
    let deadline = Instant::now() + timeout;

    loop {
        unsafe {
            let mut msg = MSG::default();
            // PeekMessageW lpmsg: *mut MSG / hwnd: Option<HWND> (windows 0.61).
            // &mut msg → *mut MSG coercion 은 Rust 가 안전하게 처리.
            while PeekMessageW(&mut msg as *mut MSG, None, 0, 0, PM_REMOVE).as_bool() {
                if msg.message == WM_QUIT {
                    return Err("WM_QUIT received during WebView2 environment creation".into());
                }
                let _ = TranslateMessage(&msg as *const MSG);
                DispatchMessageW(&msg as *const MSG);
            }
        }

        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(value) => return Ok(value),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err("WebView2 environment channel disconnected before completion".into());
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if Instant::now() >= deadline {
                    return Err(format!(
                        "WebView2 environment creation timed out after {}s",
                        timeout.as_secs()
                    ));
                }
            }
        }
    }
}
