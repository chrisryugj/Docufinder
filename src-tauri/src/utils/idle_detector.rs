//! 사용자 유휴 시간 감지 (Windows)
//!
//! GetLastInputInfo를 사용하여 마지막 입력 이후 경과 시간을 측정.
//! 백그라운드 파싱 시 사용자 활동 감지하여 일시정지.

use std::time::Duration;

#[cfg(windows)]
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
#[cfg(windows)]
use windows_sys::Win32::System::SystemInformation::GetTickCount;

/// 마지막 사용자 입력 이후 경과 시간
#[cfg(windows)]
pub fn get_idle_duration() -> Duration {
    let mut last_input = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };

    unsafe {
        if GetLastInputInfo(&mut last_input) != 0 {
            let tick_count = GetTickCount();
            let idle_ms = tick_count.wrapping_sub(last_input.dwTime);
            return Duration::from_millis(idle_ms as u64);
        }
    }

    Duration::ZERO
}

#[cfg(not(windows))]
pub fn get_idle_duration() -> Duration {
    // Non-Windows: 항상 유휴 상태로 간주
    Duration::from_secs(3600)
}

/// 사용자가 유휴 상태인지 확인 (지정 시간 이상 입력 없음)
pub fn is_user_idle(min_idle_ms: u64) -> bool {
    get_idle_duration().as_millis() >= min_idle_ms as u128
}

/// 유휴 상태가 될 때까지 대기 (async)
pub async fn wait_for_idle(min_idle_ms: u64) {
    loop {
        if is_user_idle(min_idle_ms) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

/// 유휴 상태가 될 때까지 대기 (sync, polling)
pub fn wait_for_idle_sync(min_idle_ms: u64, check_interval_ms: u64) {
    loop {
        if is_user_idle(min_idle_ms) {
            return;
        }
        std::thread::sleep(Duration::from_millis(check_interval_ms));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_idle_duration() {
        let idle = get_idle_duration();
        // 테스트 실행 중이므로 0 이상이어야 함
        assert!(idle >= Duration::ZERO);
    }
}
