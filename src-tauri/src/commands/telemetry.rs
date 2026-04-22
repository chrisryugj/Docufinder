//! 오류 리포트 — 클라이언트 에러를 Telegram Bot 으로 전송.
//!
//! 토큰/채팅 ID 는 빌드 시점 환경변수 로 주입된다. 빈 값이면 전송하지 않음 (개발 환경 보호).
//!   TELEGRAM_BOT_TOKEN=xxxxxx:yyyy
//!   TELEGRAM_CHAT_ID=-1001234567890
//!
//! 프라이버시:
//!   - 문서 내용, 검색어 절대 전송 안 함
//!   - 파일 경로는 %USERPROFILE% → ~ 마스킹
//!   - 설정 error_reporting_enabled=false 면 전송 안 함 (프론트에서 pre-check)
//!
//! 토큰 노출 리스크:
//!   - 바이너리에서 strings 로 추출 가능. 악용 시 BotFather 로 revoke + 새 토큰으로 재빌드.

use serde::{Deserialize, Serialize};

const TELEGRAM_BOT_TOKEN: &str = match option_env!("TELEGRAM_BOT_TOKEN") {
    Some(t) => t,
    None => "",
};
const TELEGRAM_CHAT_ID: &str = match option_env!("TELEGRAM_CHAT_ID") {
    Some(c) => c,
    None => "",
};

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorReport {
    /// "frontend" | "backend" | "panic"
    pub source: String,
    /// 짧은 요약 (60자 권장)
    pub title: String,
    /// 상세 메시지 + 스택 (multi-line OK)
    pub message: String,
    /// 컨텍스트 (경로/액션/버튼 등) — 자유형식 key:value
    #[serde(default)]
    pub context: std::collections::HashMap<String, String>,
}

/// 앱 버전 — Cargo.toml 에서 빌드 시 주입
fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// 경로 내 사용자 디렉토리를 ~ 로 치환
fn anonymize(text: &str) -> String {
    let home = dirs::home_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    if home.is_empty() {
        return text.to_string();
    }
    text.replace(&home, "~")
}

/// Telegram 에 POST. 블로킹 호출 (ureq).
fn send_to_telegram(text: &str) -> Result<(), String> {
    if TELEGRAM_BOT_TOKEN.is_empty() || TELEGRAM_CHAT_ID.is_empty() {
        return Err("Telegram credentials not configured at build time".into());
    }
    let url = format!(
        "https://api.telegram.org/bot{}/sendMessage",
        TELEGRAM_BOT_TOKEN
    );
    let body = serde_json::json!({
        "chat_id": TELEGRAM_CHAT_ID,
        "text": text,
        "parse_mode": "HTML",
        "disable_web_page_preview": true,
    });
    match ureq::post(&url).send_json(body) {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("telegram send failed: {e}")),
    }
}

/// 프론트엔드 호출: error_reporting_enabled 체크는 프론트에서 해야 함.
/// Rust panic hook 에서도 직접 호출 가능 (sync).
#[tauri::command]
pub async fn report_error(payload: ErrorReport) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let msg = format_report(&payload);
        send_to_telegram(&msg)
    })
    .await
    .map_err(|e| format!("spawn_blocking failed: {e}"))?
}

fn format_report(r: &ErrorReport) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    let _ = writeln!(
        out,
        "<b>🐛 [{v}] {src}</b>\n<b>{title}</b>",
        v = app_version(),
        src = r.source,
        title = html_escape(&r.title),
    );
    let _ = writeln!(
        out,
        "\n<pre>{}</pre>",
        html_escape(&anonymize(truncate(&r.message, 1500)))
    );
    if !r.context.is_empty() {
        let _ = writeln!(out, "\n<b>Context</b>");
        for (k, v) in &r.context {
            let _ = writeln!(
                out,
                "• <code>{}</code>: {}",
                html_escape(k),
                html_escape(&anonymize(truncate(v, 200))),
            );
        }
    }
    let _ = writeln!(
        out,
        "\n<i>os={}, arch={}</i>",
        std::env::consts::OS,
        std::env::consts::ARCH,
    );
    out
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        // char boundary 안전하게
        let mut end = max;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        &s[..end]
    }
}

/// Rust panic hook 에서 호출 (sync). 사용자 설정 체크 불가하므로
/// build-time env 존재 여부로만 gate.
pub fn report_panic_sync(location: &str, message: &str) {
    if TELEGRAM_BOT_TOKEN.is_empty() || TELEGRAM_CHAT_ID.is_empty() {
        return;
    }
    let report = ErrorReport {
        source: "panic".into(),
        title: format!("PANIC at {location}"),
        message: message.to_string(),
        context: std::collections::HashMap::new(),
    };
    let text = format_report(&report);
    // best-effort, ignore result
    let _ = send_to_telegram(&text);
}

/// 앱 시작 시 호출: 이전 세션에서 남긴 미전송 crash log 를 Telegram 으로 플러시.
///
/// 시나리오:
///   - 네이티브 크래시 (SEGV / access violation / stack overflow) → panic hook 실행 실패 or 프로세스 즉시 종료
///   - OOM kill / 외부 kill 전 panic hook 완료 실패
///
/// 동작:
///   1. %APPDATA%\com.anything.app\crash-*.log 중 ".sent" suffix 없는 파일 찾기
///   2. 각 파일의 최근 1MB 읽어 Telegram 전송
///   3. 성공 시 ".sent" suffix 로 rename (중복 전송 방지)
///
/// 백그라운드 스레드에서 실행 (앱 시작 지연 방지).
pub fn spawn_flush_pending_crash_logs() {
    if TELEGRAM_BOT_TOKEN.is_empty() || TELEGRAM_CHAT_ID.is_empty() {
        return;
    }
    std::thread::spawn(|| {
        let Some(data_dir) = dirs::data_dir() else { return };
        let crash_dir = data_dir.join("com.anything.app");
        let Ok(entries) = std::fs::read_dir(&crash_dir) else { return };

        for entry in entries.flatten() {
            let path = entry.path();
            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n,
                None => continue,
            };
            if !name.starts_with("crash-") || name.ends_with(".sent") {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(&path) else { continue };
            if content.trim().is_empty() {
                continue;
            }
            // 최근 1MB만 (너무 크면 Telegram 4096자 제한에 걸림 — format_report 에서 잘림)
            let tail = if content.len() > 1_000_000 {
                &content[content.len() - 1_000_000..]
            } else {
                &content
            };
            let report = ErrorReport {
                source: "crash-log".into(),
                title: format!("Deferred crash report: {}", name),
                message: tail.to_string(),
                context: std::collections::HashMap::new(),
            };
            let text = format_report(&report);
            if send_to_telegram(&text).is_ok() {
                // .sent suffix 로 rename → 다음 실행 시 재전송 방지
                let new_path = path.with_file_name(format!("{}.sent", name));
                let _ = std::fs::rename(&path, &new_path);
            }
        }
    });
}
