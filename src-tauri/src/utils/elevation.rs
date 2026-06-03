//! 현재 프로세스의 UAC elevation(관리자 권한 실행) 감지.
//!
//! 관리자 권한으로 실행된 프로세스에는 일반 logon 세션이 만든 매핑 네트워크
//! 드라이브가 보이지 않는다(이슈 #29 — `Y:\` os error 5). 이 신호를 진단 안내
//! 분기에 사용한다. 비-Windows 에서는 항상 false.

/// 현재 프로세스가 elevated(관리자 권한) 토큰으로 실행 중인지.
///
/// 토큰 조회에 실패하면 보수적으로 false — 안내를 못 띄울지언정 오탐으로 혼란을
/// 주지 않는다.
#[cfg(windows)]
pub fn is_elevated() -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::Security::{GetTokenInformation, TOKEN_ELEVATION};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    // winnt.h 안정값 — cloud_detect 관례대로 직접 정의(모듈 위치 흔들림 회피).
    const TOKEN_QUERY: u32 = 0x0008;
    const TOKEN_ELEVATION_CLASS: i32 = 20; // TOKEN_INFORMATION_CLASS::TokenElevation

    // SAFETY: 모든 포인터는 스택 지역 변수를 가리키고, 연 token 핸들은 반드시 닫는다.
    // GetCurrentProcess 는 닫을 필요 없는 pseudo-handle.
    unsafe {
        let mut token: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut ret_len = 0u32;
        let ok = GetTokenInformation(
            token,
            TOKEN_ELEVATION_CLASS,
            &mut elevation as *mut TOKEN_ELEVATION as *mut core::ffi::c_void,
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut ret_len,
        );
        CloseHandle(token);
        ok != 0 && elevation.TokenIsElevated != 0
    }
}

#[cfg(not(windows))]
pub fn is_elevated() -> bool {
    false
}
