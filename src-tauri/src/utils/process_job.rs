//! 자식 프로세스(Node.js 사이드카) 생명주기 관리 — 이슈 #33.
//!
//! 인덱싱 중 HWP/PDF 변환을 위해 kordoc CLI 를 `node` 자식 프로세스로 띄우는데,
//! `std::process::Child` 는 Drop 에서 kill 하지 않고 Windows 는 부모 종료 시 자식을
//! 자동 정리하지 않는다. 그래서 트레이 종료(`graceful_shutdown` → `process::exit`)
//! 나 네이티브 크래시로 앱이 죽으면 in-flight node.exe 들이 고아로 남아 CPU 를
//! 계속 점유했다.
//!
//! 해결: 전역 Job Object 를 만들어 모든 node 자식을 묶는다. Job 에
//! `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` 가 설정돼 있어, 앱 프로세스가 종료되는
//! 순간(정상 종료 / `process::exit` / 네이티브 크래시 모두) Job 핸들이 닫히면서
//! OS 가 묶인 자식 프로세스를 강제 종료한다 — 정리 코드가 실행되지 못하는
//! 비정상 종료 경로까지 커버하는 게 핵심.
//!
//! 비 Windows: no-op (본 이슈는 Windows 한정 신고이며, 다른 플랫폼의 자식 정리
//! 전략은 별도 범위).

use std::process::Child;

/// 자식 프로세스를 앱 수명에 묶어, 앱 종료 시 함께 정리되도록 한다.
/// Windows 전용으로 동작하며 그 외 플랫폼에선 no-op.
/// 실패(권한/이미 종료 등)해도 치명적이지 않으므로 best-effort 로 로그만 남긴다.
#[cfg(windows)]
pub fn track_child(child: &Child) {
    imp::track(child);
}

#[cfg(not(windows))]
pub fn track_child(_child: &Child) {}

#[cfg(windows)]
mod imp {
    use std::os::windows::io::AsRawHandle;
    use std::process::Child;
    use std::sync::OnceLock;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    // HANDLE(= *mut c_void)은 Send/Sync 가 아니므로 정수로 보관한다.
    // 0 = 생성 실패 → track() 이 no-op 으로 동작.
    static JOB: OnceLock<isize> = OnceLock::new();

    fn job_handle() -> HANDLE {
        let raw = *JOB.get_or_init(|| unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                tracing::warn!("CreateJobObject 실패 — 자식 프로세스 자동 정리 비활성");
                return 0;
            }
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let ok = SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                std::ptr::addr_of!(info) as *const core::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );
            if ok == 0 {
                tracing::warn!("SetInformationJobObject 실패 — 자식 프로세스 자동 정리 비활성");
                return 0;
            }
            tracing::info!("자식 프로세스 Job Object 초기화 (KILL_ON_JOB_CLOSE)");
            job as isize
        });
        raw as HANDLE
    }

    pub fn track(child: &Child) {
        let job = job_handle();
        if job.is_null() {
            return;
        }
        let handle = child.as_raw_handle() as HANDLE;
        let ok = unsafe { AssignProcessToJobObject(job, handle) };
        if ok == 0 {
            // 자식이 spawn 직후 즉시 끝났거나 권한 문제 — 인덱싱 자체엔 영향 없음.
            tracing::debug!("AssignProcessToJobObject 실패 (pid={})", child.id());
        }
    }
}
