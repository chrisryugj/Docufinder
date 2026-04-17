use super::{DocumentChunk, DocumentMetadata, ParseError, ParsedDocument};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use zip::ZipArchive;

use super::{DEFAULT_CHUNK_OVERLAP, DEFAULT_CHUNK_SIZE, MAX_FILE_SIZE};

/// PPTX 파일 파싱
/// PPTX는 OOXML 기반 ZIP 포맷
/// 구조: ppt/slides/slide1.xml, slide2.xml, ...
/// 텍스트 태그: <a:t> (DrawingML namespace)
/// 노트: ppt/notesSlides/notesSlide1.xml (선택 추출)
pub fn parse(path: &Path) -> Result<ParsedDocument, ParseError> {
    // 파일 크기 체크
    if let Ok(metadata) = std::fs::metadata(path) {
        if metadata.len() > MAX_FILE_SIZE {
            return Err(ParseError::ParseError(format!(
                "PPTX 파일 크기 초과: {}MB (최대 {}MB)",
                metadata.len() / 1024 / 1024,
                MAX_FILE_SIZE / 1024 / 1024
            )));
        }
    }

    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut archive = ZipArchive::new(reader).map_err(|e| ParseError::ParseError(e.to_string()))?;

    // 압축 폭탄 방어
    super::validate_zip_archive(&mut archive)?;

    // 슬라이드 파일명 수집 + 정렬
    let mut slide_names: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        if let Ok(entry) = archive.by_index_raw(i) {
            let name = entry.name().to_string();
            if name.starts_with("ppt/slides/slide") && name.ends_with(".xml") {
                slide_names.push(name);
            }
        }
    }
    // slide1.xml, slide2.xml, ... 순서로 정렬 (숫자 기준)
    slide_names.sort_by_key(|name| extract_slide_number(name));

    // 슬라이드별 텍스트 추출
    let mut slides: Vec<SlideText> = Vec::new();
    for name in &slide_names {
        let slide_num = extract_slide_number(name);
        let text = extract_text_from_xml_entry(&mut archive, name)?;
        if !text.is_empty() {
            slides.push(SlideText {
                slide_number: slide_num,
                text,
            });
        }
    }

    // 노트 텍스트도 추출 (선택)
    let mut note_names: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        if let Ok(entry) = archive.by_index_raw(i) {
            let name = entry.name().to_string();
            if name.starts_with("ppt/notesSlides/notesSlide") && name.ends_with(".xml") {
                note_names.push(name);
            }
        }
    }
    note_names.sort_by_key(|name| extract_note_number(name));

    for name in &note_names {
        let note_num = extract_note_number(name);
        let text = extract_text_from_xml_entry(&mut archive, name)?;
        if !text.is_empty() {
            // 해당 슬라이드에 노트 텍스트 병합
            if let Some(slide) = slides.iter_mut().find(|s| s.slide_number == note_num) {
                slide.text.push_str("\n[노트] ");
                slide.text.push_str(&text);
            }
        }
    }

    if slides.is_empty() {
        tracing::warn!("PPTX file has no text content: {:?}", path);
    }

    // 전체 텍스트 + 청크 생성
    let mut all_text = String::new();
    let mut chunks = Vec::new();
    let mut global_offset = 0;

    for slide in &slides {
        if !all_text.is_empty() {
            all_text.push('\n');
            global_offset += 1;
        }

        let slide_chunks = chunk_slide(
            &slide.text,
            slide.slide_number,
            global_offset,
            DEFAULT_CHUNK_SIZE,
            DEFAULT_CHUNK_OVERLAP,
        );

        all_text.push_str(&slide.text);
        global_offset += slide.text.chars().count();
        chunks.extend(slide_chunks);
    }

    Ok(ParsedDocument {
        content: all_text,
        metadata: DocumentMetadata {
            title: path.file_stem().and_then(|s| s.to_str()).map(String::from),
            author: None,
            created_at: None,
            page_count: if slides.len() > 1 {
                Some(slides.len())
            } else {
                None
            },
        },
        chunks,
    })
}

struct SlideText {
    slide_number: usize,
    text: String,
}

/// 슬라이드 번호 추출: "ppt/slides/slide3.xml" → 3
fn extract_slide_number(name: &str) -> usize {
    name.trim_start_matches("ppt/slides/slide")
        .trim_end_matches(".xml")
        .parse::<usize>()
        .unwrap_or(0)
}

/// 노트 번호 추출: "ppt/notesSlides/notesSlide3.xml" → 3
fn extract_note_number(name: &str) -> usize {
    name.trim_start_matches("ppt/notesSlides/notesSlide")
        .trim_end_matches(".xml")
        .parse::<usize>()
        .unwrap_or(0)
}

/// ZIP 내 XML 엔트리에서 <a:t> 태그 텍스트 추출
fn extract_text_from_xml_entry<R: std::io::Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    entry_name: &str,
) -> Result<String, ParseError> {
    let entry = match archive.by_name(entry_name) {
        Ok(e) => e,
        Err(_) => return Ok(String::new()),
    };

    let mut xml_content = String::new();
    // .take()로 디컴프레션 크기 제한 (ZIP 헤더 위조 시 OOM 방어)
    {
        use std::io::Read;
        entry
            .take(super::MAX_ENTRY_UNCOMPRESSED_SIZE)
            .read_to_string(&mut xml_content)?;
    }

    let mut reader = Reader::from_str(&xml_content);
    reader.config_mut().trim_text(true);

    let mut result = String::new();
    let mut current_paragraph = String::new();
    let mut in_text = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let local = e.local_name();
                let name = std::str::from_utf8(local.as_ref()).unwrap_or("");
                if name == "t" {
                    in_text = true;
                }
            }
            Ok(Event::Text(e))
                if in_text => {
                    if let Ok(text) = e.unescape() {
                        current_paragraph.push_str(&text);
                    }
                }
            Ok(Event::End(e)) => {
                let local = e.local_name();
                let name = std::str::from_utf8(local.as_ref()).unwrap_or("");
                if name == "t" {
                    in_text = false;
                }
                // <a:p> 종료 = 문단 끝
                if name == "p" && !current_paragraph.is_empty() {
                    if !result.is_empty() {
                        result.push('\n');
                    }
                    result.push_str(&current_paragraph);
                    current_paragraph.clear();
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                tracing::warn!("PPTX XML parse error in {}: {}", entry_name, e);
                break;
            }
            _ => {}
        }
    }

    // 마지막 문단 처리
    if !current_paragraph.is_empty() {
        if !result.is_empty() {
            result.push('\n');
        }
        result.push_str(&current_paragraph);
    }

    Ok(result)
}

/// 슬라이드 단위 청크 분할 (슬라이드 번호 유지)
fn chunk_slide(
    text: &str,
    slide_number: usize,
    global_offset: usize,
    chunk_size: usize,
    overlap: usize,
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
            start_offset: global_offset + start,
            end_offset: global_offset + end,
            page_number: Some(slide_number),
            page_end: Some(slide_number),
            location_hint: Some(format!("슬라이드 {}", slide_number)),
        });

        start += step;
        if end >= total_len {
            break;
        }
    }

    chunks
}
