use super::{DocumentChunk, DocumentMetadata, ParseError, ParsedDocument};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use zip::ZipArchive;

// ============================================================================
// 압축 폭탄 방어 상수
// ============================================================================
const MAX_ENTRY_UNCOMPRESSED_SIZE: u64 = 50 * 1024 * 1024;
const MAX_TOTAL_UNCOMPRESSED_SIZE: u64 = 200 * 1024 * 1024;
const MAX_ZIP_ENTRIES: usize = 1000;
const MAX_COMPRESSION_RATIO: u64 = 100;

/// DOCX 파일 파싱
/// DOCX는 OOXML 기반 ZIP 포맷
/// 구조: word/document.xml
/// 페이지 break (<w:br w:type="page"/>) 감지 지원
pub fn parse(path: &Path) -> Result<ParsedDocument, ParseError> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut archive =
        ZipArchive::new(reader).map_err(|e| ParseError::ParseError(e.to_string()))?;

    // ========================================================================
    // 압축 폭탄 방어
    // ========================================================================
    validate_zip_archive(&mut archive)?;

    // word/document.xml 파일 읽기
    let mut document_xml = archive
        .by_name("word/document.xml")
        .map_err(|e| ParseError::ParseError(format!("document.xml not found: {}", e)))?;

    let mut contents = String::new();
    std::io::Read::read_to_string(&mut document_xml, &mut contents)?;

    let (pages, total_text) = extract_text_with_pages(&contents)?;

    if total_text.is_empty() {
        tracing::warn!("DOCX file has no text content: {:?}", path);
    }

    // 페이지별 청크 생성
    let chunks = chunk_pages(&pages, super::DEFAULT_CHUNK_SIZE, super::DEFAULT_CHUNK_OVERLAP);
    let page_count = pages.len();

    Ok(ParsedDocument {
        content: total_text,
        metadata: DocumentMetadata {
            title: path.file_stem().and_then(|s| s.to_str()).map(String::from),
            author: None,
            created_at: None,
            page_count: if page_count > 1 { Some(page_count) } else { None },
        },
        chunks,
    })
}

/// 페이지별 텍스트 정보
struct PageText {
    page_number: usize,
    text: String,
    start_offset: usize,
}

/// DOCX document.xml에서 페이지별 텍스트 추출
/// <w:br w:type="page"/> 태그로 페이지 구분
fn extract_text_with_pages(xml_content: &str) -> Result<(Vec<PageText>, String), ParseError> {
    let mut reader = Reader::from_str(xml_content);
    reader.config_mut().trim_text(true);

    let mut pages: Vec<PageText> = Vec::new();
    let mut current_page_text = String::new();
    let mut current_page = 1;
    let mut current_paragraph = String::new();
    let mut in_text = false;
    let mut total_offset = 0;
    let mut page_start_offset = 0;

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
            Ok(Event::Empty(e)) => {
                let local_name = e.local_name();
                let name = std::str::from_utf8(local_name.as_ref()).unwrap_or("");

                // <w:br w:type="page"/> 페이지 브레이크 감지
                if name == "br" {
                    let is_page_break = e.attributes().any(|attr| {
                        if let Ok(attr) = attr {
                            let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                            let val = std::str::from_utf8(&attr.value).unwrap_or("");
                            (key == "type" || key == "w:type") && val == "page"
                        } else {
                            false
                        }
                    });

                    if is_page_break {
                        // 현재 문단 추가 후 페이지 저장
                        if !current_paragraph.is_empty() {
                            if !current_page_text.is_empty() {
                                current_page_text.push('\n');
                            }
                            current_page_text.push_str(&current_paragraph);
                            total_offset += current_paragraph.chars().count() + 1;
                            current_paragraph.clear();
                        }

                        if !current_page_text.is_empty() {
                            pages.push(PageText {
                                page_number: current_page,
                                text: current_page_text.clone(),
                                start_offset: page_start_offset,
                            });
                        }
                        current_page += 1;
                        current_page_text.clear();
                        page_start_offset = total_offset;
                    }
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
                    if !current_page_text.is_empty() {
                        current_page_text.push('\n');
                        total_offset += 1;
                    }
                    current_page_text.push_str(&current_paragraph);
                    total_offset += current_paragraph.chars().count();
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

    // 마지막 문단/페이지 처리
    if !current_paragraph.is_empty() {
        if !current_page_text.is_empty() {
            current_page_text.push('\n');
        }
        current_page_text.push_str(&current_paragraph);
    }
    if !current_page_text.is_empty() {
        pages.push(PageText {
            page_number: current_page,
            text: current_page_text,
            start_offset: page_start_offset,
        });
    }

    // 전체 텍스트 생성
    let total_text = pages.iter().map(|p| p.text.as_str()).collect::<Vec<_>>().join("\n");

    Ok((pages, total_text))
}

/// 페이지별로 청크 분할 (페이지 번호 유지)
fn chunk_pages(pages: &[PageText], chunk_size: usize, overlap: usize) -> Vec<DocumentChunk> {
    let mut chunks = Vec::new();

    for page in pages {
        let chars: Vec<char> = page.text.chars().collect();
        let total_len = chars.len();

        if total_len == 0 {
            continue;
        }

        let step = chunk_size.saturating_sub(overlap).max(1);
        let mut start = 0;

        while start < total_len {
            let end = (start + chunk_size).min(total_len);
            let chunk_content: String = chars[start..end].iter().collect();

            chunks.push(DocumentChunk {
                content: chunk_content,
                start_offset: page.start_offset + start,
                end_offset: page.start_offset + end,
                page_number: Some(page.page_number),
                page_end: Some(page.page_number),
                location_hint: Some(format!("페이지 {}", page.page_number)),
            });

            start += step;

            if end >= total_len {
                break;
            }
        }
    }

    chunks
}

/// ZIP 아카이브 압축 폭탄 방어 검증
fn validate_zip_archive<R: std::io::Read + std::io::Seek>(archive: &mut ZipArchive<R>) -> Result<(), ParseError> {
    // 엔트리 수 제한
    if archive.len() > MAX_ZIP_ENTRIES {
        return Err(ParseError::ParseError(format!(
            "ZIP 엔트리 수 초과: {} (최대 {})",
            archive.len(), MAX_ZIP_ENTRIES
        )));
    }

    // 총 uncompressed size 검증
    let mut total_uncompressed: u64 = 0;
    for i in 0..archive.len() {
        if let Ok(entry) = archive.by_index_raw(i) {
            let uncompressed = entry.size();
            let compressed = entry.compressed_size();

            if uncompressed > MAX_ENTRY_UNCOMPRESSED_SIZE {
                return Err(ParseError::ParseError(format!(
                    "ZIP 엔트리 크기 초과: {} bytes (최대 {} bytes)",
                    uncompressed, MAX_ENTRY_UNCOMPRESSED_SIZE
                )));
            }

            if compressed > 0 && uncompressed / compressed > MAX_COMPRESSION_RATIO {
                return Err(ParseError::ParseError(format!(
                    "의심스러운 압축 비율: {}:1 - 압축 폭탄 가능성",
                    uncompressed / compressed
                )));
            }

            total_uncompressed += uncompressed;
        }
    }

    if total_uncompressed > MAX_TOTAL_UNCOMPRESSED_SIZE {
        return Err(ParseError::ParseError(format!(
            "총 압축 해제 크기 초과: {} bytes (최대 {} bytes)",
            total_uncompressed, MAX_TOTAL_UNCOMPRESSED_SIZE
        )));
    }

    Ok(())
}
