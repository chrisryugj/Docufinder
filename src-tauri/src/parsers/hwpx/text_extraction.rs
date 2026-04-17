use quick_xml::events::Event;
use quick_xml::Reader;

use super::models::{LineSeg, ParagraphNode};
use crate::parsers::ParseError;

/// HWPX section XML에서 문단 단위로 텍스트 추출 (구조적 파싱)
/// lineSeg, 객체 높이, 스타일 ID, 페이지 브레이크 태그 감지
pub(super) fn extract_paragraphs_from_section(
    xml_content: &str,
) -> Result<Vec<ParagraphNode>, ParseError> {
    let mut reader = Reader::from_str(xml_content);
    reader.config_mut().trim_text(true);

    let mut paragraphs: Vec<ParagraphNode> = Vec::new();
    let mut current_paragraph = String::new();
    let mut in_text = false;
    let mut in_paragraph = false;
    let mut pending_page_break = false;
    let mut split_paragraph = false;
    let mut total_char_offset: usize = 0;

    // Phase 1: 스타일 ID 추적
    let mut current_style_id: Option<String> = None;

    // Phase 1: 객체 높이 추적
    let mut in_object: bool = false;
    let mut object_depth: usize = 0;
    let mut current_object_height: f32 = 0.0;
    let mut para_object_height: f32 = 0.0;

    // 표(table) 구조 추적
    let mut in_table: bool = false;
    let mut table_depth: usize = 0;
    let mut in_table_cell: bool = false;
    let mut current_cell_text: String = String::new();
    let mut table_row_cells: Vec<String> = Vec::new();
    let mut table_rows: Vec<String> = Vec::new();

    // Phase 2: lineSeg 추적
    let mut in_lineseg_array = false;
    let mut current_linesegs: Vec<LineSeg> = Vec::new();

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

                // p 태그: 스타일 ID 추출
                if name_l == "p" {
                    in_paragraph = true;
                    current_style_id = None;
                    para_object_height = 0.0;
                    current_linesegs.clear();

                    for attr in e.attributes().flatten() {
                        let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                        let val = std::str::from_utf8(&attr.value).unwrap_or("");
                        if key == "styleIDRef" || key == "style" || key == "styleid" {
                            current_style_id = Some(val.to_string());
                        }
                    }
                }

                // 표(tbl) 태그 — 별도 처리 (행/열 구조 보존)
                if name_l == "tbl" && in_paragraph {
                    in_table = true;
                    table_depth = 1;
                    table_rows.clear();
                    table_row_cells.clear();
                    current_cell_text.clear();
                    current_object_height = 0.0;
                } else if in_table {
                    table_depth += 1;
                    // tr/tc 태그 추적
                    if name_l == "tr" {
                        table_row_cells.clear();
                    } else if name_l == "tc" {
                        in_table_cell = true;
                        current_cell_text.clear();
                    }
                }

                // 객체 태그 감지 (pic, container, rect, ellipse, curve 등 — tbl 제외)
                if matches!(
                    name_l.as_str(),
                    "pic" | "container" | "rect" | "ellipse" | "curve"
                ) && in_paragraph
                {
                    in_object = true;
                    object_depth = 1;
                    current_object_height = 0.0;
                } else if in_object {
                    object_depth += 1;
                }

                // 객체/표 내부의 sz 태그에서 높이 추출
                if name_l == "sz" && (in_object || in_table) {
                    for attr in e.attributes().flatten() {
                        let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                        let val = std::str::from_utf8(&attr.value).unwrap_or("");
                        if key == "h" || key == "height" {
                            if let Ok(h) = val.parse::<f32>() {
                                if h > 0.0 && h < 500000.0 {
                                    current_object_height = h;
                                }
                            }
                        }
                    }
                }

                // linesegarray 태그
                if name_l == "linesegarray" && in_paragraph {
                    in_lineseg_array = true;
                }

                // lineseg 태그
                if name_l == "lineseg" && in_lineseg_array {
                    let mut text_start: usize = 0;
                    let mut line_h: u32 = 0;

                    for attr in e.attributes().flatten() {
                        let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                        let val = std::str::from_utf8(&attr.value).unwrap_or("");
                        let key_l = key.to_ascii_lowercase();
                        match key_l.as_str() {
                            "textstartpos" | "textpos" | "textstart" => {
                                text_start = val.parse().unwrap_or(0);
                            }
                            "lineheight" | "lineht" | "lnheight" => {
                                line_h = val.parse().unwrap_or(0);
                            }
                            // spacing을 line_height에 합산
                            "spacing" => {
                                if let Ok(sp) = val.parse::<u32>() {
                                    line_h = line_h.saturating_add(sp);
                                }
                            }
                            _ => {}
                        }
                    }

                    if line_h > 0 {
                        current_linesegs.push(LineSeg {
                            text_start_pos: text_start,
                            line_height: line_h,
                        });
                    }
                }

                // 페이지 브레이크 감지
                let mut is_page_break =
                    matches!(name_l.as_str(), "pagebreak" | "pgbreak" | "page-break");
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
                            style_id: current_style_id.clone(),
                            object_height: para_object_height,
                            line_segs: std::mem::take(&mut current_linesegs),
                        });
                        total_char_offset += para_text.chars().count() + 1;
                        split_paragraph = true;
                        para_object_height = 0.0;
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
            Ok(Event::Text(e))
                if in_text => {
                    let text = e
                        .unescape()
                        .map_err(|e| ParseError::ParseError(e.to_string()))?;
                    current_paragraph.push_str(&text);
                }
            Ok(Event::End(e)) => {
                let local_name = e.local_name();
                let name = std::str::from_utf8(local_name.as_ref()).unwrap_or("");
                let name_l = name.to_ascii_lowercase();

                if name_l == "t" {
                    in_text = false;
                }

                // 표 태그 종료
                if in_table {
                    if name_l == "tc" {
                        // 셀 종료: 셀 텍스트를 행에 추가
                        let cell = current_cell_text.trim().to_string();
                        table_row_cells.push(cell);
                        current_cell_text.clear();
                        in_table_cell = false;
                    } else if name_l == "tr" {
                        // 행 종료: 빈 행 제거 후 셀들을 탭으로 합쳐서 행 완성
                        let has_content = table_row_cells.iter().any(|c| !c.is_empty());
                        if !table_row_cells.is_empty() && has_content {
                            table_rows.push(table_row_cells.join("\t"));
                        }
                        table_row_cells.clear();
                    } else if name_l == "tbl" {
                        // 표 종료: 행들을 줄바꿈으로 합쳐서 하나의 문단으로 추가
                        if !table_rows.is_empty() {
                            let table_text = table_rows.join("\n");
                            paragraphs.push(ParagraphNode {
                                text: table_text.clone(),
                                char_offset: total_char_offset,
                                has_page_break_before: pending_page_break,
                                style_id: current_style_id.clone(),
                                object_height: current_object_height,
                                line_segs: Vec::new(),
                            });
                            total_char_offset += table_text.chars().count() + 1;
                            pending_page_break = false;
                        }
                        para_object_height += current_object_height;
                        in_table = false;
                        table_depth = 0;
                        current_object_height = 0.0;
                        table_rows.clear();
                    } else {
                        table_depth = table_depth.saturating_sub(1);
                    }
                }

                // 객체 태그 종료 (tbl 제외)
                if in_object {
                    if matches!(
                        name_l.as_str(),
                        "pic" | "container" | "rect" | "ellipse" | "curve"
                    ) {
                        // 객체 높이를 문단에 합산
                        para_object_height += current_object_height;
                        in_object = false;
                        object_depth = 0;
                        current_object_height = 0.0;
                    } else {
                        object_depth = object_depth.saturating_sub(1);
                    }
                }

                // linesegarray 종료
                if name_l == "linesegarray" {
                    in_lineseg_array = false;
                }

                // p 태그 종료 = 문단 끝
                if name_l == "p" {
                    in_paragraph = false;

                    if in_table && in_table_cell {
                        // 표 셀 안의 문단 → paragraphs가 아닌 셀 버퍼에 추가
                        let para_text = std::mem::take(&mut current_paragraph);
                        if !para_text.is_empty() {
                            if !current_cell_text.is_empty() {
                                current_cell_text.push(' '); // 셀 내 다중 문단은 공백으로 연결
                            }
                            current_cell_text.push_str(&para_text);
                        }
                    } else if !current_paragraph.is_empty() || !split_paragraph {
                        let para_text = std::mem::take(&mut current_paragraph);

                        paragraphs.push(ParagraphNode {
                            text: para_text.clone(),
                            char_offset: total_char_offset,
                            has_page_break_before: pending_page_break,
                            style_id: current_style_id.clone(),
                            object_height: para_object_height,
                            line_segs: std::mem::take(&mut current_linesegs),
                        });

                        // 오프셋 업데이트 (문단 텍스트 + 줄바꿈)
                        total_char_offset += para_text.chars().count() + 1;
                        pending_page_break = false;
                    } else {
                        current_paragraph.clear();
                    }

                    split_paragraph = false;
                    para_object_height = 0.0;
                    current_linesegs.clear();
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
            style_id: current_style_id,
            object_height: para_object_height,
            line_segs: current_linesegs,
        });
    }

    Ok(paragraphs)
}
