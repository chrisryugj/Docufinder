//! Windows COM(Word.Application) 기반 DOCX 파서 — Windows 전용.
//!
//! `parsers::docx` (Rust ZIP/XML 파서) 실패 시 `wincom_fallback_docx` 에서 호출.
//! 설치된 Word 로 문서를 열어 `Content.Text` 를 추출한다.

use std::path::Path;

use super::{
    to_parse_error, v_string, var_bool, var_i32, var_str, AppGuard, AppKind, ComApartment, Obj,
};
use crate::parsers::{
    DocumentChunk, DocumentMetadata, ParseError, ParsedDocument, DEFAULT_CHUNK_OVERLAP,
    DEFAULT_CHUNK_SIZE, MAX_FILE_SIZE,
};

/// `WdSaveOptions::wdDoNotSaveChanges` — Close 시 저장하지 않음.
const WD_DO_NOT_SAVE_CHANGES: i32 = 0;
/// `WdAlertLevel::wdAlertsNone` — 경고 대화상자 비활성.
const WD_ALERTS_NONE: i32 = 0;

/// DOCX 파일을 COM(Word.Application) 으로 파싱.
///
/// `parse_with_timeout` 래퍼를 통해 별도 STA 스레드에서 실행되며,
/// 타임아웃·패닉 방어가 적용된다.
pub fn parse(path: &Path) -> Result<ParsedDocument, ParseError> {
    // 파일 크기 체크 (대용량 파일 메모리 보호)
    if let Ok(metadata) = std::fs::metadata(path) {
        if metadata.len() > MAX_FILE_SIZE {
            return Err(ParseError::ParseError(format!(
                "DOCX 파일 크기 초과: {}MB (최대 {}MB)",
                metadata.len() / 1024 / 1024,
                MAX_FILE_SIZE / 1024 / 1024
            )));
        }
    }

    let _com = ComApartment::init();

    let full_text = extract_text(path).map_err(|e| to_parse_error("Word", &e))?;
    let pages = split_pages(&full_text);
    if pages.is_empty() {
        tracing::warn!("DOCX(COM) file has no text content: {:?}", path);
    }
    let total_text = pages
        .iter()
        .map(|p| p.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let chunks = chunk_pages(&pages, DEFAULT_CHUNK_SIZE, DEFAULT_CHUNK_OVERLAP);
    let page_count = pages.len();

    Ok(ParsedDocument {
        content: total_text,
        metadata: DocumentMetadata {
            title: path.file_stem().and_then(|s| s.to_str()).map(String::from),
            author: None,
            created_at: None,
            page_count: if page_count > 1 {
                Some(page_count)
            } else {
                None
            },
        },
        chunks,
        garbled_hint: false,
    })
}

/// Word 를 COM 자동화로 구동해 문서 전체 텍스트 추출.
///
/// Application 객체 생성 → 보안 설정(Visible/DisplayAlerts/ScreenUpdating/AutomationSecurity)
/// → `Documents.Open` → `Content.Text` 읽기 → `Close` 순서로 동작.
/// Open 시 가짜 암호를 전달해 보호된 문서의 모달 다이얼로그를 차단한다.
fn extract_text(path: &Path) -> windows::core::Result<String> {
    let app = Obj::create("Word.Application")?;
    let _guard = AppGuard::new(&app, AppKind::Word);
    // Word 를 백그라운드로 실행 — 경고·화면 갱신·매크로 모두 차단.
    let _ = app.put("Visible", var_bool(false));
    let _ = app.put("DisplayAlerts", var_i32(WD_ALERTS_NONE));
    let _ = app.put("ScreenUpdating", var_bool(false));
    let _ = app.put(
        "AutomationSecurity",
        var_i32(super::MSO_AUTOMATION_SECURITY_FORCE_DISABLE),
    );

    let documents = app.get_obj("Documents", &[])?;
    let path_str = super::path_arg(path);
    let doc_var = documents.call(
        "Open",
        &[
            var_str(&path_str),             // FileName — 절대 경로
            var_bool(false),                // ConfirmConversions — 변환 확인 대화상자 비활성
            var_bool(true),                 // ReadOnly — 읽기 전용
            var_bool(false),                // AddToRecentFiles — 최근 문서 목록 추가 안 함
            var_str(super::BOGUS_PASSWORD), // PasswordDocument — 가짜 암호 (보호 문서 즉시 실패)
        ],
    )?;
    let doc = super::as_obj(&doc_var)?;

    // Content 객체의 Text 속성에서 전체 문서 텍스트를 한 번에 읽는다.
    let content = doc.get_obj("Content", &[])?;
    let text_var = content.get("Text", &[])?;
    let text = v_string(&text_var);

    let _ = doc.call("Close", &[var_i32(WD_DO_NOT_SAVE_CHANGES)]);

    Ok(text)
}

/// 페이지별 텍스트 정보 (페이지 번호 + 텍스트 + 전문 기준 시작 오프셋).
struct PageText {
    page_number: usize,
    text: String,
    start_offset: usize,
}

/// 폼 피드(`\u{000C}`) 기준 페이지 분할.
/// Word `Content.Text` 는 페이지 경계를 폼 피드 문자로 구분한다.
fn split_pages(full: &str) -> Vec<PageText> {
    let mut pages: Vec<PageText> = Vec::new();
    let mut running_offset = 0usize;

    for (idx, raw_page) in full.split('\u{000C}').enumerate() {
        let text = normalize_paragraphs(raw_page);
        if text.is_empty() {
            continue;
        }
        let char_len = text.chars().count();
        pages.push(PageText {
            page_number: idx + 1,
            text,
            start_offset: running_offset,
        });
        running_offset += char_len + 1;
    }

    pages
}

/// 단락 구분자 정규화 — CR/LF/수직탭/벨/Shift-In 등을 `\n`으로 통일하고
/// 빈 줄을 제거한다.
fn normalize_paragraphs(page: &str) -> String {
    page.split(['\r', '\n', '\u{000B}', '\u{0007}', '\u{000E}'])
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// 페이지별 청크 분할 (페이지 번호·위치 힌트 유지).
/// 각 페이지 내부에서 `chunk_size` 크기로 분할하고 `overlap` 만큼 겹친다.
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
