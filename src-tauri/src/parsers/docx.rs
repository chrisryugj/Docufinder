use super::{chunk_text, DocumentMetadata, ParseError, ParsedDocument};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use zip::ZipArchive;

/// DOCX 파일 파싱
/// DOCX는 OOXML 기반 ZIP 포맷
/// 구조: word/document.xml
pub fn parse(path: &Path) -> Result<ParsedDocument, ParseError> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut archive =
        ZipArchive::new(reader).map_err(|e| ParseError::ParseError(e.to_string()))?;

    // word/document.xml 파일 읽기
    let mut document_xml = archive
        .by_name("word/document.xml")
        .map_err(|e| ParseError::ParseError(format!("document.xml not found: {}", e)))?;

    let mut contents = String::new();
    std::io::Read::read_to_string(&mut document_xml, &mut contents)?;

    let text = extract_text_from_docx(&contents)?;

    if text.is_empty() {
        tracing::warn!("DOCX file has no text content: {:?}", path);
    }

    // 청크 분할
    let chunks = chunk_text(&text, 512, 64);

    Ok(ParsedDocument {
        content: text,
        metadata: DocumentMetadata {
            title: path.file_stem().and_then(|s| s.to_str()).map(String::from),
            author: None,
            created_at: None,
            page_count: None,
        },
        chunks,
    })
}

/// DOCX document.xml에서 텍스트 추출
fn extract_text_from_docx(xml_content: &str) -> Result<String, ParseError> {
    let mut reader = Reader::from_str(xml_content);
    reader.config_mut().trim_text(true);

    let mut text_parts: Vec<String> = Vec::new();
    let mut current_paragraph = String::new();
    let mut in_text = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let local_name = e.local_name();
                let name = std::str::from_utf8(local_name.as_ref()).unwrap_or("");

                // w:t 태그 = 텍스트 내용
                if name == "t" {
                    in_text = true;
                }
            }
            Ok(Event::Text(e)) => {
                if in_text {
                    let text = e
                        .unescape()
                        .map_err(|e| ParseError::ParseError(e.to_string()))?;
                    current_paragraph.push_str(&text);
                }
            }
            Ok(Event::End(e)) => {
                let local_name = e.local_name();
                let name = std::str::from_utf8(local_name.as_ref()).unwrap_or("");

                if name == "t" {
                    in_text = false;
                }
                // w:p 태그 종료 = 문단 끝
                if name == "p" && !current_paragraph.is_empty() {
                    text_parts.push(current_paragraph.clone());
                    current_paragraph.clear();
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                tracing::warn!("XML parse error: {}", e);
                break;
            }
            _ => {}
        }
    }

    // 마지막 문단 처리
    if !current_paragraph.is_empty() {
        text_parts.push(current_paragraph);
    }

    Ok(text_parts.join("\n"))
}
