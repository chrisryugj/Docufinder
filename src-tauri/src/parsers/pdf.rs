use super::{DocumentChunk, DocumentMetadata, ParseError, ParsedDocument};
use std::path::Path;

/// PDF 파일 파싱
/// pdf-extract 크레이트 사용, 페이지별 텍스트 추출
pub fn parse(path: &Path) -> Result<ParsedDocument, ParseError> {
    let raw_text = pdf_extract::extract_text(path)
        .map_err(|e| ParseError::ParseError(format!("PDF extraction failed: {}", e)))?;

    // 페이지별 분리 (form feed 문자 \x0c 기준)
    let pages: Vec<&str> = raw_text.split('\x0c').collect();
    let page_count = pages.len();

    let mut all_text = String::new();
    let mut chunks = Vec::new();
    let mut global_offset = 0;

    for (page_idx, page_text) in pages.iter().enumerate() {
        let cleaned = clean_pdf_text(page_text);
        if cleaned.is_empty() {
            continue;
        }

        let page_number = page_idx + 1; // 1-based

        // 페이지별 청크 생성
        let page_chunks = chunk_text_with_page(&cleaned, 512, 64, page_number, global_offset);
        chunks.extend(page_chunks);

        if !all_text.is_empty() {
            all_text.push_str("\n\n");
            global_offset += 2;
        }
        global_offset += cleaned.len();
        all_text.push_str(&cleaned);
    }

    if all_text.is_empty() {
        tracing::warn!("PDF file has no text content: {:?}", path);
    }

    Ok(ParsedDocument {
        content: all_text,
        metadata: DocumentMetadata {
            title: path.file_stem().and_then(|s| s.to_str()).map(String::from),
            author: None,
            created_at: None,
            page_count: Some(page_count),
        },
        chunks,
    })
}

/// 페이지 정보 포함 청크 분할
fn chunk_text_with_page(
    text: &str,
    chunk_size: usize,
    overlap: usize,
    page_number: usize,
    base_offset: usize,
) -> Vec<DocumentChunk> {
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
            start_offset: base_offset + start,
            end_offset: base_offset + end,
            page_number: Some(page_number),
            location_hint: Some(format!("페이지 {}", page_number)),
        });

        start += step;
        if end >= total_len {
            break;
        }
    }

    chunks
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
