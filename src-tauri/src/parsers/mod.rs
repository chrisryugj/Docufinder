pub mod docx;
pub mod eml;
pub mod hwpx;
pub mod image_ocr;
pub mod kordoc;
pub mod password_detect;
pub mod pdf;
pub mod pdf_sniff;
pub mod pptx;
pub mod rhwp;
pub mod txt;
#[cfg(windows)]
pub mod wincom;
pub mod xlsx;

use crate::ocr::OcrEngine;
#[cfg(windows)]
use crate::parsers::wincom::{docx as wincom_docx, pptx as wincom_pptx, xlsx as wincom_xlsx};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use thiserror::Error;
use zip::ZipArchive;

/// 연속 이미지 PDF 카운터 — 임계치 초과 시 같은 세션의 나머지 이미지 PDF 는
/// kordoc 호출 없이 즉시 스킵 (kordoc child 누적으로 인한 #17 크래시 방어).
/// 텍스트 PDF 가 정상 처리되면 0으로 리셋.
static SCANNED_PDF_STREAK: AtomicUsize = AtomicUsize::new(0);
const SCANNED_PDF_BREAKER_THRESHOLD: usize = 5;

/// breaker/sniff 가 "스캔 의심" 판정한 PDF 를 Rust 파서로 재확인할 때의 텍스트층 실재 판정(#45).
/// 쪽번호·워터마크 몇 글자뿐인 스캔본은 걸러내되, 오판이 데이터 유실로 이어지는 쪽(미달)이
/// 더 아프므로 임계는 낮게 잡는다.
fn substantial_pdf_text(doc: &ParsedDocument) -> bool {
    doc.content.chars().filter(|c| !c.is_whitespace()).count() >= 50
}

/// OCR off 인 PDF 의 kordoc 호출 전 스캔 의심 게이트(#45).
///
/// breaker 열림(연속 임계치 초과) 또는 sniff 스캔 판정 시 kordoc(node spawn)만 회피하고,
/// 즉시 실패하는 대신 Rust PDF 파서로 텍스트층을 먼저 확인한다:
/// - 종전 breaker 는 즉시 Err 라 성공(스트릭 리셋) 경로에 닿을 수 없는 **흡수 상태** —
///   스캔 PDF 5개 뒤의 정상 텍스트 PDF 까지 프로세스 재시작 전까지 전부 인덱싱 누락됐다.
/// - sniff 는 첫 64KB 의 비압축 마커만 보므로 텍스트 스트림이 압축된 정상 PDF 가
///   false positive 로 걸린다 — Rust 파서가 실제 텍스트를 뽑으면 그대로 인덱싱한다.
///
/// 반환: `None` = 스캔 의심 없음(kordoc 진행) / `Some(Ok)` = Rust 파서 결과로 인덱싱 +
/// 스트릭 리셋 / `Some(Err)` = 스캔 판정 스킵(사전 감지는 스트릭 증가).
fn pdf_scan_gate(path: &Path) -> Option<Result<ParsedDocument, ParseError>> {
    let streak = SCANNED_PDF_STREAK.load(Ordering::Relaxed);
    let breaker_open = streak >= SCANNED_PDF_BREAKER_THRESHOLD;
    if !breaker_open && !pdf_sniff::is_likely_scanned_pdf(path) {
        return None;
    }
    if let Ok(doc) = pdf::parse(path, None) {
        if substantial_pdf_text(&doc) {
            // 실제 텍스트층 존재 — 스캔 아님. 연속 스캔 상태도 여기서 해소되어
            // 다음 PDF 부터 kordoc(고품질 표 복원) 경로가 재개된다.
            SCANNED_PDF_STREAK.store(0, Ordering::Relaxed);
            return Some(Ok(doc));
        }
    }
    if breaker_open {
        return Some(Err(ParseError::ParseError(
            "이미지 기반 PDF (circuit breaker): kordoc 호출 회피".to_string(),
        )));
    }
    SCANNED_PDF_STREAK.fetch_add(1, Ordering::Relaxed);
    Some(Err(ParseError::ParseError(
        "이미지 기반 PDF (사전 감지): OCR 비활성 → 본문 추출 스킵".to_string(),
    )))
}

/// 기본 청크 크기 (문자 수)
/// 600자 ≈ 한국어 기준 ~400-480 토큰 → KoSimCSE 512 토큰 제한 내 수용
pub const DEFAULT_CHUNK_SIZE: usize = 600;
/// 기본 청크 오버랩 (문자 수, ~25% overlap)
pub const DEFAULT_CHUNK_OVERLAP: usize = 150;

#[derive(Error, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum ParseError {
    #[error("Unsupported file type: {0}")]
    UnsupportedFileType(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Password protected: {0}")]
    PasswordProtected(String),
    /// 클라우드 placeholder (OneDrive 등): 파일 본문이 로컬에 없음.
    /// 본문 파싱을 호출하면 OS 가 hydrate 를 트리거하므로 의도적으로 skip.
    #[error("Cloud placeholder (skip body parse): {0}")]
    CloudPlaceholder(String),
}

/// 파싱 결과
#[derive(Debug)]
pub struct ParsedDocument {
    pub content: String,
    pub metadata: DocumentMetadata,
    pub chunks: Vec<DocumentChunk>,
    /// 파서가 "원본 텍스트층이 깨졌다"고 판정한 힌트 (kordoc pageQuality 신호 —
    /// high_pua·garbled_hangul 등). OCR 로 본문을 채워 chunks 가 깨끗해져도 원본을
    /// 열어 복사하면 깨지므로, 파이프라인 garbled 판정에 OR 로 반영된다.
    pub garbled_hint: bool,
}

#[derive(Debug)]
pub struct DocumentMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub created_at: Option<i64>,
    pub page_count: Option<usize>,
}

#[derive(Debug)]
pub struct DocumentChunk {
    pub content: String,
    pub start_offset: usize,
    pub end_offset: usize,
    pub page_number: Option<usize>,
    /// 청크 끝 페이지 (page_number가 start_page, 이것이 end_page)
    pub page_end: Option<usize>,
    /// 위치 힌트 (XLSX: "Sheet1!A1:D50", PDF: "페이지 3", 등)
    pub location_hint: Option<String>,
}

/// 파일 확장자로 파서 선택 후 파싱.
///
/// 추출된 청크는 `utils::normalize_text` 로 정규화되어 검색 쿼리와 동일 정규형
/// (NFC·비가시 제거·공백 폴딩·줄바꿈 통일)으로 인덱싱된다. 쿼리측
/// (`sanitize_fts_query` 등)과 반드시 짝을 이뤄야 매칭이 일관된다. 기존 인덱스는
/// 비정규화 상태로 남으므로(점진 적용) 폴더 재인덱싱 시 정규화가 반영된다.
///
/// 인덱싱(유일한 호출 경로)은 chunks 만 소비하므로 전문 `content` 는 여기서 비운다 —
/// 유지하면 배치 파이프라인이 문서당 본문을 이중으로(전문+청크) 채널 버퍼 개수만큼
/// 들고 다녀 피크 메모리가 배가된다 (T2-5). 전문이 필요한 소비자가 생기면
/// `parse_file_inner` 기반 별도 진입점을 추가할 것.
///
/// `ocr`: OCR 엔진이 있으면 이미지 파일(jpg/png/webp/bmp/tiff)도 텍스트 추출 가능.
/// PDF 는 OCR 켜짐 시 kordoc `--ocr`(품질 신호 페이지만 정밀 인식, kordoc v4.2+)로
/// 위임된다 — 정상 PDF 에는 비용 0, kordoc 실패 시 Rust 파서+자체 OCR fallback.
/// 이미지(png/jpg/webp)도 OCR 켜짐 시 kordoc 직접 입력(v4.2.1+, 표 괘선 복원)을
/// 우선 시도하고 실패 시 자체 OCR 엔진으로 fallback (bmp/tiff 는 자체 엔진 전용).
pub fn parse_file(path: &Path, ocr: Option<&OcrEngine>) -> Result<ParsedDocument, ParseError> {
    let kordoc_ocr = if ocr.is_some() {
        kordoc::KordocOcrMode::Auto
    } else {
        kordoc::KordocOcrMode::Off
    };
    parse_file_normalized(path, ocr, kordoc_ocr)
}

/// "OCR로 다시 읽기" 전용 — PDF 를 kordoc `--ocr-force`(전 페이지 강제 재인식)로 파싱.
/// 품질 신호가 못 잡은 문서를 사용자가 명시적으로 재인식시킬 때 사용.
pub fn parse_file_force_ocr(
    path: &Path,
    ocr: Option<&OcrEngine>,
) -> Result<ParsedDocument, ParseError> {
    parse_file_normalized(path, ocr, kordoc::KordocOcrMode::Force)
}

fn parse_file_normalized(
    path: &Path,
    ocr: Option<&OcrEngine>,
    kordoc_ocr: kordoc::KordocOcrMode,
) -> Result<ParsedDocument, ParseError> {
    let mut doc = parse_file_inner(path, ocr, kordoc_ocr)?;
    doc.content = String::new();
    for chunk in &mut doc.chunks {
        chunk.content = crate::utils::normalize_text(&chunk.content);
    }
    Ok(doc)
}

/// `parse_file` 의 파서 디스패치 본체 (정규화 전 원본 텍스트 반환).
fn parse_file_inner(
    path: &Path,
    ocr: Option<&OcrEngine>,
    kordoc_ocr: kordoc::KordocOcrMode,
) -> Result<ParsedDocument, ParseError> {
    // breadcrumb: 어떤 파일이 처리 중인지 글로벌 추적 (panic / native crash 진단용).
    // RAII Guard 라 정상/패닉 양쪽 모두에서 자동 clear. 각 파서 내부에서 더 좁은 stage 로
    // 덮어쓸 수 있다 (예: parse_xlsx, parse_hwpx).
    let _bc = crate::breadcrumb::Guard::new(path, "parse_file");

    // 클라우드 placeholder 차단 — fs::read 류 호출이 Windows CldAPI 를 통해
    // 원본을 자동 다운로드(hydrate)해 인덱싱 사이드이펙트로 수백 GB 를 끌어오는 사고를 막는다.
    // 메타데이터(이름·크기·수정일)는 placeholder 에도 캐시되어 있어 호출자가 별도로 인덱싱 가능.
    if crate::utils::cloud_detect::is_cloud_placeholder(path) {
        return Err(ParseError::CloudPlaceholder(path.display().to_string()));
    }

    // 글로벌 토글이 켜져 있고 경로가 네트워크 드라이브/UNC 면 본문 파싱을 사전 차단.
    // NAVER Works · WebDAV · Drive for Desktop 가상드라이브 등은 placeholder 비트가 켜지지
    // 않지만 매 파일 read 마다 네트워크 라운드트립 또는 클라우드 다운로드를 유발한다.
    // 메타데이터만 인덱싱(파일명 검색은 동작) → 사용자가 의도적으로 본문이 필요하면
    // 설정에서 "클라우드/네트워크 본문 인덱싱" 토글을 켜야 한다.
    if crate::utils::cloud_detect::is_skip_enabled()
        && crate::utils::cloud_detect::is_network_path(path)
    {
        return Err(ParseError::CloudPlaceholder(path.display().to_string()));
    }

    // 암호 보호 파일 사전 감지 — kordoc(Node.js 사이드카) 호출 전에 차단해야
    // 내부에서 한컴/Office COM 이 시스템 모달 다이얼로그를 띄우는 사고를 막는다.
    // HWP/HWPX/DOCX/XLSX/PPTX/PDF 지원, 감지 실패 시 기존 파서 에러 기반 경로가 fallback.
    if password_detect::is_password_protected(path) {
        return Err(ParseError::PasswordProtected(path.display().to_string()));
    }

    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    // kordoc 지원 포맷: 먼저 kordoc 시도 → 실패 시 Rust 파서 fallback.
    // 실패한 kordoc 에러는 보존했다가 .hwp 처럼 Rust 파서가 없는 포맷에서 그대로 반환한다.
    // (이전엔 .hwp 가 fallback 진입 시 generic "Unsupported file type: hwp (kordoc 필요)" 로
    // 덮어써서 사용자가 진짜 원인을 알 수 없었다 — 이슈 #22 의 "kordoc 필요" false 메시지)
    let kordoc_formats = ["hwp", "hwpx", "docx", "pdf"];
    // 이미지(png/jpg/webp)는 OCR 옵션이 켜져 있을 때만 kordoc 직접 입력(v4.2.1+)으로
    // 우선 시도 — 표 괘선 복원 포함. 실패 시 아래 자체 OCR 엔진 분기가 fallback.
    let is_kordoc_image = kordoc::KORDOC_IMAGE_EXTENSIONS.contains(&extension.as_str())
        && (ocr.is_some() || kordoc_ocr != kordoc::KordocOcrMode::Off);
    let mut kordoc_err: Option<ParseError> = None;
    if (kordoc_formats.contains(&extension.as_str()) || is_kordoc_image) && kordoc::is_available() {
        // PDF 사전 sniff — 이미지 PDF + OCR off 인 경우 kordoc spawn 자체를 회피.
        // #17 크래시(0xc0000409) 의 강한 의심 원인: 스캔 PDF 다수 폴더에서 PDF 마다
        // node.exe 자식 spawn 누적 → 자식 프로세스/파이프/스레드 누수 → CRT 레벨
        // __fastfail. v2.5.6 의 사후 분기는 같은 파일 재시도만 막아 효과 미미했다.
        if extension == "pdf" && ocr.is_none() && kordoc_ocr == kordoc::KordocOcrMode::Off {
            if let Some(result) = pdf_scan_gate(path) {
                return result;
            }
        }

        // PDF + OCR 켜짐 → kordoc 내장 OCR 위임 (스캔·깨진 텍스트층 페이지만 정밀 인식,
        // 표 구조 복원 포함). 이미지는 kordoc 이 OCR 을 상시 적용하므로 모드를 그대로
        // 전달해 장시간 타임아웃만 취한다. 그 외 포맷은 ocr 플래그를 무시하므로 Off 로.
        let kordoc_opts = kordoc::KordocOptions {
            formula_ocr: extension == "pdf" && kordoc::is_formula_ocr_enabled(),
            ocr: if extension == "pdf" || is_kordoc_image {
                kordoc_ocr
            } else {
                kordoc::KordocOcrMode::Off
            },
            // 인덱싱 경로는 암호를 모른다 — 암호 문서는 사전 감지에서 이미 걸러진다
            password: None,
        };
        match kordoc::parse_with_options(path, kordoc_opts) {
            Ok(doc) => {
                if extension == "pdf" {
                    SCANNED_PDF_STREAK.store(0, Ordering::Relaxed);
                }
                return Ok(doc);
            }
            Err(e) => {
                // kordoc 사후 분기 (sniff 가 false negative 였을 때의 안전망).
                // kordoc OCR 을 시도한 경우(Auto/Force)는 스킵하지 않고 Rust 파서
                // (+자체 OCR)로 폴백한다 — kordoc OCR 모델 미설치/다운로드 실패 대비.
                if extension == "pdf"
                    && ocr.is_none()
                    && kordoc_ocr == kordoc::KordocOcrMode::Off
                    && e.to_string().contains("이미지 기반 PDF")
                {
                    SCANNED_PDF_STREAK.fetch_add(1, Ordering::Relaxed);
                    return Err(ParseError::ParseError(
                        "이미지 기반 PDF: OCR 비활성 상태에서 본문 추출 스킵".to_string(),
                    ));
                }
                tracing::warn!("kordoc fallback → Rust 파서: {} ({})", path.display(), e);
                kordoc_err = Some(e);
            }
        }
    }

    match extension.as_str() {
        "txt" | "md" => txt::parse(path),
        "eml" => eml::parse(path),
        // HWP5 바이너리: kordoc 전용 (Rust 파서 없음). kordoc 실제 에러를 그대로 반환해
        // 사용자가 "kordoc 필요"라는 잘못된 안내 대신 진짜 원인 (구버전 HWP3, 비표준 변종 등)을
        // 볼 수 있도록 한다 — 이슈 #22 진단 가시성 개선.
        "hwp" => Err(kordoc_err.unwrap_or_else(|| {
            if kordoc::is_available() {
                // kordoc 가 사용 가능한데도 에러가 None 이면 위 분기를 안 탔다는 뜻 — 이론상 도달 X.
                ParseError::ParseError("HWP 파싱 경로 비정상 진입".to_string())
            } else {
                ParseError::UnsupportedFileType("hwp (kordoc 필요)".to_string())
            }
        })),
        "hwpx" => parse_with_timeout(path, 30, "HWPX", hwpx::parse),
        "docx" => {
            let parse_result = parse_with_timeout(path, 30, "DOCX", docx::parse);
            match parse_result {
                Ok(doc) => Ok(doc),
                Err(err) => wincom_fallback_docx(path, err),
            }
        }
        "pptx" => {
            let parse_result = parse_with_timeout(path, 30, "PPTX", pptx::parse);
            match parse_result {
                Ok(doc) => Ok(doc),
                Err(err) => wincom_fallback_pptx(path, err),
            }
        }
        "xlsx" | "xls" => {
            let parse_result = parse_with_timeout(path, 15, "XLS/XLSX", xlsx::parse);
            match parse_result {
                Ok(doc) => Ok(doc),
                Err(err) => wincom_fallback_xlsx(path, err),
            }
        }
        "pdf" => pdf::parse(path, ocr),
        ext if crate::constants::OCR_IMAGE_EXTENSIONS.contains(&ext) => {
            match ocr {
                // 자체 OCR 엔진 fallback (bmp/tiff 는 이 경로 전용, webp 는 디코딩 불가로 실패)
                Some(engine) => image_ocr::parse(path, engine),
                // 엔진 없이 kordoc 만 시도한 경우(OCR로 다시 읽기 등) 실제 원인 보존
                None => Err(kordoc_err
                    .unwrap_or_else(|| ParseError::UnsupportedFileType(extension.clone()))),
            }
        }
        _ => Err(ParseError::UnsupportedFileType(extension)),
    }
}

// --- wincom fallback wrappers (Windows-only; no-op on other platforms) --------

#[cfg(windows)]
fn wincom_fallback_docx(path: &Path, err: ParseError) -> Result<ParsedDocument, ParseError> {
    if crate::constants::is_use_wincom_for_docx() {
        parse_with_timeout(path, 30, "DOCX", wincom_docx::parse)
    } else {
        Err(err)
    }
}

#[cfg(windows)]
fn wincom_fallback_pptx(path: &Path, err: ParseError) -> Result<ParsedDocument, ParseError> {
    if crate::constants::is_use_wincom_for_pptx() {
        parse_with_timeout(path, 30, "PPTX", wincom_pptx::parse)
    } else {
        Err(err)
    }
}

#[cfg(windows)]
fn wincom_fallback_xlsx(path: &Path, err: ParseError) -> Result<ParsedDocument, ParseError> {
    if crate::constants::is_use_wincom_for_xlsx() {
        parse_with_timeout(path, 30, "XLS/XLSX", wincom_xlsx::parse)
    } else {
        Err(err)
    }
}

#[cfg(not(windows))]
fn wincom_fallback_docx(_path: &Path, err: ParseError) -> Result<ParsedDocument, ParseError> {
    Err(err)
}

#[cfg(not(windows))]
fn wincom_fallback_pptx(_path: &Path, err: ParseError) -> Result<ParsedDocument, ParseError> {
    Err(err)
}

#[cfg(not(windows))]
fn wincom_fallback_xlsx(_path: &Path, err: ParseError) -> Result<ParsedDocument, ParseError> {
    Err(err)
}

/// 공통 타임아웃 + 패닉 방어 래퍼 (HWPX, DOCX, PPTX, XLSX 공통)
///
/// 별도 스레드에서 파서를 실행하고 `timeout_secs` 초 내 완료되지 않으면 에러 반환.
/// catch_unwind로 파서 내부 패닉도 안전하게 잡음.
fn parse_with_timeout<F>(
    path: &Path,
    timeout_secs: u64,
    label: &str,
    parse_fn: F,
) -> Result<ParsedDocument, ParseError>
where
    F: FnOnce(&Path) -> Result<ParsedDocument, ParseError> + Send + 'static,
{
    let path_owned = path.to_path_buf();
    let label_owned = label.to_string();
    let (tx, rx) = std::sync::mpsc::channel();

    let handle = std::thread::spawn(move || {
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| parse_fn(&path_owned)))
                .unwrap_or_else(|_| {
                    Err(ParseError::ParseError(format!(
                        "{} 파서 내부 오류 (파일 손상 가능): {}",
                        label_owned,
                        path_owned.display()
                    )))
                });
        let _ = tx.send(result);
    });

    match rx.recv_timeout(std::time::Duration::from_secs(timeout_secs)) {
        Ok(result) => {
            let _ = handle.join();
            result
        }
        Err(_) => {
            tracing::error!("{} parser timeout ({}s): {:?}", label, timeout_secs, path);
            // 클린업 스레드로 타임아웃된 파서 스레드 회수
            let label_for_log = label.to_string();
            let _ = std::thread::Builder::new()
                .name(format!("{}-cleanup", label.to_lowercase()))
                .stack_size(64 * 1024)
                .spawn(move || {
                    let _ = handle.join();
                    tracing::debug!("Timed-out {} thread reclaimed", label_for_log);
                });
            Err(ParseError::ParseError(format!(
                "{} 파싱 타임아웃 ({}초 초과): {}",
                label,
                timeout_secs,
                path.display()
            )))
        }
    }
}

// ============================================================================
// 압축 폭탄 방어 상수 (docx, hwpx 공통)
// ============================================================================

/// 단일 엔트리 최대 압축 해제 크기 (50MB)
pub const MAX_ENTRY_UNCOMPRESSED_SIZE: u64 = 50 * 1024 * 1024;
/// 전체 압축 해제 크기 합계 제한 (200MB)
pub const MAX_TOTAL_UNCOMPRESSED_SIZE: u64 = 200 * 1024 * 1024;
/// 최대 ZIP 엔트리 수
pub const MAX_ZIP_ENTRIES: usize = 1000;
/// 압축 비율 제한 (uncompressed/compressed > 100 = 의심)
pub const MAX_COMPRESSION_RATIO: u64 = 100;
/// 최대 파일 크기 (bytes) - 설정 max_file_size_mb 절대 상한과 동기화
/// 실제 필터링은 인덱서 파이프라인에서 설정값 기반으로 수행, 이 상수는 안전망
pub const MAX_FILE_SIZE: u64 = crate::constants::MAX_FILE_SIZE_LIMIT_MB * 1024 * 1024;

/// ZIP 아카이브 압축 폭탄 방어 검증 (docx, hwpx 공통)
pub fn validate_zip_archive<R: std::io::Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
) -> Result<(), ParseError> {
    if archive.len() > MAX_ZIP_ENTRIES {
        return Err(ParseError::ParseError(format!(
            "ZIP 엔트리 수 초과: {} (최대 {})",
            archive.len(),
            MAX_ZIP_ENTRIES
        )));
    }

    let mut total_uncompressed: u64 = 0;
    for i in 0..archive.len() {
        if let Ok(entry) = archive.by_index_raw(i) {
            let uncompressed = entry.size();
            let compressed = entry.compressed_size();

            if uncompressed > MAX_ENTRY_UNCOMPRESSED_SIZE {
                return Err(ParseError::ParseError(format!(
                    "ZIP 엔트리 크기 초과: {} bytes (최대 {} bytes) - {}",
                    uncompressed,
                    MAX_ENTRY_UNCOMPRESSED_SIZE,
                    entry.name()
                )));
            }

            if compressed > 0 && uncompressed / compressed > MAX_COMPRESSION_RATIO {
                return Err(ParseError::ParseError(format!(
                    "의심스러운 압축 비율: {}:1 - 압축 폭탄 가능성 ({})",
                    uncompressed / compressed,
                    entry.name()
                )));
            }

            total_uncompressed += uncompressed;
        }
    }

    if total_uncompressed > MAX_TOTAL_UNCOMPRESSED_SIZE {
        return Err(ParseError::ParseError(format!(
            "총 압축 해제 크기 초과: {} bytes (최대 {} bytes)",
            total_uncompressed, MAX_TOTAL_UNCOMPRESSED_SIZE
        )));
    }

    Ok(())
}

/// 텍스트를 청크로 분할 (문장 경계 인식)
///
/// chunk_size 근처의 문장 종결 위치(`.`, `!`, `?`, `\n\n`)에서 분할하여
/// 의미 단위가 깨지지 않도록 합니다.
pub fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> Vec<DocumentChunk> {
    let mut chunks = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let total_len = chars.len();

    if total_len == 0 {
        return chunks;
    }

    let mut start = 0;

    while start < total_len {
        let raw_end = (start + chunk_size).min(total_len);

        // 문장 경계 탐색: chunk_size의 80% ~ 100% 범위에서 마지막 문장 종결점 찾기
        let search_start = start + (chunk_size * 4 / 5).min(raw_end - start);
        let end = if raw_end < total_len {
            find_sentence_boundary(&chars, search_start, raw_end).unwrap_or(raw_end)
        } else {
            raw_end
        };

        let chunk_content: String = chars[start..end].iter().collect();

        chunks.push(DocumentChunk {
            content: chunk_content,
            start_offset: start,
            end_offset: end,
            page_number: None,
            page_end: None,
            location_hint: None,
        });

        // 다음 시작점: 문장 경계 기준으로 overlap 적용
        let next_start = if end > overlap { end - overlap } else { end };
        start = next_start.max(start + 1); // 무한루프 방지

        if end >= total_len {
            break;
        }
    }

    chunks
}

/// 문장 종결 경계 탐색 (search_start..limit 범위에서 마지막 종결점 반환)
fn find_sentence_boundary(chars: &[char], search_start: usize, limit: usize) -> Option<usize> {
    let mut best = None;
    let mut i = search_start;
    while i < limit {
        let c = chars[i];
        // 빈 줄(\n\n)은 가장 강한 경계
        if c == '\n' && i + 1 < limit && chars[i + 1] == '\n' {
            best = Some(i + 2);
            i += 2;
            continue;
        }
        // 문장 종결 문자 뒤에 공백이나 줄바꿈이 오는 경우
        if (c == '.' || c == '!' || c == '?' || c == '다' || c == '요') && i + 1 < chars.len() {
            let next = chars[i + 1];
            if next == ' ' || next == '\n' || next == '\r' {
                best = Some(i + 1);
            }
        }
        i += 1;
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::Mutex;

    /// SCANNED_PDF_STREAK 는 전역 static — 병렬 테스트 간 간섭을 막기 위해 직렬화한다.
    static STREAK_LOCK: Mutex<()> = Mutex::new(());

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    /// #45 회귀: breaker 가 열려 있어도 정상 텍스트 PDF 는 Rust 파서 경로로 인덱싱되고
    /// 스트릭이 리셋되어 흡수 상태가 해소된다 ("스캔 PDF 5개 뒤의 텍스트 PDF 가 인덱싱됨").
    #[test]
    fn scan_gate_breaker_open_indexes_text_pdf_and_resets_streak() {
        let _guard = STREAK_LOCK.lock().unwrap();
        SCANNED_PDF_STREAK.store(SCANNED_PDF_BREAKER_THRESHOLD, Ordering::Relaxed);
        let result = pdf_scan_gate(&fixture("multipage_text.pdf"))
            .expect("breaker open 이므로 게이트가 판정해야 함");
        let doc = result.expect("텍스트 PDF 는 breaker open 에서도 성공해야 함 (#45)");
        assert!(substantial_pdf_text(&doc), "본문 텍스트가 추출되어야 함");
        assert_eq!(
            SCANNED_PDF_STREAK.load(Ordering::Relaxed),
            0,
            "텍스트층 확인 시 스트릭 리셋 — 이후 PDF 는 kordoc 경로 재개"
        );
    }

    /// #45: breaker 가 열린 상태의 진짜 스캔 PDF 는 여전히 스킵(kordoc spawn 회피 의미 보존).
    #[test]
    fn scan_gate_breaker_open_still_skips_scanned_pdf() {
        let _guard = STREAK_LOCK.lock().unwrap();
        SCANNED_PDF_STREAK.store(SCANNED_PDF_BREAKER_THRESHOLD, Ordering::Relaxed);
        let result = pdf_scan_gate(&fixture("scanned_korean.pdf"))
            .expect("breaker open 이므로 게이트가 판정해야 함");
        let err = result.expect_err("텍스트층 없는 스캔 PDF 는 스킵 유지");
        assert!(
            err.to_string().contains("circuit breaker"),
            "breaker 사유가 보존되어야 함: {err}"
        );
        SCANNED_PDF_STREAK.store(0, Ordering::Relaxed);
    }

    /// breaker 닫힘 + 스캔 의심 없음 → 게이트는 관여하지 않는다(kordoc 진행).
    #[test]
    fn scan_gate_closed_text_pdf_passes_through() {
        let _guard = STREAK_LOCK.lock().unwrap();
        SCANNED_PDF_STREAK.store(0, Ordering::Relaxed);
        assert!(
            pdf_scan_gate(&fixture("multipage_text.pdf")).is_none(),
            "정상 텍스트 PDF 는 sniff 미검출 → kordoc 경로로 진행"
        );
    }
}
