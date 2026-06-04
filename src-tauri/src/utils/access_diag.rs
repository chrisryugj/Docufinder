//! 접근 거부(`os error 5`) 진단 컨텍스트 수집 (이슈 #29).
//!
//! SMB/UNC 경로 인덱싱이 실패할 때, 사용자 환경의 어떤 요인이 원인인지 한 줄
//! 로그로 좁히기 위해 실행 사용자·elevation·세션·경로 종류·원본 Win32 에러를
//! 함께 기록한다. 모든 조회는 best-effort 이며 실패 시 `?` 로 채운다. 에러 경로에서만
//! 호출되므로 비용은 신경쓰지 않는다.

use std::path::Path;

/// 접근/정규화 실패 진단 문자열 생성.
///
/// - `stage`: 실패 단계 (`path-normalize` / `folder-stats` / `read-dir` 등)
/// - `func`:  호출한 함수/API (`dunce::canonicalize`, `std::fs::read_dir` 등)
pub fn context(path: &Path, stage: &str, func: &str, err: &std::io::Error) -> String {
    let process = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "?".into());
    let user = current_user();
    let elevated = crate::utils::elevation::is_elevated();
    let session = session_id()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "?".into());
    let kind = path_kind(path);
    let raw = err.raw_os_error().unwrap_or(-1);
    format!(
        "  [access-diag] stage={stage} fn={func} path_kind={kind}\n  \
         path={}\n  \
         process={process} user={user} elevated={elevated} session_id={session}\n  \
         raw_os_error={raw} err=\"{err}\"",
        path.display()
    )
}

/// 현재 로그인 사용자 — `USERNAME`(Windows)/`USER`(posix) 환경변수. 조회 실패 시 `?`.
fn current_user() -> String {
    std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "?".into())
}

/// 경로 종류 분류 — UNC / 매핑 네트워크 드라이브 / 로컬 드라이브 / 기타.
/// "매핑 vs UNC" 구분이 이슈 #29 의 핵심 진단 포인트다.
fn path_kind(path: &Path) -> &'static str {
    use crate::utils::network_path;
    if network_path::is_network(path) {
        "UNC"
    } else if has_drive_letter(path) {
        // resolve_mapped_drive_to_unc 가 Some 이면 매핑된 네트워크 드라이브(비-Windows 는 항상 None).
        if network_path::resolve_mapped_drive_to_unc(path).is_some() {
            "mapped-network-drive"
        } else {
            "local-drive"
        }
    } else {
        "other"
    }
}

fn has_drive_letter(path: &Path) -> bool {
    let s = path.as_os_str().to_string_lossy();
    let b = s.as_bytes();
    b.len() >= 2 && b[1] == b':' && b[0].is_ascii_alphabetic()
}

/// 현재 프로세스 토큰의 Windows session id.
///
/// `ProcessIdToSessionId` 대신 elevation.rs 에서 이미 검증된 `OpenProcessToken` +
/// `GetTokenInformation(TokenSessionId)` 경로를 재사용한다 — 새 Win32 API/feature 를
/// 들이지 않아 (맥에서 cfg(windows) 컴파일 검증이 안 되는 제약 하에서도) 안전하다.
#[cfg(windows)]
fn session_id() -> Option<u32> {
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::Security::GetTokenInformation;
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    const TOKEN_QUERY: u32 = 0x0008;
    const TOKEN_SESSION_ID_CLASS: i32 = 12; // TOKEN_INFORMATION_CLASS::TokenSessionId

    // SAFETY: 포인터는 모두 스택 지역변수. 연 token 핸들은 반드시 닫는다.
    // GetCurrentProcess 는 닫을 필요 없는 pseudo-handle.
    unsafe {
        let mut token: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return None;
        }
        let mut sid: u32 = 0;
        let mut ret_len = 0u32;
        let ok = GetTokenInformation(
            token,
            TOKEN_SESSION_ID_CLASS,
            &mut sid as *mut u32 as *mut core::ffi::c_void,
            std::mem::size_of::<u32>() as u32,
            &mut ret_len,
        );
        CloseHandle(token);
        (ok != 0).then_some(sid)
    }
}

#[cfg(not(windows))]
fn session_id() -> Option<u32> {
    None
}
