//! kordoc Node.js 사이드카 — HWP/HWPX/DOCX/PDF/XLSX 마크다운 변환
//!
//! `node kordoc/dist/cli.js <path> --format json --silent` 호출 후
//! JSON 응답을 ParsedDocument로 변환한다.
//! `render` 서브커맨드로 HWPX 첫 페이지를 조판 보존 SVG 로 렌더한다 (레이아웃 미리보기).

use super::{
    chunk_text, DocumentMetadata, ParseError, ParsedDocument, DEFAULT_CHUNK_OVERLAP,
    DEFAULT_CHUNK_SIZE,
};
use serde::Deserialize;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tracing::{debug, warn};

/// kordoc 프로세스 기본 타임아웃 (초)
const KORDOC_TIMEOUT_SECS: u64 = 60;
/// 수식 OCR 활성화 시 타임아웃 (초) — 모델 로드 + 페이지별 MFD/MFR 추론으로 시간이 늘어남.
const KORDOC_FORMULA_TIMEOUT_SECS: u64 = 600;
/// 레이아웃 SVG 응답 상한 — 문서 내 이미지가 base64 로 임베드되어 다페이지 사진
/// 문서는 수십 MB 가 될 수 있다 (실측: 24MB 보도자료 HWPX → 30.4MB SVG).
/// 이를 넘기면 IPC/data URI 렌더 부담이 커서 거절한다.
const RENDER_MAX_SVG_SIZE: u64 = 50 * 1024 * 1024;

/// 번들/시스템 node 실행 파일 이름 (Windows: node.exe / 그 외: node)
#[cfg(target_os = "windows")]
const NODE_BIN: &str = "node.exe";
#[cfg(not(target_os = "windows"))]
const NODE_BIN: &str = "node";

/// kordoc 호출 옵션
#[derive(Debug, Clone, Copy, Default)]
pub struct KordocOptions {
    /// PDF 수식 OCR 활성화 (--formula-ocr)
    pub formula_ocr: bool,
}

/// 전역 수식 OCR 토글. Settings 에서 변경될 때 set_formula_ocr_enabled 로 갱신.
///
/// parse_file / parse / get_markdown 등 모든 경로에서 옵션을 일일이 전달하지 않고,
/// PDF 파일일 때만 이 값을 KordocOptions 로 주입한다.
static FORMULA_OCR_ENABLED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Settings 로딩/변경 시 호출되어 전역 토글을 갱신한다.
pub fn set_formula_ocr_enabled(enabled: bool) {
    FORMULA_OCR_ENABLED.store(enabled, std::sync::atomic::Ordering::Relaxed);
}

/// 현재 전역 formula OCR 토글 상태.
pub fn is_formula_ocr_enabled() -> bool {
    FORMULA_OCR_ENABLED.load(std::sync::atomic::Ordering::Relaxed)
}

/// PDF 경로인 경우에만 formula_ocr 전파 — HWPX/DOCX/HWP5 에는 영향 없음 (kordoc 내부에서
/// formulaOcr 플래그는 PDF 파서 분기에만 사용됨).
fn options_for_path(path: &Path) -> KordocOptions {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if ext == "pdf" && is_formula_ocr_enabled() {
        KordocOptions { formula_ocr: true }
    } else {
        KordocOptions::default()
    }
}

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
    //    Windows: `$INSTALLDIR/resources/kordoc/...`
    //    macOS:   `Contents/Resources/resources/kordoc/...` (앱 번들)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            // macOS 앱 번들: Contents/MacOS/<binary> → ../Resources/resources/kordoc
            #[cfg(target_os = "macos")]
            {
                let mac_prod = dir
                    .join("..")
                    .join("Resources")
                    .join("resources")
                    .join("kordoc")
                    .join("cli.js");
                if mac_prod.exists() {
                    return Some(mac_prod);
                }
            }
            // Windows / 평평한 배치 (Tauri array-glob)
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

/// kordoc으로 파일 파싱 → ParsedDocument (전역 formula OCR 토글 자동 반영).
pub fn parse(path: &Path) -> Result<ParsedDocument, ParseError> {
    parse_with_options(path, options_for_path(path))
}

/// kordoc 옵션을 지정해 파싱 (예: formula_ocr).
pub fn parse_with_options(path: &Path, opts: KordocOptions) -> Result<ParsedDocument, ParseError> {
    // 파일 크기 제한 (기존 Rust 파서와 동일)
    validate_file_size(path)?;

    let cli_path = find_kordoc_cli()
        .ok_or_else(|| ParseError::ParseError("kordoc CLI를 찾을 수 없습니다".to_string()))?;

    let json = call_kordoc_sync(&cli_path, path, opts)?;
    let resp: KordocResponse = serde_json::from_str(&json).map_err(|e| {
        let tail = json
            .char_indices()
            .rev()
            .nth(79)
            .map(|(idx, _)| &json[idx..])
            .unwrap_or(&json);
        warn!(
            "kordoc JSON 파싱 실패 ({}B): {} | tail={:?}",
            json.len(),
            e,
            tail
        );
        ParseError::ParseError(format!(
            "kordoc 응답 파싱 실패 ({}B, {}): tail={:?}",
            json.len(),
            e,
            tail
        ))
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

    // kordoc 은 colspan/rowspan 병합셀 표를 GFM 으로 표현하지 못해 HTML <table> 로 반환한다.
    // 그대로 인덱싱하면 검색 결과 스니펫에 td/tr/colspan 태그가 노출되므로(FTS5 토큰화 과정에서
    // 깨진 잔재까지 섞여 표시 레이어 정규식만으로는 못 막음) 검색용 content/chunks 에선 표를
    // plain text 로 직렬화한다. 미리보기 패널은 get_markdown 원본을 써 GFM 렌더가 유지된다.
    let content = html_tables_to_text(&markdown);
    let chunks = chunk_text(&content, DEFAULT_CHUNK_SIZE, DEFAULT_CHUNK_OVERLAP);

    Ok(ParsedDocument {
        content,
        metadata,
        chunks,
    })
}

/// kordoc으로 파일의 full markdown만 추출 (미리보기용, 전역 formula OCR 토글 반영).
pub fn get_markdown(path: &Path) -> Result<String, ParseError> {
    get_markdown_with_options(path, options_for_path(path))
}

/// get_markdown + 옵션 지정
pub fn get_markdown_with_options(path: &Path, opts: KordocOptions) -> Result<String, ParseError> {
    validate_file_size(path)?;

    let cli_path = find_kordoc_cli()
        .ok_or_else(|| ParseError::ParseError("kordoc CLI를 찾을 수 없습니다".to_string()))?;

    let json = call_kordoc_sync(&cli_path, path, opts)?;
    let resp: KordocResponse = serde_json::from_str(&json).map_err(|e| {
        let tail = json
            .char_indices()
            .rev()
            .nth(79)
            .map(|(idx, _)| &json[idx..])
            .unwrap_or(&json);
        warn!(
            "kordoc JSON 파싱 실패 ({}B): {} | tail={:?}",
            json.len(),
            e,
            tail
        );
        ParseError::ParseError(format!(
            "kordoc 응답 파싱 실패 ({}B, {}): tail={:?}",
            json.len(),
            e,
            tail
        ))
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

/// kordoc render — HWPX 조판을 전체 페이지 세로 스택 SVG 로 렌더 (레이아웃 미리보기용).
/// `highlights` 는 검색어 형광펜 (kordoc `--highlight`).
///
/// persistent 워커(render-worker)를 우선 사용해 node 콜드스타트를 없애고, 워커 통신
/// 실패 시 1회성 spawn 으로 폴백한다. `reflow=true` 로 조판 캐시 없는 HWPX(생성/편집본)도
/// 순수 TS 조판으로 렌더한다(캐시 있으면 kordoc 이 자동으로 캐시 재생 — 무회귀).
pub fn render_svg(path: &Path, highlights: &[String]) -> Result<String, ParseError> {
    validate_file_size(path)?;
    match render_svg_via_worker(path, highlights, true) {
        Ok(svg) => Ok(svg),
        // 워커 인프라 실패(spawn·파이프 끊김·프로세스 종료)만 1회성 spawn 폴백.
        // 렌더 실패(ok:false)는 콜드스타트로 재시도해도 동일하게 실패하고, 타임아웃은
        // 폴백까지 겹치면 60+60 ≈ 120초 스피너가 되므로 즉시 에러 반환한다.
        Err((e, true)) => {
            warn!("render 워커 인프라 실패 — 1회성 spawn 폴백: {}", e);
            render_svg_oneshot(path, highlights, true)
        }
        Err((e, false)) => Err(e),
    }
}

/// render 1회성 spawn (워커 폴백) — 임시 파일(-o)로 받아 읽은 뒤 삭제한다.
fn render_svg_oneshot(
    path: &Path,
    highlights: &[String],
    reflow: bool,
) -> Result<String, ParseError> {
    let cli_path = find_kordoc_cli()
        .ok_or_else(|| ParseError::ParseError("kordoc CLI를 찾을 수 없습니다".to_string()))?;

    let file_owned = crate::utils::network_path::simplify(path);
    let file_str = file_owned.to_string_lossy();

    // 유니크 임시 경로만 만들고 파일 생성은 kordoc 에 맡긴다 — 미리 열어두면
    // Windows 파일 공유 잠금과 충돌할 수 있다.
    let out_path = std::env::temp_dir().join(format!(
        "docufinder-render-{}-{}.svg",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default()
    ));

    debug!("kordoc render: {} -o {}", file_str, out_path.display());

    let mut args: Vec<std::ffi::OsString> = vec![
        "render".into(),
        file_str.as_ref().into(),
        "-o".into(),
        out_path.clone().into(),
        "--silent".into(),
    ];
    if reflow {
        args.push("--reflow".into());
    }
    // kordoc 은 쉼표 구분 목록을 받으므로 검색어 내 쉼표는 공백으로 정규화
    let terms: Vec<String> = highlights
        .iter()
        .map(|t| t.replace(',', " ").trim().to_string())
        .filter(|t| !t.is_empty())
        .collect();
    if !terms.is_empty() {
        args.push("--highlight".into());
        args.push(terms.join(",").into());
    }

    let result = (|| {
        let out = run_kordoc_process(
            &cli_path,
            &args,
            KORDOC_TIMEOUT_SECS,
            &path.display().to_string(),
        )?;

        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            warn!("kordoc render failed (exit {}): {}", out.status, stderr);
            let snippet = stderr_snippet(&stderr);
            return Err(ParseError::ParseError(if snippet.is_empty() {
                format!("kordoc render 실패 (exit {})", out.status)
            } else {
                // "[kordoc] 오류:" 는 CLI 로그 프리픽스 — 사용자 노출 메시지에서 제거
                snippet
                    .trim_start_matches("[kordoc] 오류:")
                    .trim()
                    .to_string()
            }));
        }

        // exit 0 이어도 출력 파일 존재/크기로 최종 판정 (조판 캐시 이상 등 방어)
        let size = std::fs::metadata(&out_path).map(|m| m.len()).unwrap_or(0);
        if size == 0 {
            return Err(ParseError::ParseError(
                "kordoc render: SVG 출력이 생성되지 않았습니다".to_string(),
            ));
        }
        if size > RENDER_MAX_SVG_SIZE {
            return Err(ParseError::ParseError(format!(
                "레이아웃 SVG 크기 초과: {}MB (최대 {}MB)",
                size / 1_048_576,
                RENDER_MAX_SVG_SIZE / 1_048_576
            )));
        }

        std::fs::read_to_string(&out_path)
            .map_err(|e| ParseError::ParseError(format!("SVG 읽기 실패: {e}")))
    })();

    // 성공/실패 무관 임시 파일 정리 (없으면 무시)
    let _ = std::fs::remove_file(&out_path);

    result
}

// ─── persistent 렌더 워커 (콜드스타트 제거) ───────────────

/// 워커 응답용 유니크 임시 SVG 경로
fn temp_svg_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "docufinder-render-{}-{}.svg",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default()
    ))
}

/// 살아있는 render-worker 프로세스 + stdin + 응답 채널(백그라운드 리더 스레드)
struct RenderWorker {
    child: Child,
    stdin: ChildStdin,
    resp_rx: Receiver<String>,
    next_id: u64,
}

/// 전역 단일 렌더 워커 — 미리보기는 순차라 Mutex 직렬화로 충분하다.
static RENDER_WORKER: Mutex<Option<RenderWorker>> = Mutex::new(None);

/// render-worker 프로세스를 띄우고 ready 신호를 소비한다.
fn spawn_render_worker() -> Result<RenderWorker, ParseError> {
    let node = which_node()
        .ok_or_else(|| ParseError::ParseError("Node.js가 설치되지 않았습니다".to_string()))?;
    let cli_path = find_kordoc_cli()
        .ok_or_else(|| ParseError::ParseError("kordoc CLI를 찾을 수 없습니다".to_string()))?;

    let mut cmd = std::process::Command::new(node);
    cmd.arg(cli_path.to_string_lossy().as_ref())
        .arg("render-worker")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| ParseError::ParseError(format!("render 워커 시작 실패: {e}")))?;
    // 이슈 #33: 앱 종료/크래시 시 고아 방지
    crate::utils::process_job::track_child(&child);

    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| ParseError::ParseError("워커 stdin 캡처 실패".to_string()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| ParseError::ParseError("워커 stdout 캡처 실패".to_string()))?;

    // 백그라운드 리더 — stdout 라인을 채널로 (요청 스레드는 recv_timeout 으로 대기)
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            match line {
                Ok(l) => {
                    if tx.send(l).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let mut worker = RenderWorker {
        child,
        stdin,
        resp_rx: rx,
        next_id: 1,
    };
    // ready 신호 대기 (모듈 로드 시간 고려 30초)
    match worker.resp_rx.recv_timeout(Duration::from_secs(30)) {
        Ok(line) if line.contains("\"ready\"") => Ok(worker),
        Ok(other) => {
            let _ = worker.child.kill();
            let _ = worker.child.wait(); // kill 만 하면 유닉스에서 좀비로 남는다
            Err(ParseError::ParseError(format!(
                "render 워커 준비 실패 — 예상치 못한 응답: {}",
                other.chars().take(80).collect::<String>()
            )))
        }
        Err(_) => {
            let _ = worker.child.kill();
            let _ = worker.child.wait();
            Err(ParseError::ParseError(
                "render 워커 준비 타임아웃".to_string(),
            ))
        }
    }
}

/// persistent 워커로 render — NDJSON 요청/응답, id 매칭, 통신/타임아웃 실패 시 워커 폐기.
///
/// 에러의 `bool` 은 "1회성 spawn 폴백이 의미 있는가" — 워커 인프라 실패(spawn·파이프
/// 끊김·프로세스 종료)만 true. 렌더 실패(ok:false)·타임아웃·SVG 후처리 실패는 폴백해도
/// 동일 실패거나 대기만 배가되므로 false.
fn render_svg_via_worker(
    path: &Path,
    highlights: &[String],
    reflow: bool,
) -> Result<String, (ParseError, bool)> {
    let file_owned = crate::utils::network_path::simplify(path);
    let file_str = file_owned.to_string_lossy().to_string();
    let out_path = temp_svg_path();
    let out_str = out_path.to_string_lossy().to_string();

    let terms: Vec<String> = highlights
        .iter()
        .map(|t| t.replace(',', " ").trim().to_string())
        .filter(|t| !t.is_empty())
        .collect();

    let mut guard = RENDER_WORKER.lock().map_err(|_| {
        (
            ParseError::ParseError("render 워커 잠금 실패".to_string()),
            false,
        )
    })?;
    if guard.is_none() {
        *guard = Some(spawn_render_worker().map_err(|e| (e, true))?);
    }
    let worker = guard.as_mut().unwrap();

    let id = worker.next_id;
    worker.next_id += 1;

    let req = serde_json::json!({
        "id": id,
        "file": file_str,
        "out": out_str,
        "reflow": reflow,
        "highlight": terms,
    });

    // worker_dead: 통신/타임아웃 실패(워커 자체 이상)만 true — render 실패(ok:false)는 워커 정상.
    // 에러의 bool 은 폴백 힌트(retry_oneshot) — 타임아웃은 워커를 폐기(worker_dead)하되
    // 폴백은 하지 않는다(대기 배가 방지).
    let mut worker_dead = false;
    let result: Result<String, (ParseError, bool)> = (|| {
        if writeln!(worker.stdin, "{req}")
            .and_then(|_| worker.stdin.flush())
            .is_err()
        {
            worker_dead = true;
            return Err((
                ParseError::ParseError("render 워커 통신 실패 (파이프 끊김)".to_string()),
                true,
            ));
        }

        let deadline = Instant::now() + Duration::from_secs(KORDOC_TIMEOUT_SECS);
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                worker_dead = true;
                return Err((
                    ParseError::ParseError(format!(
                        "render 워커 타임아웃 ({}초 초과)",
                        KORDOC_TIMEOUT_SECS
                    )),
                    false,
                ));
            }
            match worker.resp_rx.recv_timeout(remaining) {
                Ok(line) => {
                    let resp: serde_json::Value = match serde_json::from_str(&line) {
                        Ok(v) => v,
                        Err(_) => continue, // 잡음 라인 무시
                    };
                    if resp.get("id").and_then(|v| v.as_u64()) != Some(id) {
                        continue; // 이전 타임아웃 요청의 지연 응답 — 스킵
                    }
                    if resp.get("ok").and_then(|v| v.as_bool()) == Some(true) {
                        let size = std::fs::metadata(&out_path).map(|m| m.len()).unwrap_or(0);
                        if size > RENDER_MAX_SVG_SIZE {
                            return Err((
                                ParseError::ParseError(format!(
                                    "레이아웃 SVG 크기 초과: {}MB (최대 {}MB)",
                                    size / 1_048_576,
                                    RENDER_MAX_SVG_SIZE / 1_048_576
                                )),
                                false,
                            ));
                        }
                        return std::fs::read_to_string(&out_path).map_err(|e| {
                            (ParseError::ParseError(format!("SVG 읽기 실패: {e}")), false)
                        });
                    }
                    let msg = resp
                        .get("error")
                        .and_then(|v| v.as_str())
                        .unwrap_or("render 실패")
                        .to_string();
                    return Err((ParseError::ParseError(msg), false));
                }
                Err(RecvTimeoutError::Timeout) => {
                    worker_dead = true;
                    return Err((
                        ParseError::ParseError("render 워커 타임아웃".to_string()),
                        false,
                    ));
                }
                Err(RecvTimeoutError::Disconnected) => {
                    worker_dead = true;
                    return Err((
                        ParseError::ParseError("render 워커 종료됨".to_string()),
                        true,
                    ));
                }
            }
        }
    })();

    let _ = std::fs::remove_file(&out_path);

    if worker_dead {
        if let Some(w) = guard.as_mut() {
            let _ = w.child.kill();
            let _ = w.child.wait(); // 좀비 회수 (oneshot 러너와 대칭)
        }
        *guard = None;
    }
    result
}

/// kordoc 사용 가능 여부
pub fn is_available() -> bool {
    find_kordoc_cli().is_some() && which_node().is_some()
}

/// 외부 모듈(commands::formula)에서 사이드카 경로 조회용으로 노출.
pub fn find_kordoc_cli_public() -> Option<PathBuf> {
    find_kordoc_cli()
}

/// 외부 모듈(commands::formula)에서 node 실행 경로 조회용으로 노출.
pub fn which_node_public() -> Option<PathBuf> {
    which_node()
}

// ─── 내부 헬퍼 ────────────────────────────────────────

/// node 실행 파일 탐색 (번들 node 우선 → 시스템 PATH)
fn which_node() -> Option<PathBuf> {
    // 1. 번들된 node
    //    Windows: $INSTALLDIR/resources/<NODE_BIN>
    //    macOS:   Contents/Resources/resources/<NODE_BIN> (앱 번들)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            #[cfg(target_os = "macos")]
            {
                let mac_bundled = dir
                    .join("..")
                    .join("Resources")
                    .join("resources")
                    .join(NODE_BIN);
                if mac_bundled.exists() {
                    return Some(mac_bundled);
                }
            }
            let bundled = dir.join("resources").join(NODE_BIN);
            if bundled.exists() {
                return Some(bundled);
            }
            // 폴백: 평평한 배치 (구버전 호환)
            let flat = dir.join(NODE_BIN);
            if flat.exists() {
                return Some(flat);
            }
        }
    }

    // 2. 시스템 PATH (개발 모드 전용)
    // 프로덕션에서는 PATH hijacking(CWE-426) 방지를 위해 번들 node 만 허용.
    #[cfg(debug_assertions)]
    {
        which::which("node").ok()
    }
    #[cfg(not(debug_assertions))]
    {
        tracing::warn!(
            "Bundled {} not found next to executable — \
             HWP/HWPX parsing disabled. Reinstall the application.",
            NODE_BIN
        );
        None
    }
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

/// kordoc 프로세스 실행 결과 — 성공/실패 판정과 출력 해석은 호출자 몫
/// (파싱은 stdout JSON, 렌더는 exit code + 출력 파일).
struct KordocOutput {
    status: std::process::ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

/// kordoc CLI 공용 러너 (blocking thread에서 사용)
///
/// spawn → Job Object 등록(이슈 #33: 고아 node 방지) → stdout/stderr drain 스레드
/// (파이프 블록 방지) → try_wait 폴링 타임아웃까지의 프로세스 안전장치를 모든
/// kordoc 서브커맨드가 공유한다 (std::process::Child는 Drop에서 kill하지 않으므로
/// 타임아웃 시 명시적 child.kill() 호출 필수).
fn run_kordoc_process(
    cli_path: &Path,
    args: &[std::ffi::OsString],
    timeout_secs: u64,
    file_display: &str,
) -> Result<KordocOutput, ParseError> {
    use std::io::Read;
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, Instant};

    let node = which_node()
        .ok_or_else(|| ParseError::ParseError("Node.js가 설치되지 않았습니다".to_string()))?;

    let mut cmd = std::process::Command::new(node);
    cmd.arg(cli_path.to_string_lossy().as_ref())
        .args(args)
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

    // 이슈 #33: 앱 종료/크래시 시 이 node 자식이 고아로 남지 않도록 Job Object 에 묶는다.
    crate::utils::process_job::track_child(&child);

    let timeout = Duration::from_secs(timeout_secs);

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
                        timeout_secs, file_display
                    );
                    return Err(ParseError::ParseError(format!(
                        "kordoc 타임아웃 ({}초 초과): {}",
                        timeout_secs, file_display
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
    let stdout = stdout_rx.recv_timeout(drain_timeout).unwrap_or_default();
    let stderr = stderr_rx.recv_timeout(drain_timeout).unwrap_or_default();

    Ok(KordocOutput {
        status,
        stdout,
        stderr,
    })
}

/// stderr 의 비어있지 않은 라인 전부를 " | " 로 합쳐 사용자 가시 에러 스니펫으로 (최대 300자).
/// 첫 줄만 잡으면 "FAIL" 같은 헤더에 진짜 진단 메시지가 묻힌다 (이슈 #22) —
/// 마지막 라인 부근에 가장 구체적인 에러가 나오는 경향이 있어 모두 보존한다.
fn stderr_snippet(stderr: &str) -> String {
    stderr
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" | ")
        .chars()
        .take(300)
        .collect()
}

/// kordoc CLI 동기 호출 — 파싱 경로 (`--format json --silent`), stdout 의 JSON 을 반환.
fn call_kordoc_sync(
    cli_path: &Path,
    file_path: &Path,
    opts: KordocOptions,
) -> Result<String, ParseError> {
    // Windows extended-length / UNC prefix 제거 (Node.js/kordoc가 처리하지 못함).
    // 단순 strip("\\?\\") 만 하면 \\?\UNC\server\share\... 가 UNC\server\... 로 깨지므로
    // dunce::simplified 로 \\srv\share\... 형태까지 정확히 복원한다.
    let file_owned = crate::utils::network_path::simplify(file_path);
    let file_str = file_owned.to_string_lossy();

    debug!(
        "kordoc: {} {} --format json --silent{}",
        cli_path.display(),
        file_str,
        if opts.formula_ocr {
            " --formula-ocr"
        } else {
            ""
        }
    );

    let mut args: Vec<std::ffi::OsString> = vec![
        file_str.as_ref().into(),
        "--format".into(),
        "json".into(),
        "--silent".into(),
    ];
    if opts.formula_ocr {
        args.push("--formula-ocr".into());
    }

    let timeout_secs = if opts.formula_ocr {
        KORDOC_FORMULA_TIMEOUT_SECS
    } else {
        KORDOC_TIMEOUT_SECS
    };

    let out = run_kordoc_process(
        cli_path,
        &args,
        timeout_secs,
        &file_path.display().to_string(),
    )?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        warn!("kordoc failed (exit {}): {}", out.status, stderr);
        // "이미지 기반 PDF"는 kordoc이 본문 없는 스캔 PDF를 만났을 때 내는 마커.
        // parse_file 이 OCR 여부 보고 Rust 재시도를 건너뛸 수 있게 에러 문자열에 태그 유지.
        if stderr.contains("이미지 기반 PDF") {
            return Err(ParseError::ParseError(format!(
                "kordoc: 이미지 기반 PDF (exit {})",
                out.status
            )));
        }
        let snippet = stderr_snippet(&stderr);
        return Err(ParseError::ParseError(if snippet.is_empty() {
            format!("kordoc 실행 실패 (exit {})", out.status)
        } else {
            format!("kordoc 실행 실패 (exit {}): {snippet}", out.status)
        }));
    }

    // kordoc 출력 크기 제한 (100MB — OOM 방지)
    const MAX_OUTPUT_SIZE: usize = 100 * 1024 * 1024;
    if out.stdout.len() > MAX_OUTPUT_SIZE {
        return Err(ParseError::ParseError(format!(
            "kordoc 출력 크기 초과: {}MB (최대 {}MB)",
            out.stdout.len() / 1_048_576,
            MAX_OUTPUT_SIZE / 1_048_576
        )));
    }

    let output = String::from_utf8(out.stdout)
        .map_err(|_| ParseError::ParseError("kordoc 출력이 유효한 UTF-8이 아닙니다".to_string()))?;

    // pdfjs-dist 등 외부 라이브러리가 stdout에 경고를 출력하는 경우
    // JSON 시작점을 찾아서 앞의 garbage를 제거 (예: "Warning: TT: ...")
    let json_start = output
        .find('{')
        .ok_or_else(|| ParseError::ParseError("kordoc 출력에 JSON이 없습니다".to_string()))?;
    if json_start > 0 {
        // 200번째 바이트가 멀티바이트 문자 중간이면 slice가 panic — 문자 경계로 내림
        let mut preview_end = json_start.min(200);
        while !output.is_char_boundary(preview_end) {
            preview_end -= 1;
        }
        debug!(
            "kordoc stdout에 JSON 앞 {}바이트 garbage 제거: {:?}",
            json_start,
            &output[..preview_end]
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

/// kordoc 마크다운의 HTML 표(`<table>…</table>`)를 검색 인덱스용 plain text 로 변환한다.
///
/// 행은 줄바꿈, 셀은 공백으로 직렬화하고 변환되지 못한(중첩 표 등) 잔여 표/줄바꿈 태그를
/// 제거한다. 표가 없으면 입력을 그대로 돌려준다(정규식 매칭 0 → 사실상 무비용).
/// 미리보기 패널은 `get_markdown` 원본(HTML 표 유지)을 쓰므로 GFM 렌더에는 영향이 없다.
fn html_tables_to_text(md: &str) -> String {
    use std::sync::OnceLock;
    static TABLE_RE: OnceLock<regex::Regex> = OnceLock::new();
    static ROW_RE: OnceLock<regex::Regex> = OnceLock::new();
    static CELL_RE: OnceLock<regex::Regex> = OnceLock::new();
    static INNER_TAG_RE: OnceLock<regex::Regex> = OnceLock::new();
    static LEFTOVER_RE: OnceLock<regex::Regex> = OnceLock::new();

    let table_re =
        TABLE_RE.get_or_init(|| regex::Regex::new(r"(?is)<table[^>]*>.*?</table>").unwrap());
    let row_re = ROW_RE.get_or_init(|| regex::Regex::new(r"(?is)<tr[^>]*>(.*?)</tr>").unwrap());
    let cell_re =
        CELL_RE.get_or_init(|| regex::Regex::new(r"(?is)<t[dh][^>]*>(.*?)</t[dh]>").unwrap());
    let inner_tag_re = INNER_TAG_RE.get_or_init(|| regex::Regex::new(r"(?is)<[^>]+>").unwrap());
    let leftover_re = LEFTOVER_RE.get_or_init(|| {
        regex::Regex::new(r"(?is)</?(?:table|thead|tbody|tfoot|tr|td|th|col|colgroup|br)[^>]*>")
            .unwrap()
    });

    let replaced = table_re.replace_all(md, |caps: &regex::Captures| {
        let table = &caps[0];
        let mut rows: Vec<String> = Vec::new();
        for row in row_re.captures_iter(table) {
            let mut cells: Vec<String> = Vec::new();
            for cell in cell_re.captures_iter(&row[1]) {
                let stripped = inner_tag_re.replace_all(&cell[1], " ");
                let cell_text = stripped.split_whitespace().collect::<Vec<_>>().join(" ");
                if !cell_text.is_empty() {
                    cells.push(cell_text);
                }
            }
            if !cells.is_empty() {
                rows.push(cells.join(" "));
            }
        }
        if rows.is_empty() {
            String::new()
        } else {
            format!("\n{}\n", rows.join("\n"))
        }
    });

    // 중첩 표 등으로 위 변환을 빠져나간 잔여 표/줄바꿈 태그 제거 (정규식은 중첩을 셀 수 없음).
    leftover_re.replace_all(&replaced, " ").into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_table_serialized_to_plain_text() {
        let md = "앞\n<table><tr><td>A</td><td colspan=\"2\">B</td></tr>\
                  <tr><td>C</td><td>D</td></tr></table>\n뒤";
        let out = html_tables_to_text(md);
        assert!(out.contains("A B"), "행1 셀이 공백 결합돼야: {out:?}");
        assert!(out.contains("C D"), "행2 셀이 공백 결합돼야: {out:?}");
        assert!(!out.contains("<td"), "td 태그 잔존: {out:?}");
        assert!(!out.contains("colspan"), "colspan 잔존: {out:?}");
        assert!(
            out.contains("앞") && out.contains("뒤"),
            "표 바깥 본문 보존: {out:?}"
        );
    }

    #[test]
    fn no_table_is_noop() {
        let md = "# 제목\n본문 텍스트 — GFM 표 아님 | 열1 | 열2";
        assert_eq!(html_tables_to_text(md), md);
    }

    #[test]
    fn leftover_table_tags_removed() {
        // 닫는 </table> 가 없어 표 단위 변환에 실패해도 잔여 태그는 정리된다.
        let md = "<tr><td>x</td><td>y</td>";
        let out = html_tables_to_text(md);
        assert!(
            !out.contains("<td") && !out.contains("<tr"),
            "잔여 태그 정리 실패: {out:?}"
        );
        assert!(
            out.contains('x') && out.contains('y'),
            "셀 텍스트 보존: {out:?}"
        );
    }

    #[test]
    fn th_header_and_inline_tags() {
        // <th> 헤더 셀 + 셀 내부 인라인 태그(<b>)도 텍스트만 남는다.
        let md = "<table><tr><th>구분</th><th>값</th></tr>\
                  <tr><td><b>합계</b></td><td>100</td></tr></table>";
        let out = html_tables_to_text(md);
        assert!(out.contains("구분 값"), "헤더 행: {out:?}");
        assert!(
            out.contains("합계 100"),
            "본문 행 + 인라인 태그 제거: {out:?}"
        );
        assert!(!out.contains('<'), "꺾쇠 잔존: {out:?}");
    }

    #[test]
    fn multiple_tables_with_br_and_between_text() {
        let md = "<table><tr><td>a</td></tr></table>중간<table><tr><td>b<br>c</td></tr></table>";
        let out = html_tables_to_text(md);
        assert!(
            out.contains('a') && out.contains('b') && out.contains('c') && out.contains("중간"),
            "내용 보존: {out:?}"
        );
        assert!(
            !out.contains("<br") && !out.contains("<td"),
            "태그 잔존: {out:?}"
        );
    }

    #[test]
    fn nested_table_leaves_no_tags() {
        // 셀 안 중첩 표는 non-greedy 매칭이 안쪽 </table> 에서 멈춰 바깥 잔여가 생기지만,
        // leftover 정리로 태그가 본문에 노출되지 않는다 (스니펫 노출 방어가 목적).
        let md = "<table><tr><td><table><tr><td>안</td></tr></table></td></tr></table>";
        let out = html_tables_to_text(md);
        assert!(out.contains('안'), "내부 셀 텍스트 보존: {out:?}");
        assert!(
            !out.contains("<table") && !out.contains("<td") && !out.contains("</"),
            "태그 잔존: {out:?}"
        );
    }

    #[test]
    fn empty_cells_skipped() {
        let md = "<table><tr><td></td><td>값</td><td>   </td></tr></table>";
        let out = html_tables_to_text(md);
        assert_eq!(out.trim(), "값", "빈 셀은 스킵: {out:?}");
    }
}
