use super::{DocumentChunk, DocumentMetadata, ParseError, ParsedDocument};
use crate::ocr::OcrEngine;
use pdfium_render::prelude::{PdfDocument, PdfRenderConfig, Pdfium};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::Duration;

/// PDF 파싱 기본 타임아웃 (초)
/// HDD에서 대용량 PDF는 디스크 읽기만으로 수 초 소요 → 여유있게 설정
const PDF_PARSE_TIMEOUT_BASE_SECS: u64 = 5;

/// MB당 추가 타임아웃 (초) — HDD 순차 읽기 ~100MB/s 감안, 안전 마진 포함
const PDF_PARSE_TIMEOUT_PER_MB: f64 = 0.3;

/// 최대 타임아웃 상한 (초) — 무한 대기 방지
const PDF_PARSE_TIMEOUT_MAX_SECS: u64 = 30;

/// 스캔 페이지 판정 기준: 이 글자 수 미만이면 스캔 페이지로 간주
const SCANNED_PAGE_CHAR_THRESHOLD: usize = 10;

/// OCR 대상 최대 페이지 수 (성능 보호: 300페이지 스캔 PDF → 20페이지만 OCR)
const MAX_OCR_PAGES: usize = 20;

/// OCR 스킵 파일 크기 (100MB 초과 시 스캔 PDF OCR 건너뛰기)
const MAX_OCR_FILE_SIZE: u64 = 100 * 1024 * 1024;

/// OCR 입력 이미지 최대 폭 (큰 이미지 리사이즈 → OCR 속도 2~3배 향상)
const MAX_OCR_IMAGE_WIDTH: u32 = 2000;

/// pdfium 래스터화 렌더 최대 높이 (px). 폭만 지정하면 극단적 종횡비 MediaBox(예: 2×2000)는
/// 높이가 종횡비대로 스케일돼 수백만 px 비트맵을 할당하려 한다(임베디드 경로의 MAX_IMAGE_PIXELS
/// 방어와 대칭이 되도록 상한을 건다). 2000×10000 ≈ 20M px(≈80MB) — 정상 문서는 절대 도달하지
/// 않고 병리적 페이지만 이 상한에 걸려 축소(letterbox)된다.
const MAX_OCR_RENDER_HEIGHT: u32 = 10000;

/// 파일 크기 기반 동적 타임아웃 계산
fn calc_timeout_secs(path: &Path) -> u64 {
    let file_size_mb = std::fs::metadata(path)
        .map(|m| m.len() as f64 / 1_048_576.0)
        .unwrap_or(0.0);
    let timeout = PDF_PARSE_TIMEOUT_BASE_SECS as f64 + file_size_mb * PDF_PARSE_TIMEOUT_PER_MB;
    (timeout.ceil() as u64).min(PDF_PARSE_TIMEOUT_MAX_SECS)
}

/// Detach된 PDF 파싱 스레드 최대 수 (각 ~2-8MB 스택, 20개 = ~160MB 상한)
const MAX_DETACHED_THREADS: usize = 20;

/// 자동 리셋 간격 (초) — 이 시간 경과 후 카운터가 절반 이상이면 자동 리셋
const AUTO_RESET_INTERVAL_SECS: u64 = 300; // 5분

/// Detach된 PDF 파싱 스레드 카운터 (리소스 모니터링용)
/// 이 값이 높으면 hang되는 PDF가 많다는 의미
static DETACHED_THREAD_COUNT: AtomicUsize = AtomicUsize::new(0);

/// 마지막 자동 리셋 시각 (Unix timestamp 초)
static LAST_AUTO_RESET: AtomicU64 = AtomicU64::new(0);

/// 시간 경과 + 카운터 높으면 자동 리셋 (parse 진입 시 호출)
fn try_auto_reset() {
    let current = DETACHED_THREAD_COUNT.load(Ordering::Relaxed);
    if current < MAX_DETACHED_THREADS / 2 {
        return; // 절반 미만이면 리셋 불필요
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let last = LAST_AUTO_RESET.load(Ordering::Relaxed);

    if now.saturating_sub(last) >= AUTO_RESET_INTERVAL_SECS {
        // CAS로 중복 리셋 방지
        if LAST_AUTO_RESET
            .compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            let prev = DETACHED_THREAD_COUNT.swap(0, Ordering::Relaxed);
            tracing::warn!(
                "PDF detached thread counter auto-reset: {} → 0 (after {}s idle)",
                prev,
                now.saturating_sub(last)
            );
        }
    }
}

/// Detached thread 카운터 리셋 (앱 재시작 없이 PDF 파싱 재개)
///
/// hang 스레드가 MAX_DETACHED_THREADS에 도달하면 모든 PDF 파싱이 차단됨.
/// 이 함수로 카운터만 리셋하여 새 파싱 허용. 기존 hang 스레드는 OS 레벨에서 유지됨.
pub fn reset_detached_thread_count() -> usize {
    let prev = DETACHED_THREAD_COUNT.swap(0, Ordering::Relaxed);
    if prev > 0 {
        tracing::warn!(
            "PDF detached thread counter reset: {} → 0 (some threads may still be running)",
            prev
        );
    }
    prev
}

/// 현재 detached thread 수 조회
pub fn detached_thread_count() -> usize {
    DETACHED_THREAD_COUNT.load(Ordering::Relaxed)
}

/// PDF 파일 파싱
/// pdf-extract 크레이트 사용, 페이지별 텍스트 추출
/// catch_unwind + 타임아웃으로 panic/hang 방어
///
/// `ocr`: OCR 엔진이 있으면 스캔 페이지(텍스트 10자 미만)에서 이미지 추출 → OCR
pub fn parse(path: &Path, ocr: Option<&OcrEngine>) -> Result<ParsedDocument, ParseError> {
    // 파일 크기 체크 (다른 파서와 동일, 대용량 PDF OOM 방지)
    if let Ok(metadata) = std::fs::metadata(path) {
        if metadata.len() > super::MAX_FILE_SIZE {
            return Err(ParseError::ParseError(format!(
                "PDF 파일 크기 초과: {}MB (최대 {}MB)",
                metadata.len() / 1024 / 1024,
                super::MAX_FILE_SIZE / 1024 / 1024
            )));
        }
    }

    // 시간 기반 자동 리셋 (hang 스레드 누적 시 5분 후 자동 복구)
    try_auto_reset();

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

    // 페이지별 추출(extract_text_by_pages) — extract_text 는 페이지 경계 없이 전체를 하나의
    // 문자열로 반환한다(PlainTextOutput 은 form feed 를 출력하지 않아 '\x0c' split 이 항상
    // 1원소). 페이지 귀속(page_number)·스캔 페이지 판정·페이지별 OCR 전부 페이지 벡터가 전제.
    let handle = std::thread::spawn(move || {
        let result = catch_unwind(AssertUnwindSafe(|| {
            pdf_extract::extract_text_by_pages(&path_owned)
        }));
        let _ = tx.send(result);
    });

    // 동적 타임아웃 대기 (파일 크기 기반)
    let raw_pages = match rx.recv_timeout(Duration::from_secs(timeout_secs)) {
        Ok(Ok(Ok(pages))) => pages,
        Ok(Ok(Err(e))) => {
            let msg = e.to_string().to_lowercase();
            if msg.contains("password") || msg.contains("encrypt") {
                return Err(ParseError::PasswordProtected(
                    "암호로 보호된 PDF 파일입니다".to_string(),
                ));
            }
            return Err(ParseError::ParseError(format!(
                "PDF extraction failed: {}",
                e
            )));
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
                timeout_secs, count
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

    // 페이지 0개 = 손상 PDF(스펙상 페이지 ≥1) 또는 1페이지부터 추출 실패.
    // 기존 extract_text 는 이 경우 Err 를 반환했으므로 조용한 빈 문서 대신 에러로 유지.
    if raw_pages.is_empty() {
        return Err(ParseError::ParseError(
            "PDF text extraction returned no pages".to_string(),
        ));
    }

    let page_count = raw_pages.len();

    // 추출 텍스트 크기 제한 (50MB — 대용량 PDF OOM 방지, 페이지 누적 기준)
    const MAX_EXTRACTED_TEXT_SIZE: usize = 50 * 1024 * 1024;
    let mut raw_pages = raw_pages;
    let mut acc = 0usize;
    for i in 0..raw_pages.len() {
        let remain = MAX_EXTRACTED_TEXT_SIZE - acc; // 루프 불변식: acc ≤ MAX
        if raw_pages[i].len() > remain {
            tracing::warn!(
                "PDF text truncated at page {}: total > {}MB: {:?}",
                i + 1,
                MAX_EXTRACTED_TEXT_SIZE / 1_048_576,
                path
            );
            raw_pages[i].truncate(remain);
            // char 경계 안전하게 자르기
            while !raw_pages[i].is_char_boundary(raw_pages[i].len()) {
                raw_pages[i].pop();
            }
            raw_pages.truncate(i + 1);
            break;
        }
        acc += raw_pages[i].len();
    }

    // 페이지별 정리 — 스캔 판정·OCR·본문 조립이 같은 정리본을 공유
    let cleaned_pages: Vec<String> = raw_pages.iter().map(|p| clean_pdf_text(p)).collect();
    drop(raw_pages);

    // OCR 필요 여부 판정 + OCR 결과 (필요 시에만 수행)
    let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let ocr_texts: Vec<Option<String>> = if let Some(ocr_engine) = ocr {
        if file_size > MAX_OCR_FILE_SIZE {
            tracing::info!(
                "PDF too large for OCR ({:.1}MB > {}MB): {:?}",
                file_size as f64 / 1_048_576.0,
                MAX_OCR_FILE_SIZE / 1_048_576,
                path
            );
            vec![]
        } else {
            // 스캔 페이지 존재 여부: (1) 텍스트 10자 미만 OR (2) CID 디코딩 실패로 깨진 페이지
            let has_scanned = cleaned_pages.iter().any(|p| {
                p.chars().count() < SCANNED_PAGE_CHAR_THRESHOLD || looks_like_garbage_text(p)
            });
            if has_scanned {
                ocr_scanned_pages(path, &cleaned_pages, ocr_engine)
            } else {
                vec![]
            }
        }
    } else {
        vec![]
    };

    // 페이지별 처리: chunk → all_text에 추가
    let mut all_text =
        String::with_capacity(cleaned_pages.iter().map(|p| p.len() + 2).sum::<usize>());
    let mut chunks = Vec::new();
    let mut global_offset = 0;

    for (page_idx, cleaned) in cleaned_pages.iter().enumerate() {
        // OCR 결과가 있으면 대체
        let page_text: &str = if let Some(Some(ocr_text)) = ocr_texts.get(page_idx) {
            ocr_text.as_str()
        } else {
            cleaned.as_str()
        };

        if page_text.is_empty() {
            continue;
        }

        // CID 디코딩 실패로 깨진 페이지에 OCR 도 실패하면 저장하지 않음
        // (검색 인덱스에 쓰레기 문자가 들어가는 것을 방지)
        if looks_like_garbage_text(page_text) {
            tracing::warn!(
                "PDF page {} has garbage-looking text (CID decode failed + OCR unavailable), skipping: {:?}",
                page_idx + 1,
                path
            );
            continue;
        }

        let page_number = page_idx + 1; // 1-based

        // 페이지별 청크 생성
        let page_chunks = chunk_text_with_page(
            page_text,
            super::DEFAULT_CHUNK_SIZE,
            super::DEFAULT_CHUNK_OVERLAP,
            page_number,
            global_offset,
        );
        chunks.extend(page_chunks);

        if !all_text.is_empty() {
            all_text.push_str("\n\n");
            global_offset += 2;
        }
        global_offset += page_text.len();
        all_text.push_str(page_text);
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

// ============================================================================
// 스캔 PDF OCR — lopdf로 임베디드 이미지 추출 후 OCR
// ============================================================================

/// 스캔 페이지에서 이미지 추출 + OCR
fn ocr_scanned_pages(path: &Path, page_texts: &[String], ocr: &OcrEngine) -> Vec<Option<String>> {
    let doc = match lopdf::Document::load(path) {
        Ok(d) => d,
        Err(e) => {
            tracing::debug!("lopdf failed to open PDF for OCR: {}", e);
            return vec![None; page_texts.len()];
        }
    };

    let pages = doc.get_pages(); // BTreeMap<u32, ObjectId>

    let mut results: Vec<Option<String>> = vec![None; page_texts.len()];
    // pdfium 래스터화가 필요한 페이지(임베디드 이미지 추출 실패 — 미지원 코덱). Pass 1 에서
    // 큐잉하고, Pass 2 에서 pdfium 문서를 딱 1회 로드해 페이지 인덱스로 렌더한다 —
    // 페이지마다 문서 전체를 재파싱하지 않는다.
    let mut needs_raster: Vec<usize> = Vec::new();
    let mut ocr_count = 0usize;

    // ── Pass 1: 텍스트 스킵/예산 판정 + 임베디드 이미지 추출 OCR. 추출 실패는 래스터 큐로.
    for (page_idx, text) in page_texts.iter().enumerate() {
        // 텍스트 충분한 페이지는 스킵 (단, CID 디코딩 실패 페이지는 OCR 강제).
        if text.chars().count() >= SCANNED_PAGE_CHAR_THRESHOLD && !looks_like_garbage_text(text) {
            continue;
        }
        // OCR 페이지 수 제한 (성능 보호). 예산은 임베디드·래스터 OCR 를 합산해 문서 순서로 소진.
        if ocr_count >= MAX_OCR_PAGES {
            if ocr_count == MAX_OCR_PAGES {
                tracing::info!(
                    "PDF OCR page limit reached ({}), skipping remaining pages",
                    MAX_OCR_PAGES
                );
                ocr_count += 1; // 로그는 한 번만
            }
            continue;
        }
        ocr_count += 1;

        let page_num = (page_idx + 1) as u32;
        let page_id = match pages.get(&page_num) {
            Some(id) => *id,
            None => continue,
        };

        // 회전 페이지(/Rotate ≠ 0)는 임베디드 추출을 건너뛰고 래스터 큐로 — 임베디드 이미지는
        // 회전 미반영이라 누운 글자를 OCR 해 빈/쓰레기가 되지만, pdfium 렌더는 /Rotate 를
        // 반영한다.
        if page_rotation(&doc, page_id) != 0 {
            needs_raster.push(page_idx);
            continue;
        }

        // 우선순위 ① 임베디드 이미지 추출(빠름). 실패 시(미지원 코덱 — CCITTFax/JBIG2/JPX/CMYK
        // 등) 페이지를 래스터 큐에 넣어 Pass 2 에서 pdfium 으로 통째 렌더한다.
        match extract_page_image(&doc, page_id) {
            Some(image) => results[page_idx] = ocr_image_to_text(ocr, &image, page_num),
            None => needs_raster.push(page_idx),
        }
    }

    // ── Pass 2: 래스터 대상이 있으면 pdfium 을 문서당 1회 바인딩+로드해 인덱스로 렌더 → OCR.
    // 모든 페이지가 임베디드로 처리되면 dlopen·문서 로드 자체가 일어나지 않는다.
    if !needs_raster.is_empty() {
        if let Some(pdfium) = bind_pdfium() {
            // 로드는 보통 Result 로 실패하지만, 만일의 pdfium 내부 패닉이 parse 전체를 실패시켜
            // 추출된 텍스트·임베디드 OCR 까지 잃지 않도록 catch_unwind 로 방어(래스터화만 생략).
            let loaded = catch_unwind(AssertUnwindSafe(|| pdfium.load_pdf_from_file(path, None)));
            match loaded {
                Ok(Ok(document)) => {
                    for &page_idx in &needs_raster {
                        let page_num = (page_idx + 1) as u32;
                        match rasterize_doc_page(&document, page_idx, MAX_OCR_IMAGE_WIDTH) {
                            Some(image) => {
                                tracing::info!(
                                    "PDF page {} rasterized via pdfium for OCR ({}x{})",
                                    page_num,
                                    image.width(),
                                    image.height()
                                );
                                // OCR 패닉이 pdfium 문서 스코프를 unwind 로 관통하면 마샬
                                // 뮤텍스가 poison 되어 세션 내내 스캔 PDF 래스터화가 전멸한다
                                // — 여기서 흡수하고 해당 페이지만 스킵.
                                results[page_idx] = catch_unwind(AssertUnwindSafe(|| {
                                    ocr_image_to_text(ocr, &image, page_num)
                                }))
                                .unwrap_or_else(|_| {
                                    tracing::warn!("PDF page {} OCR panicked, skipping", page_num);
                                    None
                                });
                            }
                            None => {
                                tracing::debug!("No extractable image in scanned page {}", page_num)
                            }
                        }
                    }
                }
                Ok(Err(e)) => {
                    tracing::debug!("pdfium 문서 로드 실패, 래스터화 생략: {:?} ({})", path, e)
                }
                Err(_) => {
                    tracing::warn!("pdfium 문서 로드 중 panic, 래스터화 생략: {:?}", path)
                }
            }
        }
    }

    results
}

/// 페이지 /Rotate 조회 (PDF 스펙상 Pages 트리에서 상속 가능 — Parent 체인 추적).
/// 0/90/180/270 정규화 값 반환, 미지정·해석 불가는 0. 체인 깊이 제한은 순환 참조 방어.
fn page_rotation(doc: &lopdf::Document, page_id: lopdf::ObjectId) -> i64 {
    let mut dict = match doc.get_object(page_id).ok().and_then(|o| o.as_dict().ok()) {
        Some(d) => d,
        None => return 0,
    };
    for _ in 0..32 {
        if let Some(r) = resolve_integer(doc, dict, b"Rotate") {
            return r.rem_euclid(360);
        }
        match dict
            .get(b"Parent")
            .ok()
            .and_then(|p| p.as_reference().ok())
            .and_then(|id| doc.get_object(id).ok())
            .and_then(|o| o.as_dict().ok())
        {
            Some(parent) => dict = parent,
            None => return 0,
        }
    }
    0
}

/// 페이지에서 가장 큰 이미지 추출 (스캔 PDF: 페이지당 1개 이미지가 일반적)
fn extract_page_image(
    doc: &lopdf::Document,
    page_id: lopdf::ObjectId,
) -> Option<image::DynamicImage> {
    let page_obj = doc.get_object(page_id).ok()?;
    let page_dict = page_obj.as_dict().ok()?;

    // Resources 딕셔너리 (직접 또는 간접 참조)
    let resources = get_dict_value(doc, page_dict, b"Resources")?;
    let xobjects = get_dict_value(doc, resources, b"XObject")?;

    let mut largest: Option<(usize, image::DynamicImage)> = None;

    for (_, obj_ref) in xobjects.iter() {
        if let Ok(stream) = resolve_stream(doc, obj_ref) {
            let dict = &stream.dict;

            // Image XObject만 처리
            let subtype = dict.get(b"Subtype").ok().and_then(|s| resolve_name(doc, s));
            if subtype.as_deref() != Some("Image") {
                continue;
            }

            let width = resolve_integer(doc, dict, b"Width").unwrap_or(0) as u32;
            let height = resolve_integer(doc, dict, b"Height").unwrap_or(0) as u32;
            if width == 0 || height == 0 {
                continue;
            }

            let size = (width * height) as usize;
            if let Some((best_size, _)) = &largest {
                if size <= *best_size {
                    continue;
                }
            }

            // 이미지 디코딩
            if let Some(img) = decode_pdf_image(doc, stream, width, height) {
                largest = Some((size, img));
            }
        }
    }

    largest.map(|(_, img)| {
        // 큰 이미지 리사이즈 (OCR 속도 향상 + 메모리 절약)
        if img.width() > MAX_OCR_IMAGE_WIDTH {
            let ratio = MAX_OCR_IMAGE_WIDTH as f64 / img.width() as f64;
            let new_height = (img.height() as f64 * ratio) as u32;
            img.resize(
                MAX_OCR_IMAGE_WIDTH,
                new_height,
                image::imageops::FilterType::Lanczos3,
            )
        } else {
            img
        }
    })
}

// ============================================================================
// pdfium 페이지 래스터화 fallback — 임베디드 이미지 추출이 실패하는 코덱 우회
// ============================================================================

/// 준비된 이미지를 OCR 해 텍스트를 뽑는다. 빈 결과·에러는 `None`(해당 페이지 스킵).
fn ocr_image_to_text(
    ocr: &OcrEngine,
    image: &image::DynamicImage,
    page_num: u32,
) -> Option<String> {
    match ocr.recognize_image(image) {
        Ok(result) => {
            let ocr_text = result.text.trim().to_string();
            if ocr_text.is_empty() {
                None
            } else {
                tracing::info!(
                    "PDF page {} OCR: {} chars extracted",
                    page_num,
                    ocr_text.len()
                );
                Some(ocr_text)
            }
        }
        Err(e) => {
            tracing::debug!("OCR failed for PDF page {}: {}", page_num, e);
            None
        }
    }
}

/// 이미 로드된 pdfium 문서에서 페이지 하나를 이미지로 래스터화 (스캔 페이지 OCR fallback).
///
/// CCITTFax/JBIG2/JPX/CMYK 등 `decode_pdf_image` 가 지원하지 않는 코덱의 스캔본을 OCR 하기
/// 위해 페이지 자체를 렌더한다. 문서는 호출부가 문서당 1회 로드해 넘겨준다 — 페이지마다
/// 재파싱하지 않는다. render 중 예기치 못한 패닉은 `catch_unwind` 로 방어해 `None` 을 반환한다
/// (크래시 금지). `target_width` px 기준으로 렌더해 기존 OCR 이미지 폭 상한을 준수한다.
fn rasterize_doc_page(
    document: &PdfDocument<'_>,
    page_index: usize,
    target_width: u32,
) -> Option<image::DynamicImage> {
    // pdfium 페이지 인덱스는 u16 — 65,535 초과 페이지(극단적 대용량 PDF)를 조용히 절단하지
    // 않고 렌더 불가로 건너뛴다.
    let page_index = u16::try_from(page_index).ok()?;
    // pdfium 내부에서의 예기치 못한 패닉까지 방어 (OCR 경로는 catch_unwind 밖에서 실행됨).
    let rendered = catch_unwind(AssertUnwindSafe(|| {
        let page = document.pages().get(page_index).ok()?;
        let config = PdfRenderConfig::new()
            .set_target_width(target_width as i32)
            .set_maximum_height(MAX_OCR_RENDER_HEIGHT as i32);
        let bitmap = page.render_with_config(&config).ok()?;
        Some(bitmap.as_image())
    }));
    match rendered {
        Ok(img) => img,
        Err(_) => {
            tracing::warn!("pdfium 렌더 중 panic (page {})", page_index + 1);
            None
        }
    }
}

/// `PDFIUM_DYLIB_PATH` 로 pdfium 을 런타임 바인딩. 미설정/미존재/실패 시 `None`.
///
/// onnxruntime(`ORT_DYLIB_PATH`)과 동일하게 환경변수로 dylib 경로를 받는다. 값이 없거나 파일이
/// 없으면 스캔 PDF 래스터화 기능만 비활성될 뿐, 기존 PDF 파싱 경로는 영향받지 않는다.
fn bind_pdfium() -> Option<Pdfium> {
    let raw = std::env::var_os("PDFIUM_DYLIB_PATH")?;
    let dylib_path = std::path::PathBuf::from(raw);
    if !dylib_path.exists() {
        tracing::debug!(
            "PDFIUM_DYLIB_PATH 미존재 ({:?}) — 스캔 PDF 래스터화 fallback 비활성",
            dylib_path
        );
        return None;
    }
    match Pdfium::bind_to_library(&dylib_path) {
        Ok(bindings) => Some(Pdfium::new(bindings)),
        Err(e) => {
            tracing::debug!("pdfium 바인딩 실패 ({:?}): {}", dylib_path, e);
            None
        }
    }
}

/// 단일 이미지 최대 픽셀 수 (100M 픽셀 ≈ 10000×10000)
///
/// 악성 PDF가 `Width=65535, Height=65535` 같은 값으로 멀티GB 버퍼 할당을 유도하는
/// OOM 공격 차단용. decompress_flate의 50MB 상한과 함께 2중 방어.
const MAX_IMAGE_PIXELS: u64 = 100_000_000;

/// PDF 이미지 스트림 디코딩
fn decode_pdf_image(
    doc: &lopdf::Document,
    stream: &lopdf::Stream,
    width: u32,
    height: u32,
) -> Option<image::DynamicImage> {
    // 픽셀 수 상한 — 비정상 width/height로 인한 OOM 방어
    if (width as u64).saturating_mul(height as u64) > MAX_IMAGE_PIXELS {
        tracing::debug!(
            "PDF 이미지 픽셀 초과, 스킵: {}x{} > {} pixels",
            width,
            height,
            MAX_IMAGE_PIXELS
        );
        return None;
    }

    let filter = get_filter_name(&stream.dict);

    match filter.as_deref() {
        Some("DCTDecode") => {
            // JPEG — 스트림 데이터가 곧 JPEG 바이트
            let data = &stream.content;
            image::load_from_memory_with_format(data, image::ImageFormat::Jpeg).ok()
        }
        Some("FlateDecode") => {
            // zlib 압축 raw 픽셀 → flate2로 디코딩
            let decoded = decompress_flate(&stream.content)?;
            let bpc = resolve_integer(doc, &stream.dict, b"BitsPerComponent").unwrap_or(8);
            if bpc != 8 {
                return None; // 8비트가 아닌 경우 미지원
            }

            let cs = get_colorspace(&stream.dict);
            match cs.as_deref() {
                Some("DeviceRGB") | Some("RGB") => {
                    let expected = (width * height * 3) as usize;
                    if decoded.len() < expected {
                        return None;
                    }
                    image::RgbImage::from_raw(width, height, decoded)
                        .map(image::DynamicImage::ImageRgb8)
                }
                Some("DeviceGray") | Some("Gray") => {
                    let expected = (width * height) as usize;
                    if decoded.len() < expected {
                        return None;
                    }
                    image::GrayImage::from_raw(width, height, decoded)
                        .map(image::DynamicImage::ImageLuma8)
                }
                _ => None, // CMYK 등 미지원
            }
        }
        None => {
            // 비압축 raw 픽셀
            let data = &stream.content;
            let bpc = resolve_integer(doc, &stream.dict, b"BitsPerComponent").unwrap_or(8);
            if bpc != 8 {
                return None;
            }
            let cs = get_colorspace(&stream.dict);
            match cs.as_deref() {
                Some("DeviceRGB") | Some("RGB") => {
                    image::RgbImage::from_raw(width, height, data.clone())
                        .map(image::DynamicImage::ImageRgb8)
                }
                Some("DeviceGray") | Some("Gray") => {
                    image::GrayImage::from_raw(width, height, data.clone())
                        .map(image::DynamicImage::ImageLuma8)
                }
                _ => None,
            }
        }
        _ => None, // JBIG2, CCITTFax 등 미지원
    }
}

/// FlateDecode (zlib) 디코딩
fn decompress_flate(data: &[u8]) -> Option<Vec<u8>> {
    use flate2::read::ZlibDecoder;
    use std::io::Read;

    const MAX_DECOMPRESSED_SIZE: u64 = 50 * 1024 * 1024; // 50MB 상한 (디컴프레션 폭탄 방어)
    let mut decoder = ZlibDecoder::new(data).take(MAX_DECOMPRESSED_SIZE);
    let mut decoded = Vec::new();
    decoder.read_to_end(&mut decoded).ok()?;
    Some(decoded)
}

// ============================================================================
// lopdf 헬퍼 함수
// ============================================================================

/// 딕셔너리에서 값을 가져오되, 간접 참조면 따라감
fn get_dict_value<'a>(
    doc: &'a lopdf::Document,
    dict: &'a lopdf::Dictionary,
    key: &[u8],
) -> Option<&'a lopdf::Dictionary> {
    let obj = dict.get(key).ok()?;
    match obj {
        lopdf::Object::Dictionary(d) => Some(d),
        lopdf::Object::Reference(id) => doc.get_object(*id).ok().and_then(|o| o.as_dict().ok()),
        _ => None,
    }
}

/// 간접 참조를 따라가서 Stream 가져오기
fn resolve_stream<'a>(
    doc: &'a lopdf::Document,
    obj: &'a lopdf::Object,
) -> Result<&'a lopdf::Stream, ()> {
    match obj {
        lopdf::Object::Stream(s) => Ok(s),
        lopdf::Object::Reference(id) => doc
            .get_object(*id)
            .map_err(|_| ())
            .and_then(|o| o.as_stream().map_err(|_| ())),
        _ => Err(()),
    }
}

/// Name 객체 해석 (간접 참조 포함)
fn resolve_name(doc: &lopdf::Document, obj: &lopdf::Object) -> Option<String> {
    match obj {
        lopdf::Object::Name(n) => String::from_utf8(n.clone()).ok(),
        lopdf::Object::Reference(id) => doc.get_object(*id).ok().and_then(|o| {
            if let lopdf::Object::Name(n) = o {
                String::from_utf8(n.clone()).ok()
            } else {
                None
            }
        }),
        _ => None,
    }
}

/// 딕셔너리에서 정수 값 가져오기 (간접 참조 포함)
fn resolve_integer(doc: &lopdf::Document, dict: &lopdf::Dictionary, key: &[u8]) -> Option<i64> {
    let obj = dict.get(key).ok()?;
    match obj {
        lopdf::Object::Integer(i) => Some(*i),
        lopdf::Object::Reference(id) => doc.get_object(*id).ok().and_then(|o| {
            if let lopdf::Object::Integer(i) = o {
                Some(*i)
            } else {
                None
            }
        }),
        _ => None,
    }
}

/// Filter 이름 추출 (단일 또는 배열의 첫 번째)
fn get_filter_name(dict: &lopdf::Dictionary) -> Option<String> {
    let filter = dict.get(b"Filter").ok()?;
    match filter {
        lopdf::Object::Name(n) => String::from_utf8(n.clone()).ok(),
        lopdf::Object::Array(arr) => arr.first().and_then(|f| {
            if let lopdf::Object::Name(n) = f {
                String::from_utf8(n.clone()).ok()
            } else {
                None
            }
        }),
        _ => None,
    }
}

/// ColorSpace 이름 추출
fn get_colorspace(dict: &lopdf::Dictionary) -> Option<String> {
    let cs = dict.get(b"ColorSpace").ok()?;
    match cs {
        lopdf::Object::Name(n) => String::from_utf8(n.clone()).ok(),
        lopdf::Object::Array(arr) => arr.first().and_then(|f| {
            if let lopdf::Object::Name(n) = f {
                String::from_utf8(n.clone()).ok()
            } else {
                None
            }
        }),
        _ => None,
    }
}

// ============================================================================
// 기존 유틸리티 함수
// ============================================================================

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

/// CID 디코딩 실패로 깨진 텍스트 감지.
///
/// Adobe InDesign 등에서 한글/한자 폰트를 Identity-H CID 로 임베드하면서
/// ToUnicode CMap 을 누락하면 pdf-extract/pdfjs 가 CID 를 복원하지 못해
/// - ASCII 제어 문자 (< 0x20, 공백류 제외)
/// - Private Use Area (U+E000~U+F8FF)
/// - 랜덤 한자 (한글/가나 미포함 CJK) 가 섞여 나옴.
///
/// 정상 한국어/영어 텍스트는 제어 문자가 거의 없고, Hangul/Latin 비중이 지배적.
/// 이런 깨진 페이지는 스캔 페이지처럼 OCR 로 fallback 해야 함.
pub(crate) fn looks_like_garbage_text(text: &str) -> bool {
    let total = text.chars().count();
    if total < SCANNED_PAGE_CHAR_THRESHOLD {
        return false; // 짧은 텍스트는 별도 '스캔 페이지' 경로에서 처리
    }

    let mut control = 0usize;
    let mut pua = 0usize;
    let mut readable = 0usize; // Hangul + Latin alnum + 공백

    for c in text.chars() {
        match c as u32 {
            0x00..=0x08 | 0x0B | 0x0E..=0x1F => control += 1,
            0xE000..=0xF8FF => pua += 1,
            _ => {}
        }
        let is_hangul = matches!(c as u32,
            0xAC00..=0xD7A3 | 0x1100..=0x11FF | 0x3130..=0x318F);
        let is_latin_alnum = c.is_ascii_alphanumeric();
        let is_space = c.is_whitespace();
        if is_hangul || is_latin_alnum || is_space {
            readable += 1;
        }
    }

    let control_ratio = control as f64 / total as f64;
    let pua_ratio = pua as f64 / total as f64;
    let readable_ratio = readable as f64 / total as f64;

    // 제어 문자 5% 이상 OR PUA 5% 이상 OR 한글+라틴+공백 비율 30% 미만
    control_ratio > 0.05 || pua_ratio > 0.05 || readable_ratio < 0.30
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

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{dictionary, Document, Object};

    /// /Rotate 직접 지정·Pages 상속·미지정 기본 0·음수 정규화 (리뷰 #6 — 회전 스캔은
    /// 임베디드 추출 대신 needs_raster 강등의 판정 함수)
    #[test]
    fn test_page_rotation_direct_inherited_default() {
        let mut doc = Document::with_version("1.4");
        let pages_id = doc.new_object_id();
        let p_direct = doc.add_object(dictionary! {
            "Type" => "Page", "Parent" => Object::Reference(pages_id), "Rotate" => 90
        });
        let p_inherit = doc.add_object(dictionary! {
            "Type" => "Page", "Parent" => Object::Reference(pages_id)
        });
        let p_negative = doc.add_object(dictionary! {
            "Type" => "Page", "Parent" => Object::Reference(pages_id), "Rotate" => -90
        });
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![
                    Object::Reference(p_direct),
                    Object::Reference(p_inherit),
                    Object::Reference(p_negative),
                ],
                "Count" => 3,
                "Rotate" => 180
            }),
        );

        assert_eq!(page_rotation(&doc, p_direct), 90);
        assert_eq!(page_rotation(&doc, p_inherit), 180, "Pages 트리 상속");
        assert_eq!(page_rotation(&doc, p_negative), 270, "-90 → 270 정규화");

        // 상속 원천도 없으면 0 (기본)
        let mut doc2 = Document::with_version("1.4");
        let pages2 = doc2.new_object_id();
        let p_plain = doc2.add_object(dictionary! {
            "Type" => "Page", "Parent" => Object::Reference(pages2)
        });
        doc2.objects.insert(
            pages2,
            Object::Dictionary(dictionary! {
                "Type" => "Pages", "Kids" => vec![Object::Reference(p_plain)], "Count" => 1
            }),
        );
        assert_eq!(page_rotation(&doc2, p_plain), 0);
    }
}
