//! 네이티브 DLL 로드 실패 원인 진단 (Windows 전용)
//!
//! ONNX Runtime 로드가 막히는 환경(이슈 #35)에서 `ort` 는 실패를 panic 으로만 알리고
//! 원인 코드는 남기지 않는다. 여기서 같은 DLL 을 직접 한 번 더 열어보고 `GetLastError`
//! 를 해석해, "보안솔루션 차단 / 런타임 누락 / 파일 손상" 중 무엇인지 로그로 특정한다.
//!
//! 진단 전용 — 성공하든 실패하든 앱 동작은 바꾸지 않는다.

/// ONNX Runtime DLL 과 그 전제인 VC++ 재배포 런타임을 직접 로드해 결과를 로그로 남긴다.
/// 프로세스당 1회만 수행한다(워밍업 재시도마다 반복하지 않도록).
pub fn diagnose_onnxruntime_once() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static DONE: AtomicBool = AtomicBool::new(false);
    if DONE.swap(true, Ordering::AcqRel) {
        return;
    }
    diagnose_impl();
}

#[cfg(windows)]
fn diagnose_impl() {
    let Some(ort_path) = std::env::var_os("ORT_DYLIB_PATH") else {
        tracing::warn!("[DLL 진단] ORT_DYLIB_PATH 미설정 — 진단 생략");
        return;
    };

    // onnxruntime.dll 은 C++ 로 빌드돼 VC++ 재배포 런타임을 요구한다. Rust 본체는 이
    // 라이브러리들을 쓰지 않으므로, 앱이 떠도 여기서만 없을 수 있다.
    for name in ["vcruntime140.dll", "vcruntime140_1.dll", "msvcp140.dll"] {
        report(name, try_load(&std::ffi::OsString::from(name)));
    }
    report(&ort_path.to_string_lossy(), try_load(&ort_path));
}

#[cfg(not(windows))]
fn diagnose_impl() {}

#[cfg(windows)]
fn try_load(path: &std::ffi::OsStr) -> Result<(), u32> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::GetLastError;
    use windows_sys::Win32::System::LibraryLoader::{
        LoadLibraryExW, LOAD_WITH_ALTERED_SEARCH_PATH,
    };

    let wide: Vec<u16> = path.encode_wide().chain(std::iter::once(0)).collect();
    // ALTERED_SEARCH_PATH: DLL 옆에 놓인 의존 DLL 도 함께 찾도록 (설치본 배치 그대로 재현)
    let handle = unsafe {
        LoadLibraryExW(
            wide.as_ptr(),
            std::ptr::null_mut(),
            LOAD_WITH_ALTERED_SEARCH_PATH,
        )
    };
    if handle.is_null() {
        Err(unsafe { GetLastError() })
    } else {
        Ok(())
    }
}

#[cfg(windows)]
fn report(name: &str, result: Result<(), u32>) {
    match result {
        Ok(()) => tracing::info!("[DLL 진단] {} 로드 OK", name),
        Err(code) => tracing::warn!(
            "[DLL 진단] {} 로드 실패 (오류 {}) — {}",
            name,
            code,
            explain(code)
        ),
    }
}

/// Windows 로더 오류 코드 해석 — 사용자가 바로 조치할 수 있는 문장으로.
#[cfg(windows)]
fn explain(code: u32) -> &'static str {
    match code {
        2 | 3 => "파일이 없습니다(설치본 손상 가능) — 앱을 재설치해 주세요",
        5 => "액세스 거부 — 보안솔루션/정책이 이 파일의 로드를 막고 있습니다",
        126 => "의존 DLL 을 찾지 못했습니다 — 'Microsoft Visual C++ 2015-2022 재배포 패키지(x64)' 설치가 필요합니다",
        193 => "잘못된 이미지 형식 — 파일이 손상됐거나 32/64비트가 맞지 않습니다",
        1114 => "DLL 초기화 루틴 실패 — 보안솔루션의 후킹/주입 또는 VC++ 재배포 런타임 버전 불일치가 대표 원인입니다",
        _ => "원인 미상 — 이 오류 번호를 이슈에 알려 주세요",
    }
}
