//! Windows COM(PowerPoint.Application) 기반 PPTX 파서 — Windows 전용.
//!
//! `parsers::pptx` (Rust ZIP/XML 파서) 실패 시 `wincom_fallback_pptx` 에서 호출.
//! 설치된 PowerPoint 로 프레젠테이션을 열어 각 슬라이드의 도형(Shape) 텍스트와
//! 노트(NotesPage) 텍스트를 추출한다.

use std::path::Path;

use super::{
    to_parse_error, v_string, var_i32, var_str, AppGuard, ComApartment, Obj, MSO_FALSE, MSO_TRUE,
};
use crate::parsers::{
    DocumentChunk, DocumentMetadata, ParseError, ParsedDocument, DEFAULT_CHUNK_OVERLAP,
    DEFAULT_CHUNK_SIZE, MAX_FILE_SIZE,
};

/// `PpAlertLevel::ppAlertsNone` — 경고 대화상자 비활성.
const PP_ALERTS_NONE: i32 = 1;

/// PPTX 파일을 COM(PowerPoint.Application) 으로 파싱.
///
/// `parse_with_timeout` 래퍼를 통해 별도 STA 스레드에서 실행되며,
/// 타임아웃·패닉 방어가 적용된다.
pub fn parse(path: &Path) -> Result<ParsedDocument, ParseError> {
    // 파일 크기 체크 (대용량 파일 메모리 보호)
    if let Ok(metadata) = std::fs::metadata(path) {
        if metadata.len() > MAX_FILE_SIZE {
            return Err(ParseError::ParseError(format!(
                "PPTX 파일 크기 초과: {}MB (최대 {}MB)",
                metadata.len() / 1024 / 1024,
                MAX_FILE_SIZE / 1024 / 1024
            )));
        }
    }

    let _com = ComApartment::init();

    let slides = extract_slides(path).map_err(|e| to_parse_error("PowerPoint", &e))?;
    if slides.is_empty() {
        tracing::warn!("PPTX(COM) file has no text content: {:?}", path);
    }
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
        garbled_hint: false,
    })
}

/// 슬라이드별 텍스트 정보 (슬라이드 번호 + 정규화된 텍스트).
struct SlideText {
    slide_number: usize,
    text: String,
}

/// PowerPoint 를 COM 자동화로 구동해 각 슬라이드의 텍스트를 추출.
///
/// Application → `Presentations.Open` → `Slides` 컬렉션 순회 →
/// 각 슬라이드의 Shape 텍스트 + `NotesPage` 텍스트 수집 순서로 동작.
fn extract_slides(path: &Path) -> windows::core::Result<Vec<SlideText>> {
    let app = Obj::create("PowerPoint.Application")?;
    let _guard = AppGuard::new(&app);
    // PowerPoint 는 Word/Excel 과 달리 Visible=false 미지원 — 항상 가시 창으로 동작.
    let _ = app.put("DisplayAlerts", var_i32(PP_ALERTS_NONE));
    let _ = app.put(
        "AutomationSecurity",
        var_i32(super::MSO_AUTOMATION_SECURITY_FORCE_DISABLE),
    );

    let presentations = app.get_obj("Presentations", &[])?;
    let path_str = super::path_arg(path);
    let pres_var = presentations.call(
        "Open",
        &[
            var_str(&path_str), // FileName — 절대 경로
            var_i32(MSO_TRUE),  // ReadOnly — 읽기 전용
            var_i32(MSO_FALSE), // Untitled — 새 제목 부여 안 함
            var_i32(MSO_FALSE), // WithWindow — PowerPoint 창 표시 안 함
        ],
    )?;
    let pres = super::as_obj(&pres_var)?;

    let slides_col = pres.get_obj("Slides", &[])?;
    let count = slides_col.get_i32("Count", &[]);

    let mut out: Vec<SlideText> = Vec::new();
    for i in 1..=count {
        let slide = match slides_col.get_obj("Item", &[var_i32(i)]) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let mut slide_text = collect_shape_text(&slide);
        // 노트 페이지(발표자 메모) 텍스트 추가 — "[노트]" prefix 로 본문과 구분.
        if let Ok(notes_page) = slide.get_obj("NotesPage", &[]) {
            let note = collect_shape_text(&notes_page);
            if !note.is_empty() {
                if !slide_text.is_empty() {
                    slide_text.push('\n');
                }
                slide_text.push_str("[노트] ");
                slide_text.push_str(&note);
            }
        }
        if !slide_text.is_empty() {
            out.push(SlideText {
                slide_number: i as usize,
                text: slide_text,
            });
        }
    }

    let _ = pres.call("Close", &[]);

    Ok(out)
}

/// 컨테이너(슬라이드 또는 노트 페이지) 내 모든 Shape 의 텍스트를 수집.
///
/// 각 Shape → `HasTextFrame` → `TextFrame.HasText` → `TextRange.Text` 순서로
/// 텍스트를 안전하게 추출한다 (텍스트가 없는 도형/이미지는 건너뜀).
fn collect_shape_text(container: &Obj) -> String {
    let shapes = match container.get_obj("Shapes", &[]) {
        Ok(s) => s,
        Err(_) => return String::new(),
    };
    let count = shapes.get_i32("Count", &[]);
    let mut parts: Vec<String> = Vec::new();
    for j in 1..=count {
        let shape = match shapes.get_obj("Item", &[var_i32(j)]) {
            Ok(s) => s,
            Err(_) => continue,
        };
        if shape.get_i32("HasTextFrame", &[]) != MSO_TRUE {
            continue;
        }
        let text_frame = match shape.get_obj("TextFrame", &[]) {
            Ok(t) => t,
            Err(_) => continue,
        };
        if text_frame.get_i32("HasText", &[]) != MSO_TRUE {
            continue;
        }
        let text_range = match text_frame.get_obj("TextRange", &[]) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let raw = match text_range.get("Text", &[]) {
            Ok(v) => v_string(&v),
            Err(_) => continue,
        };
        let normalized = normalize_lines(&raw);
        if !normalized.is_empty() {
            parts.push(normalized);
        }
    }

    parts.join("\n")
}

/// 줄바꿈 정규화 — CR/LF/수직탭을 `\n`으로 통일하고 빈 줄을 제거한다.
fn normalize_lines(text: &str) -> String {
    text.split(['\r', '\n', '\u{000B}'])
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// 슬라이드 텍스트를 청크로 분할 (슬라이드 번호·위치 힌트 유지).
/// `global_offset` 은 전체 문서 기준 오프셋 (start_offset/end_offset 에 사용).
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
