use super::{DocumentChunk, DocumentMetadata, ParseError, ParsedDocument};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::BTreeMap;
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

    // section 파일 정렬을 위해 BTreeMap 사용
    let mut sections: BTreeMap<usize, String> = BTreeMap::new();

    // Contents 폴더 내 section*.xml 파일들 파싱
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| ParseError::ParseError(e.to_string()))?;

        let name = file.name().to_string();

        // section XML 파일만 처리 (section0.xml, section1.xml, ...)
        if name.starts_with("Contents/section") && name.ends_with(".xml") {
            // section 번호 추출
            let section_num = name
                .trim_start_matches("Contents/section")
                .trim_end_matches(".xml")
                .parse::<usize>()
                .unwrap_or(0);

            let mut contents = String::new();
            std::io::Read::read_to_string(&mut file, &mut contents)?;

            let section_text = extract_text_from_hwpx_section(&contents)?;
            if !section_text.is_empty() {
                sections.insert(section_num, section_text);
            }
        }
    }

    let mut all_text = String::new();
    let mut chunks = Vec::new();
    let mut global_offset = 0;
    let section_count = sections.len();

    // 섹션 순서대로 처리 (0, 1, 2, ...)
    for (section_num, section_text) in sections {
        let page_number = section_num + 1; // 1-based 페이지 번호

        // 섹션별 청크 생성
        let section_chunks =
            chunk_text_with_page(&section_text, 512, 64, page_number, global_offset);
        chunks.extend(section_chunks);

        if !all_text.is_empty() {
            all_text.push_str("\n\n");
            global_offset += 2;
        }
        global_offset += section_text.len();
        all_text.push_str(&section_text);
    }

    if all_text.is_empty() {
        tracing::warn!("HWPX file has no text content: {:?}", path);
    }

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
            location_hint: Some(format!("섹션 {}", page_number)),
        });

        start += step;
        if end >= total_len {
            break;
        }
    }

    chunks
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
