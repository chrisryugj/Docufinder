use super::{chunk_text, DocumentMetadata, ParseError, ParsedDocument};
use std::fs;
use std::path::Path;

/// TXT/MD 파일 파싱
pub fn parse(path: &Path) -> Result<ParsedDocument, ParseError> {
    let content = fs::read_to_string(path)?;

    // 청크 분할 (512자, 64자 오버랩)
    let chunks = chunk_text(&content, 512, 64);

    Ok(ParsedDocument {
        content,
        metadata: DocumentMetadata {
            title: path.file_stem().and_then(|s| s.to_str()).map(String::from),
            author: None,
            created_at: None,
            page_count: None,
        },
        chunks,
    })
}
