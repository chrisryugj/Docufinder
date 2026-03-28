mod layout;
mod models;
mod text_extraction;
mod xml_parsing;

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use zip::ZipArchive;

use super::{DocumentChunk, DocumentMetadata, ParseError, ParsedDocument};
use super::{MAX_ENTRY_UNCOMPRESSED_SIZE, MAX_FILE_SIZE};

use layout::{LayoutSimulator, PageMap};
use models::ParagraphNode;
use text_extraction::extract_paragraphs_from_section;
use xml_parsing::{parse_header_xml, parse_page_settings, parse_styles_xml};

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
    // 파일 크기 체크 (대용량 파일 메모리 보호)
    if let Ok(metadata) = std::fs::metadata(path) {
        if metadata.len() > MAX_FILE_SIZE {
            return Err(ParseError::ParseError(format!(
                "HWPX 파일 크기 초과: {}MB (최대 {}MB)",
                metadata.len() / 1024 / 1024,
                MAX_FILE_SIZE / 1024 / 1024
            )));
        }
    }

    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut archive = ZipArchive::new(reader).map_err(|e| ParseError::ParseError(e.to_string()))?;

    // 압축 폭탄 방어: 사전 검증 (공통 모듈)
    super::validate_zip_archive(&mut archive)?;

    // ========================================================================
    // 1회 루프로 header.xml + section*.xml 모두 수집
    // ========================================================================
    let mut header_content: Option<String> = None;
    let mut styles_content: Option<String> = None;
    let mut section_xmls: BTreeMap<usize, String> = BTreeMap::new();

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| ParseError::ParseError(e.to_string()))?;

        let name = file.name().to_string();

        // header.xml 수집 (하드 제한: 압축 폭탄 방어)
        if name == "Contents/header.xml" {
            let mut contents = String::new();
            std::io::Read::take(&mut file, MAX_ENTRY_UNCOMPRESSED_SIZE)
                .read_to_string(&mut contents)?;
            header_content = Some(contents);
            continue;
        }

        // styles.xml 수집 (스타일별 폰트/줄간격)
        if name == "Contents/styles.xml" {
            let mut contents = String::new();
            std::io::Read::take(&mut file, MAX_ENTRY_UNCOMPRESSED_SIZE)
                .read_to_string(&mut contents)?;
            styles_content = Some(contents);
            continue;
        }

        // section XML 파일만 처리 (section0.xml, section1.xml, ...)
        if name.starts_with("Contents/section") && name.ends_with(".xml") {
            let section_num = name
                .trim_start_matches("Contents/section")
                .trim_end_matches(".xml")
                .parse::<usize>()
                .unwrap_or(0);

            // 하드 제한: 압축 폭탄 방어 (사전 검증 우회 대비)
            let mut contents = String::new();
            std::io::Read::take(&mut file, MAX_ENTRY_UNCOMPRESSED_SIZE)
                .read_to_string(&mut contents)?;
            section_xmls.insert(section_num, contents);
        }
    }

    // header.xml에서 기본 스타일 파싱
    let default_style = header_content
        .as_ref()
        .map(|c| parse_header_xml(c))
        .unwrap_or_default();

    // styles.xml에서 스타일별 속성 파싱
    let styles_map = styles_content
        .as_ref()
        .map(|c| parse_styles_xml(c))
        .unwrap_or_default();

    // 섹션별 페이지 설정 파싱 + 문단 추출 + 페이지맵 빌드
    let mut all_paragraphs: Vec<ParagraphNode> = Vec::new();
    let mut page_starts = vec![0usize];
    let mut total_char_offset: usize = 0;

    for (section_idx, xml) in &section_xmls {
        // 각 섹션의 페이지 설정 파싱 (표/도형 오염 방지된 컨텍스트 기반)
        let section_settings = parse_page_settings(xml);

        let mut section_paras = extract_paragraphs_from_section(xml)?;

        // 섹션 간 구분: 첫 번째가 아닌 섹션은 페이지 브레이크로 처리
        if *section_idx > 0 && !section_paras.is_empty() {
            section_paras[0].has_page_break_before = true;
        }

        // 오프셋 조정
        for para in &mut section_paras {
            para.char_offset += total_char_offset;
        }

        // 섹션별 LayoutSimulator로 페이지맵 빌드
        // 섹션은 항상 새 페이지에서 시작하므로 simulator 재생성이 정확
        let mut simulator = LayoutSimulator::new(&section_settings, &default_style);
        for para in &section_paras {
            simulator.layout_paragraph(para, &mut page_starts, &styles_map, &default_style);
        }

        // 전체 오프셋 업데이트
        if let Some(last) = section_paras.last() {
            total_char_offset = last.char_offset + last.text.chars().count() + 1;
        }

        all_paragraphs.extend(section_paras);
    }

    let page_map = PageMap { page_starts };

    // 전체 문자 수 계산 (문단 + 줄바꿈)
    let total_chars: usize = all_paragraphs
        .iter()
        .map(|p| p.text.chars().count())
        .sum::<usize>()
        + all_paragraphs.len().saturating_sub(1);

    // lineSeg 커버리지 계산 (신뢰도 판정용)
    let paras_with_linesegs = all_paragraphs
        .iter()
        .filter(|p| !p.line_segs.is_empty())
        .count();
    let total_paras = all_paragraphs.len().max(1);
    let lineseg_coverage = paras_with_linesegs as f32 / total_paras as f32;

    // Sanity check: 시뮬레이션 결과 검증
    let estimated_pages = page_map.total_pages();
    let chars_per_page = if estimated_pages > 0 {
        total_chars / estimated_pages
    } else {
        0
    };

    // lineSeg 커버리지가 50% 이상이면 시뮬레이션 신뢰도 높음 → fallback 스킵
    // lineSeg 없이 페이지당 250자 미만이면 시뮬레이션 오류로 판단
    let is_unreasonable =
        total_chars > 0 && estimated_pages > 1 && chars_per_page < 250 && lineseg_coverage < 0.5;

    let (page_map, page_count) = if is_unreasonable {
        tracing::warn!(
            "HWPX layout sim unreasonable: {} pages for {} chars ({} chars/page, fontSz={}, lineSpacing={}%, lineSeg={:.0}%). Falling back to proportional.",
            estimated_pages,
            total_chars,
            chars_per_page,
            default_style.font_size,
            default_style.line_spacing,
            lineseg_coverage * 100.0
        );
        // 비례 배분 fallback: ~1500 chars/page (한글 A4 기본 추정)
        let est_pages = (total_chars / 1500).max(1);
        let cpp = total_chars / est_pages;
        let mut page_starts = vec![0usize];
        for i in 1..est_pages {
            page_starts.push(i * cpp);
        }
        let pm = PageMap { page_starts };
        let pc = pm.total_pages();
        (pm, pc)
    } else {
        let pc = page_map.total_pages();
        (page_map, pc)
    };

    // 신뢰도 표시 (lineSeg 기반이면 HIGH, 아니면 LOW)
    let confidence = if lineseg_coverage >= 0.5 {
        "HIGH(lineSeg)"
    } else if styles_map.is_empty() {
        "LOW(basic-sim)"
    } else {
        "MEDIUM(styled-sim)"
    };

    tracing::debug!(
        "HWPX page calc: {} 문단, {} 페이지, 신뢰도={}, lineSeg={:.0}%, 폰트 {}hwpunit, 줄간격 {}%",
        all_paragraphs.len(),
        page_count,
        confidence,
        lineseg_coverage * 100.0,
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
            page_end: Some(end_page),
            location_hint: Some(location_hint),
        });

        start += step;
        if end >= total_len {
            break;
        }
    }

    chunks
}
