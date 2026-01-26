use super::{DocumentChunk, DocumentMetadata, ParseError, ParsedDocument};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::Duration;

/// PDF 파싱 타임아웃 (초) - 대부분 5초 내 완료, hang 감지용
const PDF_PARSE_TIMEOUT_SECS: u64 = 10;

/// Detach된 PDF 파싱 스레드 카운터 (리소스 모니터링용)
/// 이 값이 높으면 hang되는 PDF가 많다는 의미
static DETACHED_THREAD_COUNT: AtomicUsize = AtomicUsize::new(0);

/// 현재 detach된 PDF 스레드 수 조회
pub fn detached_thread_count() -> usize {
    DETACHED_THREAD_COUNT.load(Ordering::Relaxed)
}

/// PDF 파일 파싱
/// pdf-extract 크레이트 사용, 페이지별 텍스트 추출
/// catch_unwind + 타임아웃으로 panic/hang 방어
pub fn parse(path: &Path) -> Result<ParsedDocument, ParseError> {
    // pdf-extract가 일부 PDF에서 내부 스레드 panic 발생 → 메인 스레드 hang
    // 별도 스레드 + 타임아웃으로 방어
    let path_owned = path.to_path_buf();
    let (tx, rx) = mpsc::channel();

    let handle = std::thread::spawn(move || {
        let result = catch_unwind(AssertUnwindSafe(|| pdf_extract::extract_text(&path_owned)));
        let _ = tx.send(result);
    });

    // 타임아웃 대기
    let raw_text = match rx.recv_timeout(Duration::from_secs(PDF_PARSE_TIMEOUT_SECS)) {
        Ok(Ok(Ok(text))) => text,
        Ok(Ok(Err(e))) => {
            return Err(ParseError::ParseError(format!("PDF extraction failed: {}", e)));
        }
        Ok(Err(_)) => {
            return Err(ParseError::ParseError(
                "PDF parser panicked (unsupported font encoding)".to_string(),
            ));
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            // 타임아웃 - JoinHandle drop 시 스레드는 detach되어 백그라운드에서 계속 실행됨
            // pdf_extract가 hang된 경우 스레드 리소스는 프로세스 종료 시까지 유지됨
            let count = DETACHED_THREAD_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
            tracing::warn!(
                "PDF parsing timed out after {}s, thread detached (total: {}): {:?}",
                PDF_PARSE_TIMEOUT_SECS,
                count,
                path
            );
            if count >= 10 {
                tracing::error!(
                    "High number of detached PDF threads: {}. Consider restarting the app.",
                    count
                );
            }
            std::mem::drop(handle); // 명시적 detach
            return Err(ParseError::ParseError(format!(
                "PDF parsing timed out after {}s (detached threads: {})",
                PDF_PARSE_TIMEOUT_SECS,
                count
            )));
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            return Err(ParseError::ParseError(
                "PDF parser thread crashed".to_string(),
            ));
        }
    };

    // 스레드 정상 종료 대기 (이미 완료됨)
    let _ = handle.join();

    // 페이지별 분리 (form feed 문자 \x0c 기준)
    let pages: Vec<&str> = raw_text.split('\x0c').collect();
    let page_count = pages.len();

    let mut all_text = String::new();
    let mut chunks = Vec::new();
    let mut global_offset = 0;

    for (page_idx, page_text) in pages.iter().enumerate() {
        let cleaned = clean_pdf_text(page_text);
        if cleaned.is_empty() {
            continue;
        }

        let page_number = page_idx + 1; // 1-based

        // 페이지별 청크 생성
        let page_chunks = chunk_text_with_page(&cleaned, 512, 64, page_number, global_offset);
        chunks.extend(page_chunks);

        if !all_text.is_empty() {
            all_text.push_str("\n\n");
            global_offset += 2;
        }
        global_offset += cleaned.len();
        all_text.push_str(&cleaned);
    }

    if all_text.is_empty() {
        tracing::warn!("PDF file has no text content: {:?}", path);
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

/// 페이지 정보 포함 청크 분할
fn chunk_text_with_page(
    text: &str,
    chunk_size: usize,
    overlap: usize,
    page_number: usize,
    base_offset: usize,
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
            start_offset: base_offset + start,
            end_offset: base_offset + end,
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

/// PDF 텍스트 정리
fn clean_pdf_text(text: &str) -> String {
    let mut result = String::new();
    let mut prev_was_newline = false;

    for line in text.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            if !prev_was_newline && !result.is_empty() {
                result.push('\n');
                prev_was_newline = true;
            }
        } else {
            if !result.is_empty() && !prev_was_newline {
                result.push(' ');
            }
            result.push_str(trimmed);
            prev_was_newline = false;
        }
    }

    result.trim().to_string()
}
