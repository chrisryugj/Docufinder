use super::{DocumentChunk, DocumentMetadata, ParseError, ParsedDocument};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::Duration;

/// PDF 파싱 기본 타임아웃 (초)
/// HDD에서 대용량 PDF는 디스크 읽기만으로 수 초 소요 → 여유있게 설정
const PDF_PARSE_TIMEOUT_BASE_SECS: u64 = 5;

/// MB당 추가 타임아웃 (초) — HDD 순차 읽기 ~100MB/s 감안, 안전 마진 포함
const PDF_PARSE_TIMEOUT_PER_MB: f64 = 0.3;

/// 최대 타임아웃 상한 (초) — 무한 대기 방지
const PDF_PARSE_TIMEOUT_MAX_SECS: u64 = 30;

/// 파일 크기 기반 동적 타임아웃 계산
fn calc_timeout_secs(path: &Path) -> u64 {
    let file_size_mb = std::fs::metadata(path)
        .map(|m| m.len() as f64 / 1_048_576.0)
        .unwrap_or(0.0);
    let timeout = PDF_PARSE_TIMEOUT_BASE_SECS as f64 + file_size_mb * PDF_PARSE_TIMEOUT_PER_MB;
    (timeout.ceil() as u64).min(PDF_PARSE_TIMEOUT_MAX_SECS)
}

/// Detach된 PDF 파싱 스레드 최대 수 (각 ~2-8MB 스택, 10개 = ~80MB 상한)
/// 8GB RAM 환경에서 메모리 오버헤드 최소화
const MAX_DETACHED_THREADS: usize = 10;

/// Detach된 PDF 파싱 스레드 카운터 (리소스 모니터링용)
/// 이 값이 높으면 hang되는 PDF가 많다는 의미
static DETACHED_THREAD_COUNT: AtomicUsize = AtomicUsize::new(0);

/// PDF 파일 파싱
/// pdf-extract 크레이트 사용, 페이지별 텍스트 추출
/// catch_unwind + 타임아웃으로 panic/hang 방어
pub fn parse(path: &Path) -> Result<ParsedDocument, ParseError> {
    // hang 스레드 상한 체크 - 시스템 안정성 보호
    let current_detached = DETACHED_THREAD_COUNT.load(Ordering::Relaxed);
    if current_detached >= MAX_DETACHED_THREADS {
        return Err(ParseError::ParseError(format!(
            "PDF 파싱 중단: hang 스레드 {}개 초과 (상한 {}). 앱 재시작을 권장합니다.",
            current_detached, MAX_DETACHED_THREADS
        )));
    }

    // pdf-extract가 일부 PDF에서 내부 스레드 panic 발생 → 메인 스레드 hang
    // 별도 스레드 + 타임아웃으로 방어
    let timeout_secs = calc_timeout_secs(path);
    let path_owned = path.to_path_buf();
    let (tx, rx) = mpsc::channel();

    let handle = std::thread::spawn(move || {
        let result = catch_unwind(AssertUnwindSafe(|| pdf_extract::extract_text(&path_owned)));
        let _ = tx.send(result);
    });

    // 동적 타임아웃 대기 (파일 크기 기반)
    let raw_text = match rx.recv_timeout(Duration::from_secs(timeout_secs)) {
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
            // 타임아웃 - 별도 경량 클린업 스레드가 원본 스레드 완료를 대기 후 카운터 감소
            let count = DETACHED_THREAD_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
            tracing::warn!(
                "PDF parsing timed out after {}s, thread detached (total: {}): {:?}",
                timeout_secs,
                count,
                path
            );
            if count >= 10 {
                tracing::error!(
                    "High number of detached PDF threads: {}. Consider restarting the app.",
                    count
                );
            }
            // 클린업 스레드: 원본 스레드 완료 시 카운터 감소 (최소 스택으로 오버헤드 최소화)
            // spawn 실패 시 즉시 카운터 감소하여 누수 방지
            let cleanup_result = std::thread::Builder::new()
                .name("pdf-cleanup".into())
                .stack_size(64 * 1024) // 64KB 최소 스택
                .spawn(move || {
                    let _ = handle.join();
                    DETACHED_THREAD_COUNT.fetch_sub(1, Ordering::Relaxed);
                    tracing::debug!("Detached PDF thread completed and reclaimed");
                });
            if cleanup_result.is_err() {
                DETACHED_THREAD_COUNT.fetch_sub(1, Ordering::Relaxed);
                tracing::error!("Failed to spawn PDF cleanup thread, counter corrected");
            }
            return Err(ParseError::ParseError(format!(
                "PDF parsing timed out after {}s (detached threads: {})",
                timeout_secs,
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
        let page_chunks = chunk_text_with_page(&cleaned, super::DEFAULT_CHUNK_SIZE, super::DEFAULT_CHUNK_OVERLAP, page_number, global_offset);
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
            page_end: Some(page_number),
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
