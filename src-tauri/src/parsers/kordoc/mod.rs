//! kordoc Node.js 사이드카 — HWP/HWPX/DOCX/PDF/XLSX 마크다운 변환
//!
//! `node kordoc/dist/cli.js <path> --format json --silent` 호출 후
//! JSON 응답을 ParsedDocument로 변환한다.
//! `render` 서브커맨드로 HWPX 첫 페이지를 조판 보존 SVG 로 렌더한다 (레이아웃 미리보기).

mod process;
mod render;
mod text;
mod worker;

use super::{
    chunk_text, DocumentMetadata, ParseError, ParsedDocument, DEFAULT_CHUNK_OVERLAP,
    DEFAULT_CHUNK_SIZE,
};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use tracing::{debug, warn};

use process::{
    call_kordoc_sync, run_kordoc_process, stderr_snippet, validate_file_size, which_node,
};
use text::{annotate_chunk_pages, html_tables_to_text, parse_iso_timestamp, strip_image_refs};

pub use render::render_svg;

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

/// kordoc 이 직접 파싱하는 이미지 확장자 (kordoc v4.2.1+ — 내장 OCR 자동 적용 + 표 괘선 복원).
/// bmp/tiff 는 kordoc 미지원 — 자체 OCR 엔진(image_ocr) 전용으로 남는다.
pub const KORDOC_IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp"];

/// kordoc 텍스트 OCR 모드 (kordoc v4.2.0+, PDF·이미지)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum KordocOcrMode {
    /// OCR 안 함 (기본)
    #[default]
    Off,
    /// `--ocr`: 품질 신호가 OCR 을 권하는 페이지만 인식 (스캔·글꼴 매핑 깨짐).
    /// 정상 PDF 에는 비용 0 (대상 페이지 없으면 모델 로드도 안 함).
    Auto,
    /// `--ocr-force`: 전 페이지 강제 재인식 ("OCR로 다시 읽기" 버튼용)
    Force,
}

/// kordoc 호출 옵션
#[derive(Debug, Clone, Default)]
pub struct KordocOptions {
    /// PDF 수식 OCR 활성화 (--formula-ocr)
    pub formula_ocr: bool,
    /// PDF 텍스트 OCR (내장 PP-OCRv5 korean — 첫 사용 시 모델 ~18MB 자동 다운로드)
    pub ocr: KordocOcrMode,
    /// 암호로 보호된 문서의 열기 암호 (kordoc v4.4.0+, HWPX·HWP3·HWP5).
    /// 인덱싱 경로는 항상 None — 사용자가 미리보기에서 입력했을 때만 전달한다.
    pub password: Option<String>,
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
        KordocOptions {
            formula_ocr: true,
            ..KordocOptions::default()
        }
    } else {
        KordocOptions::default()
    }
}

// ─── JSON 응답 구조체 ─────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct KordocResponse {
    success: bool,
    markdown: Option<String>,
    metadata: Option<KordocMetadata>,
    #[serde(default)]
    warnings: Vec<KordocWarning>,
    error: Option<String>,
    /// 실패 원인 코드 (kordoc v4.4.0+ — 실패 시에도 --format json 이 JSON 을 낸다).
    /// "ENCRYPTED" 면 열기 암호가 필요하거나 입력한 암호가 틀린 것.
    #[serde(default)]
    code: Option<String>,
    /// PDF 전용 — 텍스트층 부재 (kordoc v2.9+)
    #[serde(default)]
    is_image_based: Option<bool>,
    /// PDF 전용 — 페이지별 품질 신호 (kordoc v2.9+, ocrReason 은 v4.2 갱신)
    #[serde(default)]
    page_quality: Vec<KordocPageQuality>,
    /// 블록 IR — 청크 페이지 매핑에 사용 (pageNumber 는 kordoc v4.7.3+ 에서
    /// 한컴 저장본 실제 쪽 번호, PDF 는 원래 실제 페이지). 구버전 응답엔 없어도 무방.
    #[serde(default)]
    blocks: Vec<KordocBlock>,
}

/// kordoc IRBlock — 페이지 매핑에 필요한 필드만 역직렬화
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct KordocBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: Option<String>,
    page_number: Option<usize>,
    table: Option<KordocTable>,
}

/// kordoc IRTable — `rows`/`cols` 는 **개수(정수)** 이고 셀 데이터는 `cells` 다 (실 JSON 확인).
#[derive(Deserialize)]
struct KordocTable {
    #[serde(default)]
    cells: Vec<Vec<KordocCell>>,
}

#[derive(Deserialize)]
struct KordocCell {
    text: Option<String>,
}

/// 원본 텍스트층이 "깨진" 신호 — 스캔(low_text)은 제외 (복사할 텍스트 자체가 없음).
/// OCR 로 본문을 채웠어도 원본을 열어 복사하면 깨진다는 사실은 그대로라 유지.
/// vector_text(kordoc 4.14.4+): 글자를 곡선으로 그린 쪽 — 원본에서 복사하면 한글이 나오지 않는다.
fn text_layer_garbled(pages: &[KordocPageQuality]) -> bool {
    pages.iter().any(|p| {
        (p.needs_ocr || p.ocr_applied)
            && matches!(
                p.ocr_reason.as_deref(),
                Some(
                    "high_pua"
                        | "high_control"
                        | "high_replacement"
                        | "garbled_hangul"
                        | "vector_text"
                )
            )
    })
}

/// kordoc 페이지 품질 신호 (필요 필드만 역직렬화)
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct KordocPageQuality {
    #[serde(default)]
    needs_ocr: bool,
    /// "low_text" | "high_pua" | "high_control" | "high_replacement" | "garbled_hangul" | "vector_text"(kordoc 4.14.4+)
    ocr_reason: Option<String>,
    /// OCR 이 실제 적용되어 본문이 대체됨 (kordoc v4.2+)
    #[serde(default)]
    ocr_applied: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct KordocMetadata {
    title: Option<String>,
    author: Option<String>,
    created_at: Option<String>,
    page_count: Option<usize>,
    /// "layout"(실제 페이지 경계) | "section"(섹션 근사) — kordoc v4.7.3+ (#66).
    /// 구버전 응답엔 없음 → 페이지 매핑 스킵.
    page_mode: Option<String>,
}

#[derive(Deserialize)]
struct KordocWarning {
    message: String,
    /// 구조화 경고 코드 (NEEDS_OCR·OCR_APPLIED·OCR_FAILED 등, kordoc v3.0+)
    code: Option<String>,
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

    // 2. 개발 환경: 로컬 kordoc 프로젝트 (절대 경로만, debug 빌드 전용).
    //    맥 경로가 없으면 cargo test 의 kordoc e2e(암호 문서 등)가 번들을 못 찾아 조용히 건너뛴다.
    #[cfg(debug_assertions)]
    {
        let home = std::env::var("HOME").unwrap_or_default();
        for dev in [
            PathBuf::from(r"c:\github_project\kordoc\dist\cli.js"),
            PathBuf::from(home).join("workspace/kordoc/dist/cli.js"),
        ] {
            if dev.is_absolute() && dev.exists() {
                return Some(dev);
            }
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

    let json = call_kordoc_sync(&cli_path, path, opts, false)?;
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
        let msg = resp.error.unwrap_or_else(|| "kordoc 파싱 실패".to_string());
        // 암호 문서는 전용 에러로 — 호출부(미리보기)가 비밀번호 입력을 띄울 수 있어야 한다
        if resp.code.as_deref() == Some("ENCRYPTED") {
            return Err(ParseError::PasswordProtected(msg));
        }
        return Err(ParseError::ParseError(msg));
    }

    // 구조화 신호 (kordoc v4.2+): NEEDS_OCR/OCR_APPLIED 경고 코드 + 페이지 품질.
    let has_needs_ocr = resp
        .warnings
        .iter()
        .any(|w| w.code.as_deref() == Some("NEEDS_OCR"));
    let ocr_applied = resp
        .warnings
        .iter()
        .any(|w| w.code.as_deref() == Some("OCR_APPLIED"));
    let garbled_hint = text_layer_garbled(&resp.page_quality);

    let markdown = resp.markdown.unwrap_or_default();
    // kordoc 은 병합셀 표를 HTML <table> 로 반환한다. 그대로 인덱싱하면 검색 결과 스니펫에
    // td/tr/colspan 태그가 노출되므로 검색용 content/chunks 에선 표를 plain text 로 직렬화하고,
    // 이미지 참조(`![image](image_001.png)`)도 걷어낸다. 색인할 글자가 아니다.
    // 미리보기 패널은 get_markdown 원본을 써 표 렌더가 유지된다.
    let content = strip_image_refs(&html_tables_to_text(&markdown));
    if content.trim().is_empty() {
        // 이미지 기반 PDF 를 구조화 신호로 판별 — 종전 stderr 문자열 매칭 대체.
        // 스캔 페이지는 `![image](…)` 로 나와 markdown 자체는 비지 않는다(kordoc v4.0.8+).
        // 에러 문자열의 "이미지 기반 PDF" 태그는 parse_file 의 스킵 분기 계약.
        if resp.is_image_based == Some(true) || has_needs_ocr {
            return Err(ParseError::ParseError(
                "kordoc: 이미지 기반 PDF (NEEDS_OCR — 본문 없음)".to_string(),
            ));
        }
        if markdown.trim().is_empty() {
            return Err(ParseError::ParseError(
                "kordoc: 추출된 텍스트 없음".to_string(),
            ));
        }
        // 그림만 있는 문서: 실패가 아니라 색인할 글자가 없는 것 (파일명 검색은 된다).
    }

    if ocr_applied {
        tracing::info!("kordoc OCR 적용됨 [{}]", path.display());
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
        page_mode: None,
    });

    let metadata = DocumentMetadata {
        title: meta.title,
        author: meta.author,
        created_at: meta.created_at.and_then(|s| parse_iso_timestamp(&s)),
        page_count: meta.page_count,
    };

    let mut chunks = chunk_text(&content, DEFAULT_CHUNK_SIZE, DEFAULT_CHUNK_OVERLAP);

    // 실제 페이지 매핑 (#66, kordoc v4.7.3+) — pageMode="layout"(한컴 저장본·PDF·COM)일 때만
    // 블록 IR 의 쪽 번호를 content 오프셋에 사영해 청크에 "페이지 N" 힌트를 단다.
    // 섹션 근사(section)·구버전 응답은 종전대로 페이지 없음 (content/인덱스는 불변).
    if meta.page_mode.as_deref() == Some("layout") && !resp.blocks.is_empty() {
        annotate_chunk_pages(&mut chunks, &content, &resp.blocks);
    }

    Ok(ParsedDocument {
        content,
        metadata,
        chunks,
        garbled_hint,
    })
}

/// kordoc으로 파일의 full markdown만 추출 (미리보기용, 전역 formula OCR 토글 반영).
pub fn get_markdown(path: &Path) -> Result<String, ParseError> {
    get_markdown_with_options(path, options_for_path(path))
}

/// 열기 암호를 지정해 마크다운 추출 — 미리보기에서 사용자가 비밀번호를 입력했을 때.
/// 암호가 틀리면 ParseError::PasswordProtected 로 돌아온다.
pub fn get_markdown_with_password(path: &Path, password: &str) -> Result<String, ParseError> {
    get_markdown_with_options(
        path,
        KordocOptions {
            password: Some(password.to_string()),
            ..options_for_path(path)
        },
    )
}

/// get_markdown + 옵션 지정
pub fn get_markdown_with_options(path: &Path, opts: KordocOptions) -> Result<String, ParseError> {
    validate_file_size(path)?;

    let cli_path = find_kordoc_cli()
        .ok_or_else(|| ParseError::ParseError("kordoc CLI를 찾을 수 없습니다".to_string()))?;

    // 미리보기 — 워커가 모두 인덱싱 중이면 기다리지 않고 1회성 실행
    let json = call_kordoc_sync(&cli_path, path, opts, true)?;
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
        let msg = resp.error.unwrap_or_else(|| "kordoc 파싱 실패".to_string());
        // 암호 문서는 전용 에러로 — 호출부(미리보기)가 비밀번호 입력을 띄울 수 있어야 한다
        if resp.code.as_deref() == Some("ENCRYPTED") {
            return Err(ParseError::PasswordProtected(msg));
        }
        return Err(ParseError::ParseError(msg));
    }

    resp.markdown
        .filter(|m| !m.trim().is_empty())
        .ok_or_else(|| ParseError::ParseError("kordoc: 추출된 텍스트 없음".to_string()))
}

/// kordoc 사용 가능 여부
pub fn is_available() -> bool {
    find_kordoc_cli().is_some() && which_node().is_some()
}

/// 사이드카 실행 가능성 실측 — `node cli.js --version` 을 실제로 띄워 본다.
///
/// `is_available` 은 파일 존재만 보므로, 실행통제(AppLocker·매체제어 등)가 번들 `node.exe` 를
/// 막는 내부망 PC 나 node_modules 가 깨진 번들을 감지하지 못한다 — 그 경우 앱은 "가용" 으로
/// 보고하고 HWP·DOCX·PDF 가 파일마다 조용히 실패해 사용자에겐 "인덱싱이 안 된다" 로만 보인다.
/// `Ok(kordoc 버전)` / `Err(사용자에게 그대로 보여줄 원인·조치)`. 앱 시작 시 프론트가 1회 호출.
pub fn probe_runtime() -> Result<String, String> {
    let cli = find_kordoc_cli().ok_or_else(|| {
        format!("문서 변환기(kordoc)가 설치 폴더에 없습니다. 재설치가 필요합니다. {PROBE_HINT}")
    })?;
    if which_node().is_none() {
        return Err(format!(
            "문서 변환기 실행 파일(node.exe)이 설치 폴더에 없습니다. 재설치가 필요합니다. {PROBE_HINT}"
        ));
    }
    let out = run_kordoc_process(&cli, &["--version".into()], 15, "probe")
        .map_err(|e| probe_spawn_error_message(&e.to_string()))?;
    if !out.status.success() {
        let snippet = stderr_snippet(&String::from_utf8_lossy(&out.stderr));
        return Err(format!(
            "문서 변환기(kordoc)가 비정상 종료했습니다 (exit {}): {snippet}. 번들이 손상됐을 수 있어 재설치를 권장합니다. {PROBE_HINT}",
            out.status
        ));
    }
    let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
    process::remember_kordoc_version(&version);
    Ok(version)
}

const PROBE_HINT: &str =
    "허용 전까지 HWP·DOCX·PDF 본문은 인덱싱되지 않습니다 (txt·md·xlsx·pptx·eml 은 정상).";

/// spawn 실패 문자열 → 사용자 문구. ERROR_ACCESS_DENIED(5) / ERROR_ACCESS_DISABLED_BY_POLICY(1260)
/// 은 보안 정책이 프로세스 생성 자체를 거부한 것이라 "실행통제" 로 안내한다.
fn probe_spawn_error_message(raw: &str) -> String {
    if raw.contains("os error 5)") || raw.contains("os error 1260)") {
        format!(
            "문서 변환기(node.exe) 실행이 차단되었습니다 — 보안 정책(실행통제)이 설치 폴더의 node.exe 를 막고 있을 가능성이 큽니다. IT 부서에 허용을 요청하세요. {PROBE_HINT} ({raw})"
        )
    } else {
        format!("문서 변환기(kordoc) 점검 실패: {raw}. {PROBE_HINT}")
    }
}

/// 외부 모듈(commands::formula)에서 사이드카 경로 조회용으로 노출.
pub fn find_kordoc_cli_public() -> Option<PathBuf> {
    find_kordoc_cli()
}

/// 외부 모듈(commands::formula)에서 node 실행 경로 조회용으로 노출.
pub fn which_node_public() -> Option<PathBuf> {
    which_node()
}

#[cfg(test)]
mod tests;
