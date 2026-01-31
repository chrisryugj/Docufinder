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
    /// 페이지 너비 (hwpunit)
    width: u32,
    /// 상단 여백 (hwpunit)
    top_margin: u32,
    /// 하단 여백 (hwpunit)
    bottom_margin: u32,
    /// 좌측 여백 (hwpunit)
    left_margin: u32,
    /// 우측 여백 (hwpunit)
    right_margin: u32,
    /// 머리말 영역 (hwpunit)
    header_offset: u32,
    /// 꼬리말 영역 (hwpunit)
    footer_offset: u32,
}

impl Default for PageSettings {
    fn default() -> Self {
        // A4 기본값 (한글 기본 설정)
        Self {
            height: 84188,       // 약 297mm (A4)
            width: 59528,        // 약 210mm (A4)
            top_margin: 5668,    // 약 20mm
            bottom_margin: 4252, // 약 15mm
            left_margin: 4252,   // 약 15mm
            right_margin: 4252,  // 약 15mm
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

/// 문단 노드 (구조적 파싱용)
#[derive(Debug, Clone)]
struct ParagraphNode {
    /// 문단 텍스트
    text: String,
    /// 전체 텍스트 내 시작 오프셋 (글자 수 기준)
    char_offset: usize,
    /// 이 문단 앞에 강제 쪽 나눔이 있는지
    has_page_break_before: bool,
}

/// 가상 레이아웃 시뮬레이터 (Y좌표 추적 기반)
struct LayoutSimulator {
    /// 현재 페이지 번호 (1-based)
    current_page: usize,
    /// 현재 Y 위치 (hwpunit)
    current_y: f32,
    /// 페이지 유효 높이 (hwpunit)
    max_height: f32,
    /// 페이지 유효 너비 (hwpunit)
    effective_width: f32,
    /// 줄 높이 (hwpunit)
    line_height: f32,
    /// 글자 크기 (hwpunit)
    font_size: f32,
}

impl LayoutSimulator {
    fn new(page: &PageSettings, style: &DefaultStyle) -> Self {
        // 유효 높이 = 페이지 높이 - 상단여백 - 하단여백 - 머리말 - 꼬리말
        let max_height = page
            .height
            .saturating_sub(page.top_margin)
            .saturating_sub(page.bottom_margin)
            .saturating_sub(page.header_offset)
            .saturating_sub(page.footer_offset) as f32;

        // 유효 너비 = 페이지 너비 - 좌측여백 - 우측여백
        let effective_width = page
            .width
            .saturating_sub(page.left_margin)
            .saturating_sub(page.right_margin) as f32;

        // 줄 높이 = 글자크기 × (줄간격 / 100)
        let font_size = style.font_size as f32;
        let line_height = (font_size * style.line_spacing as f32 / 100.0).max(100.0);

        Self {
            current_page: 1,
            current_y: 0.0,
            max_height,
            effective_width,
            line_height,
            font_size,
        }
    }

    /// 문단 처리 후 해당 문단의 페이지 번호 반환
    fn process_paragraph(&mut self, para: &ParagraphNode) -> usize {
        // 1. 강제 쪽 나눔 체크
        if para.has_page_break_before {
            self.current_page += 1;
            self.current_y = 0.0;
        }

        // 빈 문단은 빈 줄로 처리
        if para.text.trim().is_empty() {
            let para_height = self.line_height;
            if self.current_y + para_height > self.max_height {
                self.current_page += 1;
                self.current_y = para_height;
            } else {
                self.current_y += para_height;
            }
            return self.current_page;
        }

        // 2. 문단 높이 계산 (가중치 기반 줄 수)
        let lines = self.estimate_lines(&para.text);
        let para_height = lines as f32 * self.line_height;

        // 3. 페이지 넘침 체크
        if self.current_y + para_height > self.max_height {
            // 문단이 남은 공간보다 큰 경우
            if para_height > self.max_height {
                // 문단이 페이지 전체보다 큰 경우 → 여러 페이지에 걸침
                let remaining_height = self.max_height - self.current_y;
                let lines_in_current = (remaining_height / self.line_height) as usize;
                let remaining_lines = lines.saturating_sub(lines_in_current);

                if lines_in_current > 0 {
                    self.current_y = self.max_height;
                }

                // 다음 페이지로
                self.current_page += 1;

                // 남은 줄 수로 추가 페이지 계산
                let lines_per_page = (self.max_height / self.line_height) as usize;
                if lines_per_page > 0 && remaining_lines > 0 {
                    let extra_pages = (remaining_lines.saturating_sub(1)) / lines_per_page;
                    self.current_page += extra_pages;
                    self.current_y = ((remaining_lines % lines_per_page) as f32) * self.line_height;
                    if self.current_y == 0.0 && remaining_lines > 0 {
                        self.current_y = self.max_height;
                    }
                }
            } else {
                // 다음 페이지로 넘어감
                self.current_page += 1;
                self.current_y = para_height;
            }
        } else {
            self.current_y += para_height;
        }

        self.current_page
    }

    /// 가중치 기반 줄 수 계산 (한글 2.0, 영문 1.0)
    fn estimate_lines(&self, text: &str) -> usize {
        let text_width = calculate_weighted_width(text) * self.font_size * 0.5;
        let lines = (text_width / self.effective_width).ceil() as usize;
        lines.max(1) // 최소 1줄
    }

    /// 현재까지의 총 페이지 수
    fn total_pages(&self) -> usize {
        self.current_page
    }
}

/// 텍스트의 가중치 기반 폭 계산
/// 한글/한자/전각: 2.0, ASCII: 1.0
fn calculate_weighted_width(text: &str) -> f32 {
    text.chars()
        .map(|c| if c.is_ascii() { 1.0 } else { 2.0 })
        .sum()
}

/// HWPX 파일 파싱 (Virtual Layout Simulation 적용)
/// HWPX는 OASIS ODF 기반 ZIP 포맷
/// 구조: Contents/section0.xml, section1.xml, ..., Contents/header.xml
pub fn parse(path: &Path) -> Result<ParsedDocument, ParseError> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut archive =
        ZipArchive::new(reader).map_err(|e| ParseError::ParseError(e.to_string()))?;

    // 1회 루프로 header.xml + section*.xml 모두 수집
    let mut header_content: Option<String> = None;
    let mut section_xmls: BTreeMap<usize, String> = BTreeMap::new();

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
            section_xmls.insert(section_num, contents);
        }
    }

    // header.xml에서 기본 스타일 파싱
    let default_style = header_content
        .as_ref()
        .map(|c| parse_header_xml(c))
        .unwrap_or_default();

    // 첫 번째 섹션에서 페이지 설정 파싱
    let page_settings = section_xmls
        .values()
        .next()
        .map(|xml| parse_page_settings(xml))
        .unwrap_or_default();

    // 모든 섹션에서 문단 추출 (구조적 파싱)
    let mut all_paragraphs: Vec<ParagraphNode> = Vec::new();
    let mut total_char_offset: usize = 0;

    for (section_idx, xml) in &section_xmls {
        let mut section_paras = extract_paragraphs_from_section(xml)?;

        // 섹션 간 구분: 첫 번째가 아닌 섹션은 페이지 브레이크로 처리
        if *section_idx > 0 && !section_paras.is_empty() {
            section_paras[0].has_page_break_before = true;
        }

        // 오프셋 조정
        for para in &mut section_paras {
            para.char_offset += total_char_offset;
        }

        // 전체 오프셋 업데이트
        if let Some(last) = section_paras.last() {
            total_char_offset = last.char_offset + last.text.chars().count() + 1;
        }

        all_paragraphs.extend(section_paras);
    }

    // Layout Simulator로 각 문단의 페이지 번호 계산
    let mut simulator = LayoutSimulator::new(&page_settings, &default_style);
    let mut para_pages: Vec<usize> = Vec::with_capacity(all_paragraphs.len());

    for para in &all_paragraphs {
        let page = simulator.process_paragraph(para);
        para_pages.push(page);
    }

    let page_count = simulator.total_pages();

    tracing::debug!(
        "HWPX layout sim: {} 문단, {} 페이지, 폰트 {}hwpunit, 줄간격 {}%",
        all_paragraphs.len(),
        page_count,
        default_style.font_size,
        default_style.line_spacing
    );

    // 전체 텍스트 생성
    let all_text: String = all_paragraphs
        .iter()
        .map(|p| p.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    // 청크 생성 (시뮬레이션 결과 기반)
    let chunks = chunk_with_paragraph_pages(
        &all_text,
        &all_paragraphs,
        &para_pages,
        super::DEFAULT_CHUNK_SIZE,
        super::DEFAULT_CHUNK_OVERLAP,
    );

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

/// 문단별 페이지 정보 기반 청크 분할 (Virtual Layout Simulation 결과 활용)
fn chunk_with_paragraph_pages(
    text: &str,
    paragraphs: &[ParagraphNode],
    para_pages: &[usize],
    chunk_size: usize,
    overlap: usize,
) -> Vec<DocumentChunk> {
    let mut chunks = Vec::new();

    if text.is_empty() || paragraphs.is_empty() {
        return chunks;
    }

    // 바이트 오프셋 매핑
    let byte_offsets: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
    let total_len = byte_offsets.len();

    // 문자 오프셋 → 페이지 번호 매핑 생성
    let char_to_page = CharToPageMap::new(paragraphs, para_pages);

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

        // 청크 범위의 페이지 계산 (시작~끝)
        let start_page = char_to_page.get(start).unwrap_or(1);
        let end_page = char_to_page.get(end.saturating_sub(1)).unwrap_or(start_page);

        // location_hint에 페이지 범위 표시
        let location_hint = if start_page == end_page {
            format!("페이지 {}", start_page)
        } else {
            format!("페이지 {}-{}", start_page, end_page)
        };

        chunks.push(DocumentChunk {
            content: text[byte_start..byte_end].to_string(),
            start_offset: start,
            end_offset: end,
            page_number: Some(start_page),
            location_hint: Some(location_hint),
        });

        start += step;
        if end >= total_len {
            break;
        }
    }

    chunks
}

/// 문자 오프셋으로 페이지 번호를 찾는 구조체
struct CharToPageMap {
    /// (시작 오프셋, 페이지 번호)
    ranges: Vec<(usize, usize)>,
}

impl CharToPageMap {
    fn new(paragraphs: &[ParagraphNode], para_pages: &[usize]) -> Self {
        let mut ranges: Vec<(usize, usize)> = paragraphs
            .iter()
            .zip(para_pages.iter())
            .map(|(para, &page)| (para.char_offset, page))
            .collect();

        // 오프셋 기준 정렬
        ranges.sort_by_key(|(offset, _)| *offset);

        Self { ranges }
    }

    /// 문자 오프셋에 해당하는 페이지 번호 반환
    fn get(&self, char_offset: usize) -> Option<usize> {
        if self.ranges.is_empty() {
            return None;
        }

        // 이진 검색으로 해당 오프셋이 속한 문단 찾기
        match self
            .ranges
            .binary_search_by_key(&char_offset, |(offset, _)| *offset)
        {
            Ok(idx) => Some(self.ranges[idx].1),
            Err(idx) => {
                if idx == 0 {
                    Some(self.ranges[0].1)
                } else {
                    Some(self.ranges[idx - 1].1)
                }
            }
        }
    }
}

/// HWPX section XML에서 문단 단위로 텍스트 추출 (구조적 파싱)
/// 페이지 브레이크 태그도 감지
fn extract_paragraphs_from_section(xml_content: &str) -> Result<Vec<ParagraphNode>, ParseError> {
    let mut reader = Reader::from_str(xml_content);
    reader.config_mut().trim_text(true);

    let mut paragraphs: Vec<ParagraphNode> = Vec::new();
    let mut current_paragraph = String::new();
    let mut in_text = false;
    let mut pending_page_break = false;
    let mut total_char_offset: usize = 0;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let local_name = e.local_name();
                let name = std::str::from_utf8(local_name.as_ref()).unwrap_or("");

                // hp:t 태그 = 텍스트 내용
                if name == "t" {
                    in_text = true;
                }

                // 페이지 브레이크 감지
                if name == "br" || name == "colPr" {
                    for attr in e.attributes().flatten() {
                        let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                        let val = std::str::from_utf8(&attr.value).unwrap_or("");
                        if (key == "type" || key == "breakType") && val == "page" {
                            pending_page_break = true;
                        }
                    }
                }

                // secPr (섹션 속성)에서도 페이지 브레이크 가능
                if name == "secPr" && !paragraphs.is_empty() {
                    pending_page_break = true;
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
                if name == "p" {
                    let para_text = std::mem::take(&mut current_paragraph);

                    paragraphs.push(ParagraphNode {
                        text: para_text.clone(),
                        char_offset: total_char_offset,
                        has_page_break_before: pending_page_break,
                    });

                    // 오프셋 업데이트 (문단 텍스트 + 줄바꿈)
                    total_char_offset += para_text.chars().count() + 1;
                    pending_page_break = false;
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
        paragraphs.push(ParagraphNode {
            text: current_paragraph,
            char_offset: total_char_offset,
            has_page_break_before: pending_page_break,
        });
    }

    Ok(paragraphs)
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
                        if (key == "id" || key == "name")
                            && (val == "0"
                                || val.contains("바탕")
                                || val.to_lowercase().contains("normal"))
                        {
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

                // 페이지 크기 태그들
                if name == "sz" || name == "pSz" || name == "pageSz" {
                    for attr in e.attributes().flatten() {
                        let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                        let val = std::str::from_utf8(&attr.value).unwrap_or("");
                        match key {
                            "h" | "height" => {
                                if let Ok(h) = val.parse::<u32>() {
                                    settings.height = h;
                                }
                            }
                            "w" | "width" => {
                                if let Ok(w) = val.parse::<u32>() {
                                    settings.width = w;
                                }
                            }
                            _ => {}
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
                            "left" | "l" => {
                                if let Ok(v) = val.parse::<u32>() {
                                    settings.left_margin = v;
                                }
                            }
                            "right" | "r" => {
                                if let Ok(v) = val.parse::<u32>() {
                                    settings.right_margin = v;
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
