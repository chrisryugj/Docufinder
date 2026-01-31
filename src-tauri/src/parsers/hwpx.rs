use super::{DocumentChunk, DocumentMetadata, ParseError, ParsedDocument};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use zip::ZipArchive;

// ============================================================================
// 압축 폭탄 방어 상수
// ============================================================================

/// 단일 엔트리 최대 압축 해제 크기 (50MB)
const MAX_ENTRY_UNCOMPRESSED_SIZE: u64 = 50 * 1024 * 1024;

/// 전체 압축 해제 크기 합계 제한 (200MB)
const MAX_TOTAL_UNCOMPRESSED_SIZE: u64 = 200 * 1024 * 1024;

/// 최대 ZIP 엔트리 수
const MAX_ZIP_ENTRIES: usize = 1000;

/// 압축 비율 제한 (uncompressed/compressed > 100 = 의심)
const MAX_COMPRESSION_RATIO: u64 = 100;

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
    /// 현재 Y 위치 (hwpunit)
    current_y: f32,
    /// 페이지 유효 높이 (hwpunit)
    max_height: f32,
    /// 줄 높이 (hwpunit)
    line_height: f32,
    /// 한 줄 최대 가중치 유닛 수 (ASCII=1.0, 전각=2.0)
    max_units_per_line: f32,
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
        let font_size = style.font_size.max(100) as f32;
        let line_height = (font_size * style.line_spacing.max(80) as f32 / 100.0).max(100.0);

        let max_height = max_height.max(line_height);
        let effective_width = effective_width.max(font_size);
        let max_units_per_line = (effective_width / (font_size * 0.5)).max(10.0);

        Self {
            current_y: 0.0,
            max_height,
            line_height,
            max_units_per_line,
        }
    }

    fn layout_paragraph(&mut self, para: &ParagraphNode, page_starts: &mut Vec<usize>) {
        if para.has_page_break_before {
            self.apply_page_break(page_starts, para.char_offset);
        }

        // 빈 문단은 한 줄 처리
        if para.text.trim().is_empty() {
            self.advance_line(page_starts, para.char_offset);
            return;
        }

        let mut line_units = 0.0_f32;
        let mut line_start_offset = 0usize;

        for (idx, ch) in para.text.chars().enumerate() {
            if ch == '\n' {
                self.advance_line(page_starts, para.char_offset + line_start_offset);
                line_units = 0.0;
                line_start_offset = idx + 1;
                continue;
            }

            let weight = char_weight_units(ch);
            if line_units + weight > self.max_units_per_line {
                self.advance_line(page_starts, para.char_offset + line_start_offset);
                line_units = weight;
                line_start_offset = idx;
            } else {
                line_units += weight;
            }
        }

        // 마지막 라인 처리
        self.advance_line(page_starts, para.char_offset + line_start_offset);
    }

    fn apply_page_break(&mut self, page_starts: &mut Vec<usize>, offset: usize) {
        if page_starts.last().copied() != Some(offset) {
            page_starts.push(offset);
        }
        self.current_y = 0.0;
    }

    fn advance_line(&mut self, page_starts: &mut Vec<usize>, line_start_offset: usize) {
        if self.current_y + self.line_height > self.max_height {
            self.apply_page_break(page_starts, line_start_offset);
        }
        self.current_y += self.line_height;
    }

}

#[derive(Debug, Clone)]
struct PageMap {
    page_starts: Vec<usize>,
}

impl PageMap {
    fn empty() -> Self {
        Self { page_starts: vec![0] }
    }

    fn total_pages(&self) -> usize {
        self.page_starts.len().max(1)
    }

    fn page_for_offset(&self, char_offset: usize) -> usize {
        let idx = self.page_starts.partition_point(|&start| start <= char_offset);
        idx.max(1)
    }
}

fn build_page_map(paragraphs: &[ParagraphNode], simulator: &mut LayoutSimulator) -> PageMap {
    if paragraphs.is_empty() {
        return PageMap::empty();
    }

    let mut page_starts = vec![0];
    for para in paragraphs {
        simulator.layout_paragraph(para, &mut page_starts);
    }

    PageMap { page_starts }
}

/// 문자 가중치 (ASCII=1.0, 전각=2.0)
fn char_weight_units(ch: char) -> f32 {
    if is_cjk_or_fullwidth(ch) {
        2.0
    } else {
        1.0
    }
}

fn is_cjk_or_fullwidth(ch: char) -> bool {
    if ch.is_ascii() {
        return false;
    }

    let code = ch as u32;
    matches!(
        code,
        0x1100..=0x11FF // Hangul Jamo
            | 0x2E80..=0x2FFF // CJK Radicals, Kangxi, etc
            | 0x3000..=0x303F // CJK Symbols & Punctuation
            | 0x3040..=0x30FF // Hiragana + Katakana
            | 0x3130..=0x318F // Hangul Compatibility Jamo
            | 0x31C0..=0x31EF // CJK Strokes
            | 0x3400..=0x4DBF // CJK Ext A
            | 0x4E00..=0x9FFF // CJK Unified Ideographs
            | 0xAC00..=0xD7AF // Hangul Syllables
            | 0xF900..=0xFAFF // CJK Compatibility Ideographs
            | 0xFE10..=0xFE6F // CJK Compatibility Forms
            | 0xFF00..=0xFFEF // Halfwidth/Fullwidth Forms
            | 0x1F300..=0x1FAFF // Emoji (treat as full-width)
    )
}

/// HWPX 파일 파싱 (Virtual Layout Simulation 적용)
/// HWPX는 OASIS ODF 기반 ZIP 포맷
/// 구조: Contents/section0.xml, section1.xml, ..., Contents/header.xml
pub fn parse(path: &Path) -> Result<ParsedDocument, ParseError> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut archive =
        ZipArchive::new(reader).map_err(|e| ParseError::ParseError(e.to_string()))?;

    // ========================================================================
    // 압축 폭탄 방어: 사전 검증
    // ========================================================================

    // 1. 엔트리 수 제한
    if archive.len() > MAX_ZIP_ENTRIES {
        return Err(ParseError::ParseError(format!(
            "ZIP 엔트리 수 초과: {} (최대 {})",
            archive.len(), MAX_ZIP_ENTRIES
        )));
    }

    // 2. 총 uncompressed size 검증
    let mut total_uncompressed: u64 = 0;
    for i in 0..archive.len() {
        if let Ok(entry) = archive.by_index_raw(i) {
            let uncompressed = entry.size();
            let compressed = entry.compressed_size();

            // 단일 엔트리 크기 제한
            if uncompressed > MAX_ENTRY_UNCOMPRESSED_SIZE {
                return Err(ParseError::ParseError(format!(
                    "ZIP 엔트리 크기 초과: {} bytes (최대 {} bytes) - {}",
                    uncompressed, MAX_ENTRY_UNCOMPRESSED_SIZE, entry.name()
                )));
            }

            // 압축 비율 검사 (압축 폭탄 탐지)
            if compressed > 0 && uncompressed / compressed > MAX_COMPRESSION_RATIO {
                return Err(ParseError::ParseError(format!(
                    "의심스러운 압축 비율: {}:1 - 압축 폭탄 가능성 ({})",
                    uncompressed / compressed, entry.name()
                )));
            }

            total_uncompressed += uncompressed;
        }
    }

    // 총 압축 해제 크기 제한
    if total_uncompressed > MAX_TOTAL_UNCOMPRESSED_SIZE {
        return Err(ParseError::ParseError(format!(
            "총 압축 해제 크기 초과: {} bytes (최대 {} bytes)",
            total_uncompressed, MAX_TOTAL_UNCOMPRESSED_SIZE
        )));
    }

    // ========================================================================
    // 1회 루프로 header.xml + section*.xml 모두 수집
    // ========================================================================
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

    // Layout Simulator로 페이지 맵 생성 (문단 내부 줄 단위 반영)
    let mut simulator = LayoutSimulator::new(&page_settings, &default_style);
    let page_map = build_page_map(&all_paragraphs, &mut simulator);
    let page_count = page_map.total_pages();

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
    let chunks = chunk_with_page_map(
        &all_text,
        &page_map,
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

/// 페이지 맵 기반 청크 분할 (Virtual Layout Simulation 결과 활용)
fn chunk_with_page_map(
    text: &str,
    page_map: &PageMap,
    chunk_size: usize,
    overlap: usize,
) -> Vec<DocumentChunk> {
    let mut chunks = Vec::new();

    if text.is_empty() {
        return chunks;
    }

    // 바이트 오프셋 매핑
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

        // 청크 범위의 페이지 계산 (시작~끝)
        let start_page = page_map.page_for_offset(start);
        let end_page = page_map.page_for_offset(end.saturating_sub(1));

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

/// HWPX section XML에서 문단 단위로 텍스트 추출 (구조적 파싱)
/// 페이지 브레이크 태그도 감지
fn extract_paragraphs_from_section(xml_content: &str) -> Result<Vec<ParagraphNode>, ParseError> {
    let mut reader = Reader::from_str(xml_content);
    reader.config_mut().trim_text(true);

    let mut paragraphs: Vec<ParagraphNode> = Vec::new();
    let mut current_paragraph = String::new();
    let mut in_text = false;
    let mut in_paragraph = false;
    let mut pending_page_break = false;
    let mut split_paragraph = false;
    let mut total_char_offset: usize = 0;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let local_name = e.local_name();
                let name = std::str::from_utf8(local_name.as_ref()).unwrap_or("");
                let name_l = name.to_ascii_lowercase();

                // hp:t 태그 = 텍스트 내용
                if name_l == "t" {
                    in_text = true;
                }

                if name_l == "p" {
                    in_paragraph = true;
                }

                // 페이지 브레이크 감지
                let mut is_page_break = matches!(
                    name_l.as_str(),
                    "pagebreak" | "pgbreak" | "page-break"
                );
                let mut is_line_break = false;

                if name_l == "br" || name_l == "colpr" || name_l == "break" {
                    let mut break_type: Option<String> = None;
                    for attr in e.attributes().flatten() {
                        let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                        let val = std::str::from_utf8(&attr.value).unwrap_or("");
                        let key_l = key.to_ascii_lowercase();
                        let val_l = val.to_ascii_lowercase();
                        if key_l == "type" || key_l == "breaktype" || key_l == "kind" {
                            break_type = Some(val_l);
                        }
                    }

                    if let Some(bt) = break_type {
                        if bt.contains("page") {
                            is_page_break = true;
                        } else if name_l == "br" && in_paragraph {
                            is_line_break = true;
                        }
                    } else if name_l == "br" && in_paragraph {
                        is_line_break = true;
                    }
                }

                if is_page_break {
                    if in_paragraph && !current_paragraph.is_empty() {
                        let para_text = std::mem::take(&mut current_paragraph);
                        paragraphs.push(ParagraphNode {
                            text: para_text.clone(),
                            char_offset: total_char_offset,
                            has_page_break_before: pending_page_break,
                        });
                        total_char_offset += para_text.chars().count() + 1;
                        split_paragraph = true;
                    }

                    pending_page_break = true;
                } else if is_line_break {
                    current_paragraph.push('\n');
                }

                // secPr (섹션 속성)에서도 페이지 브레이크 가능
                if name_l == "secpr" && !paragraphs.is_empty() {
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
                let name_l = name.to_ascii_lowercase();

                if name_l == "t" {
                    in_text = false;
                }
                // p 태그 종료 = 문단 끝
                if name_l == "p" {
                    in_paragraph = false;
                    if !current_paragraph.is_empty() || !split_paragraph {
                        let para_text = std::mem::take(&mut current_paragraph);

                        paragraphs.push(ParagraphNode {
                            text: para_text.clone(),
                            char_offset: total_char_offset,
                            has_page_break_before: pending_page_break,
                        });

                        // 오프셋 업데이트 (문단 텍스트 + 줄바꿈)
                        total_char_offset += para_text.chars().count() + 1;
                        pending_page_break = false;
                    } else {
                        current_paragraph.clear();
                    }

                    split_paragraph = false;
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
