//! kordoc Node.js 사이드카 — HWP/HWPX/DOCX/PDF/XLSX 마크다운 변환
//!
//! `node kordoc/dist/cli.js <path> --format json --silent` 호출 후
//! JSON 응답을 ParsedDocument로 변환한다.

use super::{
    chunk_text, DocumentMetadata, ParseError, ParsedDocument, DEFAULT_CHUNK_OVERLAP,
    DEFAULT_CHUNK_SIZE,
};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tracing::{debug, warn};

/// kordoc 프로세스 타임아웃 (초)
const KORDOC_TIMEOUT_SECS: u64 = 60;

// ─── JSON 응답 구조체 ─────────────────────────────────

#[derive(Deserialize)]
struct KordocResponse {
    success: bool,
    markdown: Option<String>,
    metadata: Option<KordocMetadata>,
    #[serde(default)]
    warnings: Vec<KordocWarning>,
    error: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct KordocMetadata {
    title: Option<String>,
    author: Option<String>,
    created_at: Option<String>,
    page_count: Option<usize>,
}

#[derive(Deserialize)]
struct KordocWarning {
    message: String,
}

// ─── kordoc CLI 경로 해석 ─────────────────────────────

/// kordoc CLI 스크립트 경로 탐색 (dev → prod 순)
fn find_kordoc_cli() -> Option<PathBuf> {
    // 1. 환경변수 (개발 빌드 전용)
    #[cfg(debug_assertions)]
    if let Ok(p) = std::env::var("KORDOC_CLI_PATH") {
        let path = PathBuf::from(&p);
        if path.exists()
            && path.extension().and_then(|e| e.to_str()) == Some("js")
        {
            return Some(path);
        } else {
            warn!("KORDOC_CLI_PATH 무시: 유효하지 않은 경로 {:?}", path);
        }
    }

    // 2. 개발 환경: 로컬 kordoc 프로젝트 (절대 경로만, debug 빌드 전용)
    #[cfg(debug_assertions)]
    {
        let dev = PathBuf::from(r"c:\github_project\kordoc\dist\cli.js");
        if dev.exists() {
            return Some(dev);
        }
    }

    // 3. 프로덕션: 번들된 리소스 디렉토리 (node.exe와 함께 배포됨)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            // Tauri 번들: resources/kordoc/cli.js
            let prod = dir.join("resources").join("kordoc").join("cli.js");
            if prod.exists() {
                return Some(prod);
            }
            // 개발 모드: resources/ 없이 직접
            let dev_prod = dir.join("kordoc").join("cli.js");
            if dev_prod.exists() {
                return Some(dev_prod);
            }
        }
    }

    None
}

// ─── 공개 API ─────────────────────────────────────────

/// kordoc으로 파일 파싱 → ParsedDocument
pub fn parse(path: &Path) -> Result<ParsedDocument, ParseError> {
    // 파일 크기 제한 (기존 Rust 파서와 동일)
    validate_file_size(path)?;

    let cli_path = find_kordoc_cli().ok_or_else(|| {
        ParseError::ParseError("kordoc CLI를 찾을 수 없습니다".to_string())
    })?;

    let json = call_kordoc_sync(&cli_path, path)?;
    let resp: KordocResponse = serde_json::from_str(&json).map_err(|e| {
        warn!("kordoc JSON 파싱 실패: {e}");
        ParseError::ParseError("kordoc 응답 파싱 실패".to_string())
    })?;

    if !resp.success {
        return Err(ParseError::ParseError(
            resp.error.unwrap_or_else(|| "kordoc 파싱 실패".to_string()),
        ));
    }

    let markdown = resp.markdown.unwrap_or_default();
    if markdown.trim().is_empty() {
        return Err(ParseError::ParseError(
            "kordoc: 추출된 텍스트 없음".to_string(),
        ));
    }

    for w in &resp.warnings {
        debug!("kordoc warning [{}]: {}", path.display(), w.message);
    }

    // 메타데이터 변환
    let meta = resp.metadata.unwrap_or(KordocMetadata {
        title: None,
        author: None,
        created_at: None,
        page_count: None,
    });

    let metadata = DocumentMetadata {
        title: meta.title,
        author: meta.author,
        created_at: meta.created_at.and_then(|s| parse_iso_timestamp(&s)),
        page_count: meta.page_count,
    };

    let chunks = chunk_text(&markdown, DEFAULT_CHUNK_SIZE, DEFAULT_CHUNK_OVERLAP);

    Ok(ParsedDocument {
        content: markdown,
        metadata,
        chunks,
    })
}

/// kordoc으로 파일의 full markdown만 추출 (미리보기용)
pub fn get_markdown(path: &Path) -> Result<String, ParseError> {
    validate_file_size(path)?;

    let cli_path = find_kordoc_cli().ok_or_else(|| {
        ParseError::ParseError("kordoc CLI를 찾을 수 없습니다".to_string())
    })?;

    let json = call_kordoc_sync(&cli_path, path)?;
    let resp: KordocResponse = serde_json::from_str(&json).map_err(|e| {
        warn!("kordoc JSON 파싱 실패: {e}");
        ParseError::ParseError("kordoc 응답 파싱 실패".to_string())
    })?;

    if !resp.success {
        return Err(ParseError::ParseError(
            resp.error.unwrap_or_else(|| "kordoc 파싱 실패".to_string()),
        ));
    }

    resp.markdown
        .filter(|m| !m.trim().is_empty())
        .ok_or_else(|| ParseError::ParseError("kordoc: 추출된 텍스트 없음".to_string()))
}

/// kordoc 사용 가능 여부
pub fn is_available() -> bool {
    find_kordoc_cli().is_some() && which_node().is_some()
}

// ─── 내부 헬퍼 ────────────────────────────────────────

/// node 실행 파일 탐색 (번들 node.exe 우선 → 시스템 PATH)
fn which_node() -> Option<PathBuf> {
    // 1. 번들된 node.exe (인스톨러에 포함됨)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let bundled = dir.join("resources").join("node.exe");
            if bundled.exists() {
                return Some(bundled);
            }
            // 개발 모드
            let dev_bundled = dir.join("node.exe");
            if dev_bundled.exists() {
                return Some(dev_bundled);
            }
        }
    }

    // 2. 시스템 PATH
    which::which("node").ok()
}

/// 파일 크기 검증 (MAX_FILE_SIZE 초과 시 거부)
fn validate_file_size(path: &Path) -> Result<(), ParseError> {
    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    if size > super::MAX_FILE_SIZE {
        return Err(ParseError::ParseError(format!(
            "파일 크기 초과: {} bytes (최대 {} bytes)",
            size,
            super::MAX_FILE_SIZE
        )));
    }
    Ok(())
}

/// kordoc CLI 동기 호출 (blocking thread에서 사용, 60초 타임아웃)
fn call_kordoc_sync(cli_path: &Path, file_path: &Path) -> Result<String, ParseError> {
    let node = which_node().ok_or_else(|| {
        ParseError::ParseError("Node.js가 설치되지 않았습니다".to_string())
    })?;

    // Windows \\?\ prefix 제거 (Node.js/kordoc가 처리하지 못함)
    let file_str = file_path.to_string_lossy();
    let file_str = file_str.strip_prefix(r"\\?\").unwrap_or(&file_str);
    let cli_str = cli_path.to_string_lossy();

    debug!("kordoc: {} {} --format json --silent", cli_str, file_str);

    let mut cmd = std::process::Command::new(node);
    cmd.arg(cli_str.as_ref())
        .arg(file_str)
        .arg("--format")
        .arg("json")
        .arg("--silent")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    let child = cmd.spawn().map_err(|e| {
        ParseError::ParseError(format!("kordoc 프로세스 시작 실패: {e}"))
    })?;

    // wait_with_output()을 별도 스레드에서 실행하여 stdout 데드락 방지
    // (try_wait 폴링은 stdout 파이프 버퍼가 차면 데드락 발생 가능)
    let timeout = std::time::Duration::from_secs(KORDOC_TIMEOUT_SECS);
    let file_display = file_path.display().to_string();
    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let result = child.wait_with_output();
        let _ = tx.send(result);
    });

    let output = match rx.recv_timeout(timeout) {
        Ok(Ok(output)) => output,
        Ok(Err(e)) => {
            return Err(ParseError::ParseError(format!(
                "kordoc 출력 읽기 실패: {e}"
            )));
        }
        Err(_) => {
            // 타임아웃 — 스레드 내 child는 drop 시 자동 kill (Windows)
            return Err(ParseError::ParseError(format!(
                "kordoc 타임아웃 ({}초 초과): {}",
                KORDOC_TIMEOUT_SECS, file_display
            )));
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!("kordoc failed (exit {}): {}", output.status, stderr);
        return Err(ParseError::ParseError(format!(
            "kordoc 실행 실패 (exit {})",
            output.status
        )));
    }

    // kordoc 출력 크기 제한 (100MB — OOM 방지)
    const MAX_OUTPUT_SIZE: usize = 100 * 1024 * 1024;
    if output.stdout.len() > MAX_OUTPUT_SIZE {
        return Err(ParseError::ParseError(format!(
            "kordoc 출력 크기 초과: {}MB (최대 {}MB)",
            output.stdout.len() / 1_048_576,
            MAX_OUTPUT_SIZE / 1_048_576
        )));
    }

    String::from_utf8(output.stdout).map_err(|_| {
        ParseError::ParseError("kordoc 출력이 유효한 UTF-8이 아닙니다".to_string())
    })
}

/// ISO 8601 → Unix timestamp (chrono 활용)
fn parse_iso_timestamp(s: &str) -> Option<i64> {
    // chrono는 이미 Cargo.toml에 의존성으로 포함되어 있음
    use chrono::{NaiveDateTime, DateTime};

    // "2024-01-15T09:30:00Z" 또는 "2024-01-15T09:30:00+09:00"
    if let Ok(dt) = DateTime::parse_from_rfc3339(s.trim()) {
        return Some(dt.timestamp());
    }
    // "2024-01-15T09:30:00" (timezone 없음 → UTC 가정)
    if let Ok(dt) = NaiveDateTime::parse_from_str(s.trim(), "%Y-%m-%dT%H:%M:%S") {
        return Some(dt.and_utc().timestamp());
    }
    // "2024-01-15" (날짜만)
    if let Ok(d) = chrono::NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d") {
        return d.and_hms_opt(0, 0, 0).map(|dt| dt.and_utc().timestamp());
    }
    None
}
