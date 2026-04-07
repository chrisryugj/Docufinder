//! 이미지 파일 OCR 파서 (JPG, PNG, BMP, TIFF)

use super::{chunk_text, DocumentMetadata, ParseError, ParsedDocument};
use crate::ocr::OcrEngine;
use std::path::Path;

/// 기본 청크 크기/오버랩 (parsers/mod.rs 기본값과 통일 — KoSimCSE 512토큰 제한 이내)
const DEFAULT_CHUNK_SIZE: usize = 600;
const DEFAULT_CHUNK_OVERLAP: usize = 150;

/// 이미지 파일에서 OCR로 텍스트 추출
pub fn parse(path: &Path, ocr: &OcrEngine) -> Result<ParsedDocument, ParseError> {
    let result = ocr
        .recognize_file(path)
        .map_err(|e| ParseError::ParseError(format!("OCR 실패: {}", e)))?;

    let text = result.text.trim().to_string();
    if text.is_empty() {
        return Err(ParseError::ParseError(
            "OCR 결과가 비어있습니다".to_string(),
        ));
    }

    let chunks = chunk_text(&text, DEFAULT_CHUNK_SIZE, DEFAULT_CHUNK_OVERLAP);

    Ok(ParsedDocument {
        content: text,
        metadata: DocumentMetadata {
            title: path
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string()),
            author: None,
            created_at: None,
            page_count: Some(1),
        },
        chunks,
    })
}
