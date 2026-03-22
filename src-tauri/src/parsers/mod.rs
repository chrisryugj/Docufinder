pub mod docx;
pub mod hwpx;
pub mod image_ocr;
pub mod pdf;
pub mod pptx;
pub mod txt;
pub mod xlsx;

use crate::ocr::OcrEngine;
use std::path::Path;
use thiserror::Error;
use zip::ZipArchive;

/// 기본 청크 크기 (문자 수)
/// 512 → 1024로 증가: 청크 수 50% 감소, DB 크기/검색 비용 절감
pub const DEFAULT_CHUNK_SIZE: usize = 1024;
/// 기본 청크 오버랩 (문자 수)
pub const DEFAULT_CHUNK_OVERLAP: usize = 128;

#[derive(Error, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum ParseError {
    #[error("Unsupported file type: {0}")]
    UnsupportedFileType(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Parse error: {0}")]
    ParseError(String),
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
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match extension.as_str() {
        "txt" | "md" => txt::parse(path),
        "hwpx" => hwpx::parse(path),
        "docx" => docx::parse(path),
        "pptx" => pptx::parse(path),
        "xlsx" | "xls" => {
            // calamine 라이브러리 내부 패닉 방지 (손상된 xls 파일 등)
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| xlsx::parse(path)))
                .unwrap_or_else(|_| {
                    tracing::error!("XLS/XLSX parser panicked: {:?}", path);
                    Err(ParseError::ParseError(format!(
                        "XLS/XLSX 파서 내부 오류 (파일 손상 가능): {}",
                        path.display()
                    )))
                })
        }
        "pdf" => pdf::parse(path),
        ext if ocr.is_some()
            && crate::constants::OCR_IMAGE_EXTENSIONS.contains(&ext) =>
        {
            image_ocr::parse(path, ocr.unwrap())
        }
        _ => Err(ParseError::UnsupportedFileType(extension)),
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
/// 최대 파일 크기 (200MB) - 8GB RAM PC OOM 방지
pub const MAX_FILE_SIZE: u64 = 200 * 1024 * 1024;

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

/// 텍스트를 청크로 분할 (공통 유틸)
pub fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> Vec<DocumentChunk> {
    let mut chunks = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let total_len = chars.len();

    if total_len == 0 {
        return chunks;
    }

    let step = chunk_size.saturating_sub(overlap).max(1);
    let mut start = 0;

    while start < total_len {
        let end = (start + chunk_size).min(total_len);
        let chunk_content: String = chars[start..end].iter().collect();

        chunks.push(DocumentChunk {
            content: chunk_content,
            start_offset: start,
            end_offset: end,
            page_number: None,
            page_end: None,
            location_hint: None,
        });

        start += step;

        if end >= total_len {
            break;
        }
    }

    chunks
}
