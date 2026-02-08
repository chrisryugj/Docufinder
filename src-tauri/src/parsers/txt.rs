use super::{chunk_text, DocumentMetadata, ParseError, ParsedDocument};
use std::fs;
use std::path::Path;

/// TXT 파서 최대 파일 크기 (50MB) - 메모리 안전성 보호
const MAX_TXT_FILE_SIZE: u64 = 50 * 1024 * 1024;

/// TXT/MD 파일 파싱
pub fn parse(path: &Path) -> Result<ParsedDocument, ParseError> {
    // 파일 크기 체크 (대용량 파일 메모리 보호)
    let file_size = fs::metadata(path)?.len();
    if file_size > MAX_TXT_FILE_SIZE {
        return Err(ParseError::ParseError(format!(
            "File too large: {}MB (max {}MB)",
            file_size / 1024 / 1024,
            MAX_TXT_FILE_SIZE / 1024 / 1024
        )));
    }

    let content = fs::read_to_string(path)?;

    // 청크 분할
    let chunks = chunk_text(&content, super::DEFAULT_CHUNK_SIZE, super::DEFAULT_CHUNK_OVERLAP);

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
