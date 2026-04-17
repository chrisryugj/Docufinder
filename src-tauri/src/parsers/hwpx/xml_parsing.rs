use std::collections::HashMap;

use quick_xml::events::Event;
use quick_xml::Reader;

use super::models::{DefaultStyle, PageSettings, StyleData};

/// styles.xml에서 스타일별 속성 파싱
/// 스타일 ID → (폰트크기, 줄간격) 매핑
pub(super) fn parse_styles_xml(xml_content: &str) -> HashMap<String, StyleData> {
    let mut reader = Reader::from_str(xml_content);
    reader.config_mut().trim_text(true);

    let mut styles: HashMap<String, StyleData> = HashMap::new();
    let mut current_id: Option<String> = None;
    let mut current_font_size: Option<u32> = None;
    let mut current_line_spacing: Option<u32> = None;
    let mut in_style = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let local_name = e.local_name();
                let name = std::str::from_utf8(local_name.as_ref()).unwrap_or("");

                // style 태그 시작
                if name == "style" || name == "Style" {
                    in_style = true;
                    current_font_size = None;
                    current_line_spacing = None;
                    current_id = None;

                    for attr in e.attributes().flatten() {
                        let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                        let val = std::str::from_utf8(&attr.value).unwrap_or("");
                        if key == "id" || key == "Id" {
                            current_id = Some(val.to_string());
                        }
                    }
                }

                // charPr (글자 속성)
                if (name == "charPr" || name == "rPr") && in_style {
                    for attr in e.attributes().flatten() {
                        let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                        let val = std::str::from_utf8(&attr.value).unwrap_or("");
                        if key == "fontSz" || key == "sz" {
                            if let Ok(sz) = val.parse::<u32>() {
                                current_font_size = Some(normalize_font_size(sz));
                            }
                        }
                    }
                }

                // lineSpacing / lnSpc (줄간격)
                if (name == "lineSpacing" || name == "lnSpc") && in_style {
                    for attr in e.attributes().flatten() {
                        let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                        let val = std::str::from_utf8(&attr.value).unwrap_or("");
                        if key == "val" || key == "value" {
                            if let Ok(ls) = val.parse::<u32>() {
                                current_line_spacing = Some(normalize_line_spacing(ls));
                            }
                        }
                    }
                }
            }
            Ok(Event::End(e)) => {
                let local_name = e.local_name();
                let name = std::str::from_utf8(local_name.as_ref()).unwrap_or("");
                if name == "style" || name == "Style" {
                    if let Some(id) = current_id.take() {
                        if current_font_size.is_some() || current_line_spacing.is_some() {
                            styles.insert(
                                id,
                                StyleData {
                                    font_size: current_font_size,
                                    line_spacing: current_line_spacing,
                                },
                            );
                        }
                    }
                    in_style = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    styles
}

/// fontSz 단위 정규화 (공통)
pub(super) fn normalize_font_size(raw: u32) -> u32 {
    let mut sz = raw;
    if sz >= 40000 {
        sz /= 100;
    } else if sz > 4000 {
        sz /= 10;
    }
    sz.clamp(500, 4000)
}

/// lineSpacing 단위 정규화 (공통)
pub(super) fn normalize_line_spacing(raw: u32) -> u32 {
    let mut ls = raw;
    if ls >= 5000 {
        ls /= 100;
    } else if ls > 500 {
        ls /= 10;
    }
    ls.clamp(80, 300)
}

/// header.xml에서 기본 스타일 파싱
/// charPr의 fontSz, paraPr의 lineSpacing 추출
pub(super) fn parse_header_xml(xml_content: &str) -> DefaultStyle {
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

                // 문단 속성 (lineSpacing) - 기본 스타일 내에서만 적용
                if (name == "lineSpacing" || name == "lnSpc") && in_default_style {
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

    // 공통 정규화 함수 사용
    let original_font = style.font_size;
    style.font_size = normalize_font_size(style.font_size);
    if style.font_size != original_font {
        tracing::debug!(
            "HWPX fontSz normalized: {} -> {}",
            original_font,
            style.font_size
        );
    }

    let original_ls = style.line_spacing;
    style.line_spacing = normalize_line_spacing(style.line_spacing);
    if style.line_spacing != original_ls {
        tracing::debug!(
            "HWPX lineSpacing normalized: {} -> {}",
            original_ls,
            style.line_spacing
        );
    }

    style
}

/// section XML에서 페이지 설정 파싱
/// secPr/pagePr 컨텍스트 내부의 pSz/pageSz/margin만 반영 (표/도형 sz 오염 방지)
pub(super) fn parse_page_settings(xml_content: &str) -> PageSettings {
    let mut reader = Reader::from_str(xml_content);
    reader.config_mut().trim_text(true);

    let defaults = PageSettings::default();
    let mut settings = defaults.clone();

    // secPr 또는 pagePr 내부에서만 페이지 크기/여백을 파싱
    let mut in_page_context = false;
    let mut context_depth: usize = 0;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let local_name = e.local_name();
                let name = std::str::from_utf8(local_name.as_ref()).unwrap_or("");
                let name_l = name.to_ascii_lowercase();

                // secPr, pagePr, masterPage 등 페이지 설정 컨텍스트 진입
                if matches!(
                    name_l.as_str(),
                    "secpr" | "pagepr" | "masterpage" | "pagelayout"
                ) {
                    in_page_context = true;
                    context_depth = 1;
                } else if in_page_context {
                    context_depth += 1;
                    parse_page_element(&name_l, &e, &mut settings);
                }
            }
            Ok(Event::Empty(e)) if in_page_context => {
                let local_name = e.local_name();
                let name = std::str::from_utf8(local_name.as_ref()).unwrap_or("");
                let name_l = name.to_ascii_lowercase();
                parse_page_element(&name_l, &e, &mut settings);
            }
            Ok(Event::End(e)) if in_page_context => {
                let local_name = e.local_name();
                let name = std::str::from_utf8(local_name.as_ref()).unwrap_or("");
                let name_l = name.to_ascii_lowercase();

                if matches!(
                    name_l.as_str(),
                    "secpr" | "pagepr" | "masterpage" | "pagelayout"
                ) {
                    in_page_context = false;
                    context_depth = 0;
                } else {
                    context_depth = context_depth.saturating_sub(1);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    // 값 검증: 비정상이면 기본값 유지
    // A4 기준 hwpunit: height ~84188, width ~59528
    // 허용 범위: 가로/세로 각 20000~200000 (B5~A3 이상 커버)
    if settings.height < 20000 || settings.height > 200000 {
        tracing::debug!(
            "HWPX page height {} out of range, using default {}",
            settings.height,
            defaults.height
        );
        settings.height = defaults.height;
    }
    if settings.width < 20000 || settings.width > 200000 {
        tracing::debug!(
            "HWPX page width {} out of range, using default {}",
            settings.width,
            defaults.width
        );
        settings.width = defaults.width;
    }

    // 여백이 페이지의 80% 이상을 차지하면 비정상
    let margin_sum = settings
        .top_margin
        .saturating_add(settings.bottom_margin)
        .saturating_add(settings.header_offset)
        .saturating_add(settings.footer_offset);
    if margin_sum > settings.height * 4 / 5 {
        tracing::debug!(
            "HWPX vertical margins {} exceed 80% of height {}, using defaults",
            margin_sum,
            settings.height
        );
        settings.top_margin = defaults.top_margin;
        settings.bottom_margin = defaults.bottom_margin;
        settings.header_offset = defaults.header_offset;
        settings.footer_offset = defaults.footer_offset;
    }

    settings
}

/// 페이지 설정 요소 파싱 헬퍼 (secPr/pagePr 컨텍스트 내에서만 호출)
fn parse_page_element(
    name: &str,
    e: &quick_xml::events::BytesStart<'_>,
    settings: &mut PageSettings,
) {
    // 페이지 크기
    if matches!(name, "sz" | "psz" | "pagesz") {
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
    if matches!(name, "margin" | "pagemar") {
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
