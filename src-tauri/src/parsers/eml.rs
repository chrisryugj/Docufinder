use super::{chunk_text, DocumentMetadata, ParseError, ParsedDocument};
use mail_parser::{Address, MessageParser};
use std::fs;
use std::path::Path;

/// EML 파서 최대 파일 크기 (50MB) - 첨부 포함 메일 대비 + 메모리 보호
const MAX_EML_FILE_SIZE: u64 = 50 * 1024 * 1024;

/// .eml 메일 파일 파싱 (RFC822 MIME) — 이슈 #29.
///
/// 헤더(제목·보낸사람·받는사람·날짜)를 본문 앞에 붙여 검색 대상에 포함하고,
/// 본문은 text/plain 파트를 우선, 없으면 text/html 을 텍스트로 변환해 추출한다.
/// charset(EUC-KR/CP949 등) 디코딩과 quoted-printable/base64 해제는 mail-parser 가 처리.
/// 첨부파일 본문은 파싱하지 않는다(메일 본문 검색이 목적).
pub fn parse(path: &Path) -> Result<ParsedDocument, ParseError> {
    let file_size = fs::metadata(path)?.len();
    if file_size > MAX_EML_FILE_SIZE {
        return Err(ParseError::ParseError(format!(
            "File too large: {}MB (max {}MB)",
            file_size / 1024 / 1024,
            MAX_EML_FILE_SIZE / 1024 / 1024
        )));
    }

    let bytes = fs::read(path)?;
    let message = MessageParser::default()
        .parse(&bytes)
        .ok_or_else(|| ParseError::ParseError("EML 파싱 실패 (RFC822 형식 아님)".to_string()))?;

    let subject = message.subject().map(|s| s.to_string());
    let from = format_address(message.from());
    let to = format_address(message.to());
    let date = message.date();

    // 본문: text/plain 우선, 없으면 html → text 변환
    let body = message
        .body_text(0)
        .map(|c| c.into_owned())
        .or_else(|| {
            message
                .body_html(0)
                .map(|h| mail_parser::decoders::html::html_to_text(&h))
        })
        .unwrap_or_default();

    // 헤더를 본문 앞에 붙여 검색 대상에 포함 (제목·보낸사람·받는사람·날짜)
    let mut content = String::new();
    if let Some(s) = &subject {
        content.push_str("제목: ");
        content.push_str(s);
        content.push('\n');
    }
    if let Some(f) = &from {
        content.push_str("보낸사람: ");
        content.push_str(f);
        content.push('\n');
    }
    if let Some(t) = &to {
        content.push_str("받는사람: ");
        content.push_str(t);
        content.push('\n');
    }
    if let Some(d) = date {
        content.push_str("날짜: ");
        content.push_str(&d.to_string());
        content.push('\n');
    }
    if !content.is_empty() {
        content.push('\n');
    }
    content.push_str(&body);

    let chunks = chunk_text(
        &content,
        super::DEFAULT_CHUNK_SIZE,
        super::DEFAULT_CHUNK_OVERLAP,
    );

    Ok(ParsedDocument {
        content,
        metadata: DocumentMetadata {
            title: subject.or_else(|| path.file_stem().and_then(|s| s.to_str()).map(String::from)),
            author: from,
            created_at: date.map(|d| d.to_timestamp()),
            page_count: None,
        },
        chunks,
    })
}

/// 메일 주소 헤더를 "이름 <주소>, ..." 형태 문자열로 변환.
/// 그룹/리스트 모두 평탄화하고, 표시할 게 없으면 None.
fn format_address(addr: Option<&Address>) -> Option<String> {
    let addr = addr?;
    let parts: Vec<String> = addr
        .iter()
        .filter_map(|a| match (a.name(), a.address()) {
            (Some(n), Some(e)) => Some(format!("{} <{}>", n.trim(), e.trim())),
            (Some(n), None) => Some(n.trim().to_string()),
            (None, Some(e)) => Some(e.trim().to_string()),
            (None, None) => None,
        })
        .filter(|s| !s.is_empty())
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_eml(content: &[u8]) -> tempfile::NamedTempFile {
        let mut f = tempfile::Builder::new().suffix(".eml").tempfile().unwrap();
        f.write_all(content).unwrap();
        f.flush().unwrap();
        f
    }

    #[test]
    fn parses_headers_and_plain_body() {
        let raw = b"From: \"Hong Gildong\" <hong@example.com>\r\n\
To: receiver@example.com\r\n\
Subject: Test Mail Subject\r\n\
Date: Mon, 1 Jun 2026 09:00:00 +0900\r\n\
Content-Type: text/plain; charset=utf-8\r\n\
\r\n\
This is the mail body.\r\n";
        let f = write_eml(raw);
        let doc = parse(f.path()).unwrap();

        assert!(doc.content.contains("Test Mail Subject"));
        assert!(doc.content.contains("This is the mail body."));
        assert!(doc.content.contains("hong@example.com"));
        assert_eq!(doc.metadata.title.as_deref(), Some("Test Mail Subject"));
        assert!(doc
            .metadata
            .author
            .as_deref()
            .unwrap()
            .contains("hong@example.com"));
        assert!(doc.metadata.created_at.is_some());
        assert!(!doc.chunks.is_empty());
    }

    #[test]
    fn falls_back_to_html_body() {
        let raw = b"Subject: HTML only\r\n\
Content-Type: text/html; charset=utf-8\r\n\
\r\n\
<html><body><p>Hello <b>world</b></p></body></html>\r\n";
        let f = write_eml(raw);
        let doc = parse(f.path()).unwrap();
        assert!(doc.content.contains("Hello"));
        assert!(doc.content.contains("world"));
        // 태그는 제거돼야 함
        assert!(!doc.content.contains("<p>"));
    }

    #[test]
    fn title_falls_back_to_filename_when_no_subject() {
        let raw = b"From: a@b.com\r\n\r\nno subject body\r\n";
        let f = write_eml(raw);
        let doc = parse(f.path()).unwrap();
        // 제목 헤더가 없으면 파일명(stem)이 title 로
        assert!(doc.metadata.title.is_some());
        assert!(doc.content.contains("no subject body"));
    }
}
