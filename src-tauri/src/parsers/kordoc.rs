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
        if path.exists() && path.extension().and_then(|e| e.to_str()) == Some("js") {
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

    // 3. 프로덕션: 번들된 리소스 디렉토리
    //    tauri.conf.json의 `"resources/kordoc/**/*"` glob이
    //    `$INSTALLDIR/resources/kordoc/...`로 배치됨
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            // 주 경로: Tauri array-glob 번들
            let prod = dir.join("resources").join("kordoc").join("cli.js");
            if prod.exists() {
                return Some(prod);
            }
            // 폴백: 객체 형식이나 평평한 배치를 쓰던 구버전 호환
            let flat = dir.join("kordoc").join("cli.js");
            if flat.exists() {
                return Some(flat);
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

    let cli_path = find_kordoc_cli()
        .ok_or_else(|| ParseError::ParseError("kordoc CLI를 찾을 수 없습니다".to_string()))?;

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

    let cli_path = find_kordoc_cli()
        .ok_or_else(|| ParseError::ParseError("kordoc CLI를 찾을 수 없습니다".to_string()))?;

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
    // 1. 번들된 node.exe (Tauri resources/ array-glob: $INSTALLDIR/resources/node.exe)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let bundled = dir.join("resources").join("node.exe");
            if bundled.exists() {
                return Some(bundled);
            }
            // 폴백: 평평한 배치 (구버전 호환)
            let flat = dir.join("node.exe");
            if flat.exists() {
                return Some(flat);
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
///
/// 타임아웃 관리:
/// - stdout/stderr를 별도 스레드에서 drain하여 파이프 블록 방지
/// - 메인 스레드는 try_wait 폴링 (std::process::Child는 Drop에서 kill하지 않으므로
///   타임아웃 시 명시적 child.kill() 호출 필수)
fn call_kordoc_sync(cli_path: &Path, file_path: &Path) -> Result<String, ParseError> {
    use std::io::Read;
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, Instant};

    let node = which_node()
        .ok_or_else(|| ParseError::ParseError("Node.js가 설치되지 않았습니다".to_string()))?;

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

    let mut child = cmd
        .spawn()
        .map_err(|e| ParseError::ParseError(format!("kordoc 프로세스 시작 실패: {e}")))?;

    let file_display = file_path.display().to_string();
    let timeout = Duration::from_secs(KORDOC_TIMEOUT_SECS);

    // stdout/stderr drain 스레드 (파이프 블록 방지)
    let mut stdout_pipe = child
        .stdout
        .take()
        .ok_or_else(|| ParseError::ParseError("kordoc stdout 캡처 실패".to_string()))?;
    let mut stderr_pipe = child
        .stderr
        .take()
        .ok_or_else(|| ParseError::ParseError("kordoc stderr 캡처 실패".to_string()))?;

    let (stdout_tx, stdout_rx) = mpsc::channel::<Vec<u8>>();
    let (stderr_tx, stderr_rx) = mpsc::channel::<Vec<u8>>();

    thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout_pipe.read_to_end(&mut buf);
        let _ = stdout_tx.send(buf);
    });
    thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stderr_pipe.read_to_end(&mut buf);
        let _ = stderr_tx.send(buf);
    });

    // 폴링 기반 타임아웃: std::process::Child::try_wait + 명시적 kill
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if start.elapsed() >= timeout {
                    // 타임아웃 — 명시적 kill + wait (좀비 방지)
                    let _ = child.kill();
                    let _ = child.wait();
                    warn!(
                        "kordoc 타임아웃 ({}초 초과), 프로세스 강제 종료: {}",
                        KORDOC_TIMEOUT_SECS, file_display
                    );
                    return Err(ParseError::ParseError(format!(
                        "kordoc 타임아웃 ({}초 초과): {}",
                        KORDOC_TIMEOUT_SECS, file_display
                    )));
                }
                thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ParseError::ParseError(format!(
                    "kordoc 프로세스 대기 실패: {e}"
                )));
            }
        }
    };

    // 프로세스 종료 후 파이프 drain 결과 수거 (짧은 타임아웃 — 이미 프로세스 끝났으면 즉시 EOF)
    let drain_timeout = Duration::from_secs(2);
    let stdout_buf = stdout_rx.recv_timeout(drain_timeout).unwrap_or_default();
    let stderr_buf = stderr_rx.recv_timeout(drain_timeout).unwrap_or_default();

    if !status.success() {
        let stderr = String::from_utf8_lossy(&stderr_buf);
        warn!("kordoc failed (exit {}): {}", status, stderr);
        return Err(ParseError::ParseError(format!(
            "kordoc 실행 실패 (exit {status})"
        )));
    }

    // kordoc 출력 크기 제한 (100MB — OOM 방지)
    const MAX_OUTPUT_SIZE: usize = 100 * 1024 * 1024;
    if stdout_buf.len() > MAX_OUTPUT_SIZE {
        return Err(ParseError::ParseError(format!(
            "kordoc 출력 크기 초과: {}MB (최대 {}MB)",
            stdout_buf.len() / 1_048_576,
            MAX_OUTPUT_SIZE / 1_048_576
        )));
    }

    let output = String::from_utf8(stdout_buf)
        .map_err(|_| ParseError::ParseError("kordoc 출력이 유효한 UTF-8이 아닙니다".to_string()))?;

    // pdfjs-dist 등 외부 라이브러리가 stdout에 경고를 출력하는 경우
    // JSON 시작점을 찾아서 앞의 garbage를 제거 (예: "Warning: TT: ...")
    let json_start = output.find('{').ok_or_else(|| {
        ParseError::ParseError("kordoc 출력에 JSON이 없습니다".to_string())
    })?;
    if json_start > 0 {
        debug!(
            "kordoc stdout에 JSON 앞 {}바이트 garbage 제거: {:?}",
            json_start,
            &output[..json_start.min(200)]
        );
    }
    Ok(output[json_start..].to_string())
}

/// ISO 8601 → Unix timestamp (chrono 활용)
fn parse_iso_timestamp(s: &str) -> Option<i64> {
    // chrono는 이미 Cargo.toml에 의존성으로 포함되어 있음
    use chrono::{DateTime, NaiveDateTime};

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
