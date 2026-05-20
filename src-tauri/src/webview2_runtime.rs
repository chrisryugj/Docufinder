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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use webview2_com::CreateCoreWebView2EnvironmentCompletedHandler;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    CreateCoreWebView2EnvironmentWithOptions, ICoreWebView2Environment,
};
use windows::core::PCWSTR;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, MessageBoxW, PeekMessageW, TranslateMessage, MB_ICONERROR, MB_OK,
    MB_SYSTEMMODAL, MSG, PM_REMOVE, WM_QUIT,
};

/// 환경 생성 대기 timeout. callback 은 일반적으로 수백 ms 안에 호출되므로 5초면
/// 충분. 초과 시 inject 포기하고 fallback (system registry detection) 진행.
const CREATE_ENV_TIMEOUT: Duration = Duration::from_secs(5);

/// `msedgewebview2.exe` 가 직접 들어있는 폴더를 exe 디렉토리 기준으로 검색한다.
/// (Microsoft 의 `CreateCoreWebView2EnvironmentWithOptions(browserExecutableFolder=...)`
/// API 는 msedgewebview2.exe 의 직접 부모 폴더를 요구함.)
///
/// 지원 레이아웃 (우선순위 순):
/// - `<exe_dir>/webview2-runtime/EBWebView/x64/msedgewebview2.exe`
///   ← LTSC installer 가 풀어두는 표준 구조 (Tauri `webviewInstallMode:fixedRuntime` +
///   `path:webview2-runtime/` + Microsoft Fixed Version Runtime SDK 의 `EBWebView/<arch>/`)
/// - `<exe_dir>/EBWebView/x64/msedgewebview2.exe`        ← 사용자가 EBWebView 폴더만 풀어둠
/// - `<exe_dir>/EBWebView/msedgewebview2.exe`            ← arch sub dir 없는 평면 zip
/// - `<exe_dir>/msedgewebview2.exe`                       ← 사용자가 같은 폴더에 직접
/// - `<exe_dir>/<any>/msedgewebview2.exe`                 ← sub dir 한 단계 안
/// - `<exe_dir>/<any>/EBWebView/x64/msedgewebview2.exe`   ← Microsoft Fixed Version Runtime cab
///   unzip 의 root 폴더 (예: `Microsoft.WebView2.FixedVersionRuntime.<ver>.x64/`) 그대로 둔 경우
pub fn detect_fixed_runtime_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let exe_dir = exe.parent()?;

    let mut candidates: Vec<PathBuf> = Vec::with_capacity(16);
    // 우선순위 1: LTSC installer 가 풀어두는 정식 위치
    // (이슈 #24 JS190-prog 로그 상 `webview2-runtime/EBWebView/x64/` 로 풀림 확인).
    candidates.push(
        exe_dir
            .join("webview2-runtime")
            .join("EBWebView")
            .join("x64"),
    );
    // 우선순위 2: EBWebView 폴더만 평면으로 풀어둠.
    candidates.push(exe_dir.join("EBWebView").join("x64"));
    candidates.push(exe_dir.join("EBWebView"));
    // 우선순위 3: exe 와 같은 폴더에 직접.
    candidates.push(exe_dir.to_path_buf());
    // 우선순위 4: sub dir 한 단계 안 + 그 안의 `EBWebView/x64/` 도 함께.
    if let Ok(entries) = std::fs::read_dir(exe_dir) {
        for entry in entries.flatten() {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                let p = entry.path();
                // webview2-runtime 은 우선순위 1 에서 명시 확인됐으므로 skip.
                if p.file_name().is_some_and(|n| n == "webview2-runtime") {
                    continue;
                }
                candidates.push(p.join("EBWebView").join("x64"));
                candidates.push(p);
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
    // v2.6.16: WebView2 environment 생성 API 는 COM 기반이라 호출 thread 가
    // CoInitializeEx 로 STA apartment 초기화돼 있어야 한다. Tauri setup() 시점의
    // main thread 는 winit/tao event loop 가 아직 시작되지 않아 COM 미초기화 상태일
    // 수 있고, 그 경우 `CO_E_NOTINITIALIZED (0x800401F0)` 가 떨어진다 (이슈 #23
    // austinjung827 v2.6.15 로그에서 확정). GH Actions runner 는 OS/사전 환경이
    // 다른 모듈에서 이미 COM 을 초기화해둔 상태라 우리 호출이 가려졌었다.
    //
    // S_FALSE (이미 같은 mode 로 초기화됨) / RPC_E_CHANGED_MODE (다른 apartment 로
    // 이미 초기화됨) 는 둘 다 무시 — COM 자체는 활성 상태이므로 후속 API 호출이
    // 가능하다. 진짜 실패는 거의 없다.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }

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

/// Fixed Version Runtime 폴더에 App Container 읽기 권한을 부여한다 (Windows 10 필수).
///
/// WHY: WebView2 v120 부터 renderer 프로세스가 App Container sandbox 안에서
/// 실행된다. unpackaged Win32 앱(본 앱)이 Fixed Version Runtime 을 쓸 때, runtime
/// 폴더에 `ALL APPLICATION PACKAGES`(S-1-15-2-1) / `ALL RESTRICTED APPLICATION
/// PACKAGES`(S-1-15-2-2) 읽기 권한이 없으면 sandbox 안 renderer 가 runtime 파일을
/// 못 읽어 controller 생성이 hang/실패한다 (Microsoft 공식 문서
/// microsoft-edge/webview2/concepts/distribution 의 Fixed Version 배포 절차 명시).
/// NSIS 가 푼 폴더엔 이 ACL 이 없으므로 startup 에서 보강한다. Windows 11 / packaged
/// 앱에는 영향 없음.
///
/// `.acl-applied` 마커로 멱등 처리. runtime 폴더가 앱 업데이트로 통째 교체되면
/// 마커도 사라져 자동 재실행된다. 반환값은 ACL 부여 성공 여부 (진단용).
pub fn grant_app_container_access(runtime_dir: &Path) -> bool {
    let marker = runtime_dir.join(".acl-applied");
    if marker.exists() {
        tracing::info!("App Container ACL: 이미 적용됨 (marker present)");
        return true;
    }

    // SID 직접 지정 — 로케일 독립적 (한글 Windows 의 "모든 애플리케이션 패키지" 등
    // 표시 이름에 의존하지 않는다). (OI)(CI) = 하위 파일/폴더 상속, (RX) = 읽기+실행.
    let mut all_ok = true;
    for sid in ["*S-1-15-2-1", "*S-1-15-2-2"] {
        match std::process::Command::new("icacls")
            .arg(runtime_dir)
            .arg("/grant")
            .arg(format!("{sid}:(OI)(CI)(RX)"))
            .args(["/T", "/C", "/Q"])
            .output()
        {
            Ok(out) if out.status.success() => {
                tracing::info!("App Container ACL 부여: {sid}");
            }
            Ok(out) => {
                all_ok = false;
                tracing::warn!(
                    "App Container ACL 부여 실패 {sid} (exit {:?}): {}",
                    out.status.code(),
                    String::from_utf8_lossy(&out.stderr).trim()
                );
            }
            Err(e) => {
                all_ok = false;
                tracing::warn!("icacls 실행 실패 {sid}: {e}");
            }
        }
    }

    if all_ok {
        let _ = std::fs::write(&marker, b"v2.6.17");
    }
    all_ok
}

/// Fixed Version Runtime 폴더의 controller 필수 파일 존재/크기 요약 문자열.
/// startup 로그 + watchdog 진단 다이얼로그 양쪽에서 쓴다.
pub fn runtime_diagnostics(runtime_dir: &Path) -> String {
    let mut lines = vec![format!("runtime: {}", runtime_dir.display())];

    // controller(브라우저 자식 프로세스) 가 요구하는 핵심 파일 — 존재 + 크기.
    for name in [
        "msedgewebview2.exe",
        "msedgewebview2.exe.sig",
        "icudtl.dat",
        "resources.pak",
        "v8_context_snapshot.bin",
    ] {
        match std::fs::metadata(runtime_dir.join(name)) {
            Ok(m) => lines.push(format!("  {name}: {} bytes", m.len())),
            Err(_) => lines.push(format!("  {name}: MISSING")),
        }
    }

    // EmbeddedBrowserWebView.dll 은 evergreen 구조상 EBWebView\<arch>\ 하위에 있어
    // 재귀 탐색한다.
    lines.push(
        match find_recursive(runtime_dir, "EmbeddedBrowserWebView.dll") {
            Some((path, size)) => {
                format!(
                    "  EmbeddedBrowserWebView.dll: {size} bytes ({})",
                    path.display()
                )
            }
            None => "  EmbeddedBrowserWebView.dll: MISSING".to_string(),
        },
    );

    let (count, total) = dir_size(runtime_dir);
    lines.push(format!("  total: {count} files, {} MB", total / 1_048_576));
    lines.join("\n")
}

/// runtime_dir 하위에서 파일명이 일치하는 첫 파일의 (경로, 크기).
fn find_recursive(root: &Path, name: &str) -> Option<(PathBuf, u64)> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.file_name().is_some_and(|n| n == name) {
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                return Some((path, size));
            }
        }
    }
    None
}

/// runtime_dir 하위 (파일 수, 총 바이트).
fn dir_size(root: &Path) -> (u64, u64) {
    let mut stack = vec![root.to_path_buf()];
    let (mut count, mut total) = (0u64, 0u64);
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                count += 1;
                total += entry.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
    }
    (count, total)
}

/// `builder.build()` (wry 의 controller 생성) 가 hang 할 때를 대비한 watchdog.
///
/// wry 0.54 는 `CreateCoreWebView2Controller` 의 완료 callback 을
/// `webview2_com::wait_with_pump` 로 **무한 대기**한다 (GetMessageA 블로킹). 회사
/// 보안 정책이 브라우저 자식 프로세스를 차단하거나 runtime 이 불완전하면 callback
/// 이 영영 안 와서 `build()` 가 영원히 멈춘다 (이슈 #23 v2.6.16). 사용자는 작업
/// 관리자 강제 종료 외에 방법이 없다. timeout 후 진단 메시지를 띄우고 종료한다.
pub struct BuildWatchdog {
    done: Arc<AtomicBool>,
}

impl BuildWatchdog {
    /// `build()` 가 (성공이든 실패든) 반환된 직후 호출해 watchdog 을 해제한다.
    pub fn disarm(&self) {
        self.done.store(true, Ordering::SeqCst);
    }
}

/// `timeout` 안에 `disarm()` 이 호출되지 않으면 `diag` 를 담은 에러 다이얼로그를
/// 띄우고 `exit(1)`.
pub fn spawn_build_watchdog(timeout: Duration, diag: String) -> BuildWatchdog {
    let done = Arc::new(AtomicBool::new(false));
    let done_thread = done.clone();
    std::thread::spawn(move || {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(250));
            if done_thread.load(Ordering::SeqCst) {
                return;
            }
        }
        if done_thread.load(Ordering::SeqCst) {
            return;
        }
        tracing::error!(
            "WebView2 controller 생성이 {}s 내 미완료 — hang 판정, 종료",
            timeout.as_secs()
        );
        let body = format!(
            "WebView2 초기화가 응답하지 않습니다.\n\n\
             WebView2 런타임은 정상 감지됐으나 브라우저 프로세스 생성 단계에서\n\
             멈췄습니다. 회사 보안 정책(EDR / AppLocker)이 자식 프로세스 생성을\n\
             차단하는 환경일 수 있습니다.\n\n\
             [진단 정보]\n{diag}\n\n\
             위 내용을 캡처해 개발자에게 전달해 주세요. 앱을 종료합니다."
        );
        show_error_dialog("Anything — WebView2 초기화 실패", &body);
        std::process::exit(1);
    });
    BuildWatchdog { done }
}

fn show_error_dialog(title: &str, body: &str) {
    let title_w: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    let body_w: Vec<u16> = body.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let _ = MessageBoxW(
            None,
            PCWSTR(body_w.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            MB_OK | MB_ICONERROR | MB_SYSTEMMODAL,
        );
    }
}
