//! Panic / crash 로그 BENIGN 필터
//!
//! 파서 라이브러리의 알려진 panic (catch_unwind로 처리됨) 과
//! 앱 종료 시점의 이벤트 루프 race (tao) 는 crash.log 오염 방지 대상.
//!
//! 두 경로에서 공유:
//! - `lib.rs`의 panic hook — 실시간 panic 차단
//! - `commands/telemetry.rs`의 deferred flush — 이전 버전/다른 경로에서
//!   이미 기록된 crash-{date}.log 재전송 시 차단

/// 안전하게 무시해도 되는 panic 소스 (파일 경로 substring)
pub const BENIGN_PANIC_SOURCES: &[&str] = &[
    "pdf-extract",
    "lopdf",
    "type1-encoding-parser", // pdf-extract transitive: 손상된 Type1 폰트
    "cff-parser",            // pdf-extract transitive: CFF 폰트 파서
    "quick-xml",
    "calamine",
    "zip-",    // zip-2.x, zip-rs 등
    "ort",     // ONNX Runtime 내부 panic (세션 초기화 / DLL 로드)
    "usearch", // 벡터 인덱스 C++ 바인딩 panic (reserve / add 중)
    "lindera", // 형태소 사전 로드 panic (embedded ko-dic 압축 해제)
    "tao-",    // Windows event loop 내부 상태 전이 패닉 (앱 종료 시점)
    "wry-",    // WebView2 바인딩 panic
    "muda-",   // 트레이/메뉴 바인딩 panic
];

/// panic location 문자열(`file:line`)이 BENIGN 소스에 해당하는지 판정.
#[inline]
pub fn is_benign_location(location: &str) -> bool {
    BENIGN_PANIC_SOURCES
        .iter()
        .any(|src| location.contains(src))
}

/// crash-{date}.log 파일 내용 전체가 BENIGN 패닉으로만 구성되어 있는지 판정.
///
/// 사용처: `spawn_flush_pending_crash_logs` — 과거 버전(tao 필터 추가 전 v2.5.5 이하)
/// 또는 panic hook이 실행되지 못한 경로에서 이미 디스크에 쌓인 로그를 재전송하기 전에
/// 검사해 스팸 차단.
///
/// 빈 파일은 `false` (전송할 게 없음).
/// 한 줄이라도 BENIGN 아니면 `false` (전송 필요).
pub fn is_all_benign(content: &str) -> bool {
    let mut has_content = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        has_content = true;
        if !BENIGN_PANIC_SOURCES.iter().any(|src| trimmed.contains(src)) {
            return false;
        }
    }
    has_content
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tao_panic_is_benign() {
        let loc = r"C:\Users\Chris\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\tao-0.34.5\src\platform_impl\windows\event_loop\runner.rs:368";
        assert!(is_benign_location(loc));
    }

    #[test]
    fn ort_panic_is_benign() {
        let loc = r"C:\Users\user\.cargo\registry\src\ort-2.0.0-rc.11\src\session\mod.rs:123";
        assert!(is_benign_location(loc));
    }

    #[test]
    fn user_panic_is_not_benign() {
        let loc = r"src\parsers\hwpx\mod.rs:42";
        assert!(!is_benign_location(loc));
    }

    #[test]
    fn all_benign_log() {
        let log = "\
[2026-04-24 08:40:40] PANIC at C:\\...\\tao-0.34.5\\runner.rs:368: cannot move state from Destroyed
[2026-04-24 09:12:11] PANIC at C:\\...\\ort-2.0.0-rc.11\\session.rs:42: session init failed
";
        assert!(is_all_benign(log));
    }

    #[test]
    fn mixed_log_is_not_all_benign() {
        let log = "\
[2026-04-24 08:40:40] PANIC at C:\\...\\tao-0.34.5\\runner.rs:368: foo
[2026-04-24 09:00:00] PANIC at src\\commands\\index.rs:99: real bug
";
        assert!(!is_all_benign(log));
    }

    #[test]
    fn empty_log_returns_false() {
        assert!(!is_all_benign(""));
        assert!(!is_all_benign("\n\n  \n"));
    }
}
