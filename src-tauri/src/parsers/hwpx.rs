use super::{chunk_text, DocumentMetadata, ParseError, ParsedDocument};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use zip::ZipArchive;

/// HWPX 파일 파싱
/// HWPX는 OASIS ODF 기반 ZIP 포맷
/// 구조: Contents/section0.xml, section1.xml, ...
pub fn parse(path: &Path) -> Result<ParsedDocument, ParseError> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut archive =
        ZipArchive::new(reader).map_err(|e| ParseError::ParseError(e.to_string()))?;

    let mut all_text = String::new();
    let mut section_count = 0;

    // Contents 폴더 내 section*.xml 파일들 파싱
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| ParseError::ParseError(e.to_string()))?;

        let name = file.name().to_string();

        // section XML 파일만 처리
        if name.starts_with("Contents/section") && name.ends_with(".xml") {
            let mut contents = String::new();
            std::io::Read::read_to_string(&mut file, &mut contents)?;

            let section_text = extract_text_from_hwpx_section(&contents)?;
            if !section_text.is_empty() {
                if !all_text.is_empty() {
                    all_text.push_str("\n\n");
                }
                all_text.push_str(&section_text);
                section_count += 1;
            }
        }
    }

    if all_text.is_empty() {
        tracing::warn!("HWPX file has no text content: {:?}", path);
    }

    // 청크 분할
    let chunks = chunk_text(&all_text, 512, 64);

    Ok(ParsedDocument {
        content: all_text,
        metadata: DocumentMetadata {
            title: path.file_stem().and_then(|s| s.to_str()).map(String::from),
            author: None,
            created_at: None,
            page_count: Some(section_count),
        },
        chunks,
    })
}

/// HWPX section XML에서 텍스트 추출
fn extract_text_from_hwpx_section(xml_content: &str) -> Result<String, ParseError> {
    let mut reader = Reader::from_str(xml_content);
    reader.config_mut().trim_text(true);

    let mut text_parts: Vec<String> = Vec::new();
    let mut current_paragraph = String::new();
    let mut in_text = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let local_name = e.local_name();
                let name = std::str::from_utf8(local_name.as_ref()).unwrap_or("");

                // hp:t 태그 = 텍스트 내용
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
                // p 태그 종료 = 문단 끝
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
