use super::{chunk_text, DocumentMetadata, ParseError, ParsedDocument};
use std::path::Path;

/// PDF 파일 파싱
/// pdf-extract 크레이트 사용
pub fn parse(path: &Path) -> Result<ParsedDocument, ParseError> {
    let text = pdf_extract::extract_text(path)
        .map_err(|e| ParseError::ParseError(format!("PDF extraction failed: {}", e)))?;

    // 텍스트 정리 (연속된 공백 제거, 줄바꿈 정리)
    let cleaned_text = clean_pdf_text(&text);

    if cleaned_text.is_empty() {
        tracing::warn!("PDF file has no text content: {:?}", path);
    }

    // 청크 분할
    let chunks = chunk_text(&cleaned_text, 512, 64);

    Ok(ParsedDocument {
        content: cleaned_text,
        metadata: DocumentMetadata {
            title: path.file_stem().and_then(|s| s.to_str()).map(String::from),
            author: None,
            created_at: None,
            page_count: None,
        },
        chunks,
    })
}

/// PDF 텍스트 정리
fn clean_pdf_text(text: &str) -> String {
    let mut result = String::new();
    let mut prev_was_newline = false;

    for line in text.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            if !prev_was_newline && !result.is_empty() {
                result.push('\n');
                prev_was_newline = true;
            }
        } else {
            if !result.is_empty() && !prev_was_newline {
                result.push(' ');
            }
            result.push_str(trimmed);
            prev_was_newline = false;
        }
    }

    result.trim().to_string()
}
