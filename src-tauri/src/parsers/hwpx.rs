use super::{DocumentChunk, DocumentMetadata, ParseError, ParsedDocument};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use zip::ZipArchive;

/// HWPX 페이지 설정 (단위: hwpunit, 1pt = 100 hwpunit)
#[derive(Debug, Clone)]
struct PageSettings {
    /// 페이지 높이 (hwpunit)
    height: u32,
    /// 상단 여백 (hwpunit)
    top_margin: u32,
    /// 하단 여백 (hwpunit)
    bottom_margin: u32,
    /// 머리말 영역 (hwpunit)
    header_offset: u32,
    /// 꼬리말 영역 (hwpunit)
    footer_offset: u32,
}

impl Default for PageSettings {
    fn default() -> Self {
        // A4 기본값 (한글 기본 설정)
        Self {
            height: 84188,      // 약 297mm (A4)
            top_margin: 5668,   // 약 20mm
            bottom_margin: 4252, // 약 15mm
            header_offset: 4252,
            footer_offset: 4252,
        }
    }
}

/// 기본 스타일 정보
#[derive(Debug, Clone)]
struct DefaultStyle {
    /// 기본 글자 크기 (hwpunit, 1pt = 100)
    font_size: u32,
    /// 줄간격 (%, 예: 160 = 160%)
    line_spacing: u32,
}

impl Default for DefaultStyle {
    fn default() -> Self {
        Self {
            font_size: 1000,    // 10pt
            line_spacing: 160,  // 160%
        }
    }
}

/// 페이지 계산기
#[allow(dead_code)]
struct PageCalculator {
    /// 한 페이지에 들어가는 대략적인 글자 수
    chars_per_page: usize,
    /// 한 줄에 들어가는 대략적인 글자 수
    chars_per_line: usize,
    /// 한 페이지 줄 수
    lines_per_page: usize,
}

impl PageCalculator {
    fn new(page: &PageSettings, style: &DefaultStyle) -> Self {
        // 유효 높이 = 페이지 높이 - 상단여백 - 하단여백 - 머리말 - 꼬리말
        let effective_height = page.height
            .saturating_sub(page.top_margin)
            .saturating_sub(page.bottom_margin)
            .saturating_sub(page.header_offset)
            .saturating_sub(page.footer_offset);

        // 한 줄 높이 = 글자크기 × (줄간격 / 100)
        let line_height = (style.font_size * style.line_spacing) / 100;
        let line_height = line_height.max(100); // 최소 1pt

        // 한 페이지 줄 수
        let lines_per_page = (effective_height / line_height) as usize;
        let lines_per_page = lines_per_page.max(10); // 최소 10줄

        // 한 줄 글자 수 (A4 기준 약 40자 추정)
        let chars_per_line = 40_usize;

        // 한 페이지 글자 수
        let chars_per_page = lines_per_page * chars_per_line;

        Self {
            chars_per_page,
            chars_per_line,
            lines_per_page,
        }
    }

    /// 문자 오프셋으로 페이지 번호 계산 (1-based)
    fn page_for_offset(&self, char_offset: usize) -> usize {
        (char_offset / self.chars_per_page) + 1
    }

    /// 전체 글자 수로 총 페이지 수 계산
    fn total_pages(&self, total_chars: usize) -> usize {
        ((total_chars + self.chars_per_page - 1) / self.chars_per_page).max(1)
    }
}

/// HWPX 파일 파싱
/// HWPX는 OASIS ODF 기반 ZIP 포맷
/// 구조: Contents/section0.xml, section1.xml, ..., Contents/header.xml
pub fn parse(path: &Path) -> Result<ParsedDocument, ParseError> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut archive =
        ZipArchive::new(reader).map_err(|e| ParseError::ParseError(e.to_string()))?;

    // 1회 루프로 header.xml + section*.xml 모두 수집
    let mut header_content: Option<String> = None;
    let mut sections: BTreeMap<usize, (String, String)> = BTreeMap::new(); // (텍스트, 원본 XML)

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| ParseError::ParseError(e.to_string()))?;

        let name = file.name().to_string();

        // header.xml 수집
        if name == "Contents/header.xml" {
            let mut contents = String::new();
            std::io::Read::read_to_string(&mut file, &mut contents)?;
            header_content = Some(contents);
            continue;
        }

        // section XML 파일만 처리 (section0.xml, section1.xml, ...)
        if name.starts_with("Contents/section") && name.ends_with(".xml") {
            let section_num = name
                .trim_start_matches("Contents/section")
                .trim_end_matches(".xml")
                .parse::<usize>()
                .unwrap_or(0);

            let mut contents = String::new();
            std::io::Read::read_to_string(&mut file, &mut contents)?;

            let section_text = extract_text_from_hwpx_section(&contents)?;
            if !section_text.is_empty() {
                sections.insert(section_num, (section_text, contents));
            }
        }
    }

    // header.xml에서 기본 스타일 파싱
    let default_style = header_content
        .as_ref()
        .map(|c| parse_header_xml(c))
        .unwrap_or_default();

    // 3. 첫 번째 섹션에서 페이지 설정 파싱
    let page_settings = sections
        .values()
        .next()
        .map(|(_, xml)| parse_page_settings(xml))
        .unwrap_or_default();

    // 4. 페이지 계산기 생성
    let calculator = PageCalculator::new(&page_settings, &default_style);

    tracing::debug!(
        "HWPX page calc: {}자/페이지, {}줄/페이지, 폰트 {}hwpunit, 줄간격 {}%",
        calculator.chars_per_page,
        calculator.lines_per_page,
        default_style.font_size,
        default_style.line_spacing
    );

    // 5. 전체 텍스트 합치기
    let mut all_text = String::new();
    for (_, (section_text, _)) in &sections {
        if !all_text.is_empty() {
            all_text.push_str("\n\n");
        }
        all_text.push_str(section_text);
    }

    // 6. 페이지 계산 기반 청크 생성
    let chunks = chunk_text_with_calculator(&all_text, super::DEFAULT_CHUNK_SIZE, super::DEFAULT_CHUNK_OVERLAP, &calculator);
    let page_count = calculator.total_pages(all_text.chars().count());

    if all_text.is_empty() {
        tracing::warn!("HWPX file has no text content: {:?}", path);
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

/// 페이지 계산기 기반 청크 분할 (정확한 페이지 번호)
/// 메모리 최적화: Vec<char> 대신 바이트 오프셋 매핑 사용
fn chunk_text_with_calculator(
    text: &str,
    chunk_size: usize,
    overlap: usize,
    calculator: &PageCalculator,
) -> Vec<DocumentChunk> {
    let mut chunks = Vec::new();

    if text.is_empty() {
        return chunks;
    }

    // 바이트 오프셋만 저장 (Vec<char> 4bytes/char → Vec<usize> 8bytes/char이지만
    // 실제 문자 데이터 복사 없이 원본 text에서 직접 슬라이싱 가능)
    let byte_offsets: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
    let total_len = byte_offsets.len();

    let step = chunk_size.saturating_sub(overlap).max(1);
    let mut start = 0;

    while start < total_len {
        let end = (start + chunk_size).min(total_len);

        // 바이트 오프셋으로 직접 슬라이싱
        let byte_start = byte_offsets[start];
        let byte_end = if end < total_len {
            byte_offsets[end]
        } else {
            text.len()
        };

        // 청크 시작 위치 기준 페이지 번호 계산
        let page_number = calculator.page_for_offset(start);

        chunks.push(DocumentChunk {
            content: text[byte_start..byte_end].to_string(),
            start_offset: start,
            end_offset: end,
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

/// header.xml에서 기본 스타일 파싱
/// charPr의 fontSz, paraPr의 lineSpacing 추출
fn parse_header_xml(xml_content: &str) -> DefaultStyle {
    let mut reader = Reader::from_str(xml_content);
    reader.config_mut().trim_text(true);

    let mut style = DefaultStyle::default();
    let mut in_default_style = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let local_name = e.local_name();
                let name = std::str::from_utf8(local_name.as_ref()).unwrap_or("");

                // 기본 스타일 (바탕글) 찾기
                if name == "style" {
                    for attr in e.attributes().flatten() {
                        let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                        let val = std::str::from_utf8(&attr.value).unwrap_or("");
                        // 기본 스타일 ID 또는 이름으로 판별
                        if (key == "id" || key == "name") &&
                           (val == "0" || val.contains("바탕") || val.to_lowercase().contains("normal")) {
                            in_default_style = true;
                        }
                    }
                }

                // 글자 속성 (fontSz)
                if name == "charPr" && in_default_style {
                    for attr in e.attributes().flatten() {
                        let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                        let val = std::str::from_utf8(&attr.value).unwrap_or("");
                        if key == "fontSz" || key == "sz" {
                            if let Ok(sz) = val.parse::<u32>() {
                                style.font_size = sz;
                            }
                        }
                    }
                }

                // 문단 속성 (lineSpacing)
                if name == "lineSpacing" || name == "lnSpc" {
                    for attr in e.attributes().flatten() {
                        let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                        let val = std::str::from_utf8(&attr.value).unwrap_or("");
                        if key == "val" || key == "value" {
                            if let Ok(ls) = val.parse::<u32>() {
                                style.line_spacing = ls;
                            }
                        }
                    }
                }
            }
            Ok(Event::End(e)) => {
                let local_name = e.local_name();
                let name = std::str::from_utf8(local_name.as_ref()).unwrap_or("");
                if name == "style" {
                    in_default_style = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    style
}

/// section XML에서 페이지 설정 파싱
/// sec > pPr 또는 secPr 내의 width, height, margins 추출
fn parse_page_settings(xml_content: &str) -> PageSettings {
    let mut reader = Reader::from_str(xml_content);
    reader.config_mut().trim_text(true);

    let mut settings = PageSettings::default();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let local_name = e.local_name();
                let name = std::str::from_utf8(local_name.as_ref()).unwrap_or("");

                // 페이지 속성 태그들
                if name == "sz" || name == "pSz" || name == "pageSz" {
                    for attr in e.attributes().flatten() {
                        let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                        let val = std::str::from_utf8(&attr.value).unwrap_or("");
                        if key == "h" || key == "height" {
                            if let Ok(h) = val.parse::<u32>() {
                                settings.height = h;
                            }
                        }
                    }
                }

                // 여백 설정
                if name == "margin" || name == "pageMar" {
                    for attr in e.attributes().flatten() {
                        let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                        let val = std::str::from_utf8(&attr.value).unwrap_or("");
                        match key {
                            "top" | "t" => {
                                if let Ok(v) = val.parse::<u32>() {
                                    settings.top_margin = v;
                                }
                            }
                            "bottom" | "b" => {
                                if let Ok(v) = val.parse::<u32>() {
                                    settings.bottom_margin = v;
                                }
                            }
                            "header" => {
                                if let Ok(v) = val.parse::<u32>() {
                                    settings.header_offset = v;
                                }
                            }
                            "footer" => {
                                if let Ok(v) = val.parse::<u32>() {
                                    settings.footer_offset = v;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    settings
}
