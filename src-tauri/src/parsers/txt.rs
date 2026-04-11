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

    // UTF-8 먼저 시도, 실패 시 EUC-KR/CP949 감지 폴백
    let content = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => {
            let bytes = fs::read(path)?;
            decode_with_fallback(&bytes)
                .ok_or_else(|| ParseError::ParseError(
                    "인코딩을 인식할 수 없습니다 (UTF-8, EUC-KR 모두 실패)".to_string(),
                ))?
        }
    };

    // 청크 분할
    let chunks = chunk_text(
        &content,
        super::DEFAULT_CHUNK_SIZE,
        super::DEFAULT_CHUNK_OVERLAP,
    );

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

/// EUC-KR(CP949) 등 non-UTF-8 인코딩 감지 + 디코딩
fn decode_with_fallback(bytes: &[u8]) -> Option<String> {
    // EUC-KR (한국 관공서 문서에서 가장 흔한 non-UTF-8 인코딩)
    let (decoded, _, had_errors) = encoding_rs::EUC_KR.decode(bytes);
    if !had_errors {
        return Some(decoded.into_owned());
    }

    // 에러가 있어도 대부분 디코딩 가능하면 사용
    // (일부 CP949 확장 문자가 EUC-KR에 없을 수 있으나 대체 문자로 처리)
    let (decoded, _, _) = encoding_rs::EUC_KR.decode(bytes);
    let decoded_str = decoded.into_owned();

    // 대체 문자(U+FFFD) 비율이 10% 미만이면 수용
    let total_chars = decoded_str.chars().count();
    if total_chars == 0 {
        return None;
    }
    let replacement_count = decoded_str.chars().filter(|&c| c == '\u{FFFD}').count();
    if replacement_count * 10 < total_chars {
        Some(decoded_str)
    } else {
        None
    }
}
