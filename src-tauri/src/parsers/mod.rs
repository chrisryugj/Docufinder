pub mod docx;
pub mod hwpx;
pub mod image_ocr;
pub mod kordoc;
pub mod password_detect;
pub mod pdf;
pub mod pdf_sniff;
pub mod pptx;
pub mod txt;
pub mod xlsx;

use crate::ocr::OcrEngine;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use thiserror::Error;
use zip::ZipArchive;

/// 연속 이미지 PDF 카운터 — 임계치 초과 시 같은 세션의 나머지 이미지 PDF 는
/// kordoc 호출 없이 즉시 스킵 (kordoc child 누적으로 인한 #17 크래시 방어).
/// 텍스트 PDF 가 정상 처리되면 0으로 리셋.
static SCANNED_PDF_STREAK: AtomicUsize = AtomicUsize::new(0);
const SCANNED_PDF_BREAKER_THRESHOLD: usize = 5;

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

/// 파일 확장자로 파서 선택 후 파싱
///
/// `ocr`: OCR 엔진이 있으면 이미지 파일(jpg/png/bmp/tiff)도 텍스트 추출 가능
pub fn parse_file(path: &Path, ocr: Option<&OcrEngine>) -> Result<ParsedDocument, ParseError> {
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

    // kordoc 지원 포맷: 먼저 kordoc 시도 → 실패 시 Rust 파서 fallback
    let kordoc_formats = ["hwp", "hwpx", "docx", "pdf"];
    if kordoc_formats.contains(&extension.as_str()) && kordoc::is_available() {
        // PDF 사전 sniff — 이미지 PDF + OCR off 인 경우 kordoc spawn 자체를 회피.
        // #17 크래시(0xc0000409) 의 강한 의심 원인: 스캔 PDF 다수 폴더에서 PDF 마다
        // node.exe 자식 spawn 누적 → 자식 프로세스/파이프/스레드 누수 → CRT 레벨
        // __fastfail. v2.5.6 의 사후 분기는 같은 파일 재시도만 막아 효과 미미했다.
        if extension == "pdf" && ocr.is_none() {
            let streak = SCANNED_PDF_STREAK.load(Ordering::Relaxed);
            // Circuit breaker: 연속 임계치 초과 시 sniff 도 건너뛰고 즉시 스킵.
            if streak >= SCANNED_PDF_BREAKER_THRESHOLD {
                return Err(ParseError::ParseError(
                    "이미지 기반 PDF (circuit breaker): kordoc 호출 회피".to_string(),
                ));
            }
            if pdf_sniff::is_likely_scanned_pdf(path) {
                SCANNED_PDF_STREAK.fetch_add(1, Ordering::Relaxed);
                return Err(ParseError::ParseError(
                    "이미지 기반 PDF (사전 감지): OCR 비활성 → 본문 추출 스킵".to_string(),
                ));
            }
        }

        match kordoc::parse(path) {
            Ok(doc) => {
                if extension == "pdf" {
                    SCANNED_PDF_STREAK.store(0, Ordering::Relaxed);
                }
                return Ok(doc);
            }
            Err(e) => {
                // kordoc 사후 분기 (sniff 가 false negative 였을 때의 안전망).
                if extension == "pdf" && ocr.is_none() && e.to_string().contains("이미지 기반 PDF")
                {
                    SCANNED_PDF_STREAK.fetch_add(1, Ordering::Relaxed);
                    return Err(ParseError::ParseError(
                        "이미지 기반 PDF: OCR 비활성 상태에서 본문 추출 스킵".to_string(),
                    ));
                }
                tracing::warn!("kordoc fallback → Rust 파서: {} ({})", path.display(), e);
            }
        }
    }

    match extension.as_str() {
        "txt" | "md" => txt::parse(path),
        // HWP5 바이너리: kordoc 전용 (Rust 파서 없음, 위에서 이미 시도했으면 여기서 에러)
        "hwp" => Err(ParseError::UnsupportedFileType(
            "hwp (kordoc 필요)".to_string(),
        )),
        "hwpx" => parse_with_timeout(path, 30, "HWPX", hwpx::parse),
        "docx" => parse_with_timeout(path, 30, "DOCX", docx::parse),
        "pptx" => parse_with_timeout(path, 30, "PPTX", pptx::parse),
        "xlsx" | "xls" => parse_with_timeout(path, 15, "XLS/XLSX", xlsx::parse),
        "pdf" => pdf::parse(path, ocr),
        ext if ocr.is_some() && crate::constants::OCR_IMAGE_EXTENSIONS.contains(&ext) => {
            image_ocr::parse(path, ocr.unwrap())
        }
        _ => Err(ParseError::UnsupportedFileType(extension)),
    }
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
