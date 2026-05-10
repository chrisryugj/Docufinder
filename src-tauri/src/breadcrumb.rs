//! Crash breadcrumb — "현재 처리 중인 파일 + 단계" 영속 기록.
//!
//! 사용자 환경에서 native crash (stack overflow / abort / segfault) 가 발생하면
//! `panic_info.location()` 만으로는 어떤 파일이 트리거였는지 알 수 없다.
//! lib.rs panic hook 과 SEH/SIGSEGV 핸들러가 이 breadcrumb 를 읽어 crash log 에 기록한다.
//!
//! 설계:
//! - 단일 글로벌 `RwLock<Option<Breadcrumb>>` — 마지막 1건만 보관 (인덱싱은 thread pool 이지만
//!   crash 직전 처리 중이던 어느 한 파일이 보이면 충분).
//! - lock-free / lock-poisoning safe — panic hook 에서 호출되므로 `try_read` 사용.
//! - 단계는 `&'static str` 슬라이스로 zero-alloc.

use std::path::PathBuf;
use std::sync::RwLock;

#[derive(Clone, Debug)]
pub struct Breadcrumb {
    pub path: PathBuf,
    pub stage: &'static str,
    pub thread: String,
    pub set_at: std::time::SystemTime,
}

static CURRENT: RwLock<Option<Breadcrumb>> = RwLock::new(None);

/// 처리 시작 시 호출. `stage` 는 `&'static str` (예: "parse_xlsx", "tokenize_chunk").
pub fn set(path: &std::path::Path, stage: &'static str) {
    if let Ok(mut guard) = CURRENT.write() {
        *guard = Some(Breadcrumb {
            path: path.to_path_buf(),
            stage,
            thread: std::thread::current()
                .name()
                .unwrap_or("unnamed")
                .to_string(),
            set_at: std::time::SystemTime::now(),
        });
    }
}

/// 정상 완료 시 호출. crash 가 발생하지 않으면 다음 `set` 까지 살아있어도 무방하지만,
/// 짝을 맞춰 두면 panic hook 의 false attribution 을 줄인다.
pub fn clear() {
    if let Ok(mut guard) = CURRENT.write() {
        *guard = None;
    }
}

/// panic hook / SEH 핸들러용 — 가능한 부작용 없이 현재 breadcrumb 스냅샷 반환.
/// lock 이 poisoned 이거나 contention 이면 None.
pub fn snapshot() -> Option<Breadcrumb> {
    CURRENT.try_read().ok().and_then(|g| g.clone())
}

/// RAII 가드: scope 종료 시 자동 clear. 정상/panic 양쪽 모두에서 동작.
pub struct Guard;

impl Guard {
    pub fn new(path: &std::path::Path, stage: &'static str) -> Self {
        set(path, stage);
        Self
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        clear();
    }
}

/// crash log 한 줄 포맷팅 (panic hook 에서 사용).
pub fn format_for_log(bc: &Breadcrumb) -> String {
    let elapsed_ms = bc
        .set_at
        .elapsed()
        .map(|d| d.as_millis() as i64)
        .unwrap_or(-1);
    format!(
        "BREADCRUMB stage={} thread={} elapsed_ms={} path={}",
        bc.stage,
        bc.thread,
        elapsed_ms,
        bc.path.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn set_clear_roundtrip() {
        clear();
        assert!(snapshot().is_none());
        set(Path::new("/tmp/foo.xls"), "parse_xlsx");
        let s = snapshot().expect("present");
        assert_eq!(s.stage, "parse_xlsx");
        assert_eq!(s.path, Path::new("/tmp/foo.xls"));
        clear();
        assert!(snapshot().is_none());
    }

    #[test]
    fn guard_clears_on_drop() {
        clear();
        {
            let _g = Guard::new(Path::new("/tmp/bar.xlsx"), "tokenize_chunk");
            assert!(snapshot().is_some());
        }
        assert!(snapshot().is_none());
    }

    #[test]
    fn guard_clears_on_panic_unwind() {
        clear();
        let _ = std::panic::catch_unwind(|| {
            let _g = Guard::new(Path::new("/tmp/baz.docx"), "embed");
            panic!("simulated");
        });
        assert!(
            snapshot().is_none(),
            "Drop should run on unwind and clear breadcrumb"
        );
    }
}
