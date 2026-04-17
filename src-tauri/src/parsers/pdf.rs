use super::{DocumentChunk, DocumentMetadata, ParseError, ParsedDocument};
use crate::ocr::OcrEngine;
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

    let handle = std::thread::spawn(move || {
        let result = catch_unwind(AssertUnwindSafe(|| pdf_extract::extract_text(&path_owned)));
        let _ = tx.send(result);
    });

    // 동적 타임아웃 대기 (파일 크기 기반)
    let raw_text = match rx.recv_timeout(Duration::from_secs(timeout_secs)) {
        Ok(Ok(Ok(text))) => text,
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

    // 추출 텍스트 크기 제한 (50MB — 대용량 PDF OOM 방지)
    const MAX_EXTRACTED_TEXT_SIZE: usize = 50 * 1024 * 1024;
    let raw_text = if raw_text.len() > MAX_EXTRACTED_TEXT_SIZE {
        tracing::warn!(
            "PDF text truncated: {}MB → {}MB: {:?}",
            raw_text.len() / 1_048_576,
            MAX_EXTRACTED_TEXT_SIZE / 1_048_576,
            path
        );
        let mut truncated = raw_text;
        truncated.truncate(MAX_EXTRACTED_TEXT_SIZE);
        // char 경계 안전하게 자르기
        while !truncated.is_char_boundary(truncated.len()) {
            truncated.pop();
        }
        truncated
    } else {
        raw_text
    };

    // 페이지별 분리 (form feed 문자 \x0c 기준)
    let page_count = raw_text.matches('\x0c').count() + 1;

    // OCR 필요 여부 판정 + OCR 결과 (필요 시에만 cleaned_pages 전체 보유)
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
            // 스캔 페이지 존재 여부를 빠르게 확인 (clean 전 raw 텍스트로)
            let has_scanned = raw_text.split('\x0c').any(|p| {
                let trimmed = p.trim();
                trimmed.chars().count() < SCANNED_PAGE_CHAR_THRESHOLD
            });
            if has_scanned {
                // OCR 경로: cleaned_pages 전체 필요 (lopdf가 페이지 번호로 접근하므로)
                let cleaned: Vec<String> = raw_text.split('\x0c').map(clean_pdf_text).collect();
                ocr_scanned_pages(path, &cleaned, ocr_engine)
            } else {
                vec![]
            }
        }
    } else {
        vec![]
    };

    // 페이지별 스트리밍 처리: clean → chunk → all_text에 추가
    // raw_text를 split 이터레이터로 소비하여 메모리 피크 최소화
    let mut all_text = String::with_capacity(raw_text.len() / 2); // 정리 후 대략 50% 추정
    let mut chunks = Vec::new();
    let mut global_offset = 0;

    for (page_idx, raw_page) in raw_text.split('\x0c').enumerate() {
        // OCR 결과가 있으면 대체, 없으면 현재 페이지만 clean
        let page_text_owned;
        let page_text: &str = if let Some(Some(ocr_text)) = ocr_texts.get(page_idx) {
            ocr_text.as_str()
        } else {
            page_text_owned = clean_pdf_text(raw_page);
            &page_text_owned
        };

        if page_text.is_empty() {
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

    let mut ocr_count = 0usize;

    page_texts
        .iter()
        .enumerate()
        .map(|(page_idx, text)| {
            // 텍스트 충분한 페이지는 스킵
            if text.chars().count() >= SCANNED_PAGE_CHAR_THRESHOLD {
                return None;
            }

            // OCR 페이지 수 제한 (성능 보호)
            if ocr_count >= MAX_OCR_PAGES {
                if ocr_count == MAX_OCR_PAGES {
                    tracing::info!(
                        "PDF OCR page limit reached ({}), skipping remaining pages",
                        MAX_OCR_PAGES
                    );
                }
                return None;
            }
            ocr_count += 1;

            let page_num = (page_idx + 1) as u32;
            let page_id = match pages.get(&page_num) {
                Some(id) => *id,
                None => return None,
            };

            // 페이지에서 가장 큰 이미지 추출
            let image = match extract_page_image(&doc, page_id) {
                Some(img) => img,
                None => {
                    tracing::debug!("No extractable image in scanned page {}", page_num);
                    return None;
                }
            };

            // OCR 실행
            match ocr.recognize_image(&image) {
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
        })
        .collect()
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
