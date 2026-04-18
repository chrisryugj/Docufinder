//! 인덱싱 파이프라인
//!
//! 파일 파싱 → 청크 생성 → FTS5 인덱싱 → 벡터 인덱싱
//! rayon을 활용한 병렬 파싱 지원

pub use super::collector::*;
pub use super::sync::*;

use crate::constants::{METADATA_EXCLUDED_EXTENSIONS, OCR_IMAGE_EXTENSIONS, SUPPORTED_EXTENSIONS};
use crate::db;
use crate::indexer::exclusions::is_excluded_dir;
use crate::ocr::OcrEngine;
use crate::parsers::{parse_file, ParsedDocument};
use crate::tokenizer::{LinderaKoTokenizer, TextTokenizer};

use crossbeam_channel::{bounded, RecvTimeoutError};
use once_cell::sync::Lazy;
use rayon::prelude::*;
use rusqlite::Connection;
use std::fs;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use super::collector::{collect_files, save_file_metadata_only};

/// FTS 인덱싱 시 형태소 토큰 생성용 글로벌 토크나이저 (lazy init)
pub(crate) static FTS_TOKENIZER: Lazy<Option<LinderaKoTokenizer>> =
    Lazy::new(|| match LinderaKoTokenizer::new() {
        Ok(t) => {
            tracing::info!("FTS 형태소 분석기 초기화 완료");
            Some(t)
        }
        Err(e) => {
            tracing::warn!("FTS 형태소 분석기 초기화 실패 (형태소 없이 인덱싱): {}", e);
            None
        }
    });

/// 스트리밍 파이프라인 채널 버퍼 크기
/// 32: 8GB RAM PC에서 대용량 문서(XLSX/PDF) 동시 버퍼링 시 메모리 피크 억제
pub(crate) const CHANNEL_BUFFER_SIZE: usize = 32;

/// FTS 배치 트랜잭션 크기 - fsync 오버헤드 감소 (3~5배 성능 향상)
pub(crate) const TRANSACTION_BATCH_SIZE: usize = 200;

/// 에러 벡터 최대 엔트리 수 (메모리 bloat 방지)
pub(crate) const MAX_INDEXING_ERRORS: usize = 200;

/// `\\?\` prefix 제거 + display()로 깔끔한 경로 출력
fn clean_path_display(path: &Path) -> String {
    clean_path_str(&path.to_string_lossy())
}

/// 문자열 경로에서 `\\?\` prefix 제거
fn clean_path_str(path: &str) -> String {
    path.strip_prefix(r"\\?\").unwrap_or(path).to_string()
}

/// 파싱 결과 (스트리밍 파이프라인용)
pub(crate) enum ParseResult {
    Success {
        path: PathBuf,
        document: ParsedDocument,
    },
    Failure {
        path: PathBuf,
        error: String,
    },
}

// ==================== 2단계 인덱싱: FTS 전용 ====================

/// FTS 인덱싱 진행률 정보
#[derive(Debug, Clone, serde::Serialize)]
pub struct FtsIndexingProgress {
    pub phase: String,
    pub total_files: usize,
    pub processed_files: usize,
    pub current_file: Option<String>,
    pub folder_path: String,
}

/// FTS 진행률 콜백 타입
pub type FtsProgressCallback = Box<dyn Fn(FtsIndexingProgress) + Send + Sync>;

/// 폴더 인덱싱 - FTS만 (1단계, 벡터 제외)
/// skip_indexed: true이면 이미 fts_indexed_at이 있는 파일은 건너뜀 (resume 용)
#[allow(clippy::too_many_arguments)]
pub fn index_folder_fts_only(
    conn: &Connection,
    folder_path: &Path,
    recursive: bool,
    cancel_flag: Arc<AtomicBool>,
    progress_callback: Option<FtsProgressCallback>,
    max_file_size_mb: u64,
    excluded_dirs: &[String],
    ocr_engine: Option<Arc<OcrEngine>>,
) -> Result<FolderIndexResult, IndexError> {
    index_folder_fts_impl(
        conn,
        folder_path,
        recursive,
        cancel_flag,
        progress_callback,
        max_file_size_mb,
        false,
        excluded_dirs,
        ocr_engine,
    )
}

/// 폴더 인덱싱 재개 - 이미 인덱싱된 파일 스킵
#[allow(clippy::too_many_arguments)]
pub fn resume_folder_fts(
    conn: &Connection,
    folder_path: &Path,
    recursive: bool,
    cancel_flag: Arc<AtomicBool>,
    progress_callback: Option<FtsProgressCallback>,
    max_file_size_mb: u64,
    excluded_dirs: &[String],
    ocr_engine: Option<Arc<OcrEngine>>,
) -> Result<FolderIndexResult, IndexError> {
    index_folder_fts_impl(
        conn,
        folder_path,
        recursive,
        cancel_flag,
        progress_callback,
        max_file_size_mb,
        true,
        excluded_dirs,
        ocr_engine,
    )
}

#[allow(clippy::too_many_arguments)]
fn index_folder_fts_impl(
    conn: &Connection,
    folder_path: &Path,
    recursive: bool,
    cancel_flag: Arc<AtomicBool>,
    progress_callback: Option<FtsProgressCallback>,
    max_file_size_mb: u64,
    skip_indexed: bool,
    excluded_dirs: &[String],
    ocr_engine: Option<Arc<OcrEngine>>,
) -> Result<FolderIndexResult, IndexError> {
    use crate::utils::disk_info::{detect_disk_type, DiskSettings};

    let folder_str = folder_path.to_string_lossy().to_string();

    // 실제 디스크 타입에 맞춘 스레드 수 조정 (HDD: 2, SSD: 4)
    let disk_type = detect_disk_type(folder_path);
    let disk_settings = DiskSettings::for_disk_type(disk_type);
    tracing::info!(
        "[FTS] Disk: {:?}, threads: {}, throttle: {}ms",
        disk_type,
        disk_settings.parallel_threads,
        disk_settings.throttle_ms
    );

    // ⚡ 진행률 throttling (100ms 또는 10파일마다) - UI 렌더링 부하 감소
    use std::cell::Cell;
    let last_progress_time = Cell::new(std::time::Instant::now());
    let last_progress_count = Cell::new(0usize);
    const PROGRESS_THROTTLE_MS: u64 = 100;
    const PROGRESS_THROTTLE_FILES: usize = 10;

    let send_progress = |phase: &str,
                         total: usize,
                         processed: usize,
                         current: Option<&str>,
                         force: bool| {
        if let Some(ref cb) = progress_callback {
            // throttle: 100ms 또는 10파일마다, 또는 force=true일 때만 전송
            let now = std::time::Instant::now();
            let elapsed = now.duration_since(last_progress_time.get()).as_millis() as u64;
            let files_since = processed.saturating_sub(last_progress_count.get());

            if force || elapsed >= PROGRESS_THROTTLE_MS || files_since >= PROGRESS_THROTTLE_FILES {
                cb(FtsIndexingProgress {
                    phase: phase.to_string(),
                    total_files: total,
                    processed_files: processed,
                    current_file: current.map(|s| s.to_string()),
                    folder_path: folder_str.clone(),
                });
                last_progress_time.set(now);
                last_progress_count.set(processed);
            }
        }
    };

    // 1. 파일 스캔 (메타데이터 스캔에서 이미 수집한 경우 재사용하여 이중 FS 순회 방지)
    send_progress("scanning", 0, 0, None, true); // force: 시작
    let max_file_size_bytes = if max_file_size_mb > 0 {
        max_file_size_mb * 1_048_576
    } else {
        0
    };
    let all_files = collect_files(folder_path, recursive, cancel_flag.as_ref(), excluded_dirs);

    // 파싱 가능 파일 / 메타데이터 전용 파일 분리
    let has_ocr = ocr_engine.is_some();
    let (mut file_paths, metadata_only): (Vec<_>, Vec<_>) = all_files.into_iter().partition(|p| {
        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        let is_supported = SUPPORTED_EXTENSIONS.contains(&ext.as_str());
        let is_ocr_image = has_ocr && OCR_IMAGE_EXTENSIONS.contains(&ext.as_str());
        if !is_supported && !is_ocr_image {
            return false;
        }
        // 파싱 대상만 크기 제한 적용
        if max_file_size_bytes > 0 {
            if let Ok(meta) = p.metadata() {
                if meta.len() > max_file_size_bytes {
                    tracing::debug!(
                        "Skipping large file ({} MB): {:?}",
                        meta.len() / 1_048_576,
                        p
                    );
                    return false;
                }
            }
        }
        true
    });

    // 메타데이터 전용 파일 배치 저장 (파일명 검색용, 콘텐츠 파싱 없음)
    // 필터: 문서 확장자만 저장 (DLL, INI, 압축파일 등은 제외)
    let metadata_docs: Vec<_> = metadata_only
        .iter()
        .filter(|p| {
            let ext = p
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            // hwp, md, txt 등 문서류만 허용
            matches!(ext.as_str(), "txt" | "md" | "hwp" | "pdf")
        })
        .collect();

    if !metadata_docs.is_empty() {
        tracing::info!(
            "[FTS] Storing metadata for {} document files",
            metadata_docs.len()
        );
        let _ = conn.execute_batch("BEGIN");
        for (i, path) in metadata_docs.iter().enumerate() {
            if cancel_flag.load(Ordering::Acquire) {
                break;
            }
            let _ = save_file_metadata_only(conn, path);
            if (i + 1) % TRANSACTION_BATCH_SIZE == 0 {
                if let Err(e) = conn.execute_batch("COMMIT; BEGIN") {
                    tracing::warn!("Metadata batch commit failed: {}", e);
                    if conn.is_autocommit() {
                        let _ = conn.execute_batch("BEGIN");
                    }
                }
            }
        }
        let _ = conn.execute_batch("COMMIT");
    }

    // skip_indexed: 이미 인덱싱된 파일 제외 (resume 용)
    if skip_indexed {
        let already_indexed =
            crate::db::get_fts_indexed_paths_in_folder(conn, &folder_str).unwrap_or_default();
        if !already_indexed.is_empty() {
            let before = file_paths.len();
            file_paths.retain(|p| {
                let path_str = p.to_string_lossy().to_string();
                !already_indexed.contains(&path_str)
            });
            let skipped = before - file_paths.len();
            tracing::info!("[FTS Resume] Skipping {} already-indexed files", skipped);
        }
    }

    let total = file_paths.len();

    tracing::info!("[FTS] Found {} files to index in {:?}", total, folder_path);
    send_progress("scanning", total, 0, None, true); // force: 스캔 완료

    if cancel_flag.load(Ordering::Acquire) {
        send_progress("cancelled", total, 0, None, true); // force: 취소
        return Ok(FolderIndexResult {
            folder_path: folder_str,
            indexed_count: 0,
            failed_count: 0,
            vectors_count: 0,
            errors: vec![],
            was_cancelled: true,
            ocr_image_count: 0,
        });
    }

    // 2. 스트리밍 파이프라인 (디스크 유형 기반 병렬화)
    let (sender, receiver) = bounded::<ParseResult>(CHANNEL_BUFFER_SIZE);
    let cancel_flag_producer = cancel_flag.clone();
    let parallel_threads = disk_settings.parallel_threads;
    let throttle_ms = disk_settings.throttle_ms;

    let producer_handle = std::thread::spawn(move || {
        // 커스텀 ThreadPool (디스크 유형에 따른 스레드 수)
        let pool = match rayon::ThreadPoolBuilder::new()
            .num_threads(parallel_threads)
            .build()
            .or_else(|_| rayon::ThreadPoolBuilder::new().num_threads(2).build())
        {
            Ok(pool) => pool,
            Err(e) => {
                tracing::error!("Failed to create thread pool: {}", e);
                let _ = sender.send(ParseResult::Failure {
                    path: file_paths.first().cloned().unwrap_or_default(),
                    error: format!("Thread pool creation failed: {}", e),
                });
                return;
            }
        };

        // OCR 엔진 참조 (Arc clone은 move 전에 완료)
        let ocr_ref = ocr_engine.as_ref();

        pool.install(|| {
            let _ = file_paths.par_iter().try_for_each(|path| {
                if cancel_flag_producer.load(Ordering::Acquire) {
                    return Err(());
                }

                let path_clone = path.clone();
                let ocr_deref = ocr_ref.map(|e| e.as_ref());
                let result =
                    match catch_unwind(AssertUnwindSafe(|| parse_file(&path_clone, ocr_deref))) {
                        Ok(Ok(doc)) => ParseResult::Success {
                            path: path.clone(),
                            document: doc,
                        },
                        Ok(Err(e)) => ParseResult::Failure {
                            path: path.clone(),
                            error: e.to_string(),
                        },
                        Err(_) => ParseResult::Failure {
                            path: path.clone(),
                            error: "Parser panicked".to_string(),
                        },
                    };

                // HDD throttle: I/O 부하 감소
                if throttle_ms > 0 {
                    std::thread::sleep(std::time::Duration::from_millis(throttle_ms));
                }

                sender.send(result).map_err(|_| ())
            });
        });
    });

    // 3. Consumer: FTS만 저장 (벡터 제외) - 배치 트랜잭션 적용
    let mut indexed = 0;
    let mut failed = 0;
    let mut errors: Vec<String> = Vec::new();
    let mut suppressed_errors: usize = 0;
    let mut ocr_image_count: usize = 0;
    let mut processed = 0;
    let mut was_cancelled = false;
    let mut batch_count = 0;

    let recv_timeout = Duration::from_millis(100);

    // 배치 트랜잭션 시작
    if let Err(e) = conn.execute_batch("BEGIN") {
        return Err(IndexError::DbError(format!(
            "Failed to begin transaction: {}",
            e
        )));
    }

    {
        loop {
            if cancel_flag.load(Ordering::Acquire) {
                // 취소 시 현재까지 커밋
                let _ = conn.execute_batch("COMMIT");
                send_progress("cancelled", total, processed, None, true); // force: 취소
                was_cancelled = true;
                break;
            }

            match receiver.recv_timeout(recv_timeout) {
                Ok(result) => {
                    processed += 1;
                    batch_count += 1;

                    match result {
                        ParseResult::Success { path, document } => {
                            let file_name = path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("unknown");
                            send_progress("indexing", total, processed, Some(file_name), false); // throttled

                            match save_document_to_db_fts_only_no_tx(
                                conn,
                                &path,
                                document,
                                FTS_TOKENIZER.as_ref().map(|t| t as &dyn TextTokenizer),
                            ) {
                                Ok(_) => {
                                    indexed += 1;
                                    // OCR 이미지 파일 카운트
                                    let ext = path
                                        .extension()
                                        .and_then(|e| e.to_str())
                                        .unwrap_or("")
                                        .to_lowercase();
                                    if OCR_IMAGE_EXTENSIONS.contains(&ext.as_str()) {
                                        ocr_image_count += 1;
                                    }
                                }
                                Err(e) => {
                                    failed += 1;
                                    if errors.len() < MAX_INDEXING_ERRORS {
                                        errors.push(format!(
                                            "{}\t{}",
                                            clean_path_display(&path),
                                            e
                                        ));
                                    } else {
                                        suppressed_errors += 1;
                                    }
                                }
                            }
                        }
                        ParseResult::Failure { path, error } => {
                            if let Err(e) = save_file_metadata_only(conn, &path) {
                                tracing::warn!("Failed to save metadata for {:?}: {}", path, e);
                            }
                            failed += 1;
                            if errors.len() < MAX_INDEXING_ERRORS {
                                errors.push(format!("{}\t{}", clean_path_display(&path), error));
                            } else {
                                suppressed_errors += 1;
                            }
                            send_progress("indexing", total, processed, None, false);
                            // throttled
                        }
                    }

                    // 배치 크기마다 커밋 후 새 트랜잭션 시작
                    if batch_count >= TRANSACTION_BATCH_SIZE {
                        if let Err(e) = conn.execute_batch("COMMIT; BEGIN") {
                            tracing::warn!("Batch commit failed: {}", e);
                            // 트랜잭션 상태 복구: autocommit이면 BEGIN 재시도
                            if conn.is_autocommit() {
                                let _ = conn.execute_batch("BEGIN");
                            }
                        }
                        batch_count = 0;
                    }
                }
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
    }

    // 최종 커밋
    if !was_cancelled {
        if let Err(e) = conn.execute_batch("COMMIT") {
            tracing::warn!("Final commit failed: {}", e);
        }
    }

    // 항상 producer 스레드 join (취소 시에도 channel drop으로 빠르게 종료됨)
    // receiver는 이미 drop되었으므로 producer의 sender.send()가 Err 반환 → 루프 종료
    let _ = producer_handle.join();

    let phase = if was_cancelled {
        "cancelled"
    } else {
        "completed"
    };
    send_progress(phase, total, processed, None, true); // force: 완료

    if suppressed_errors > 0 {
        errors.push(format!("... 외 {}건 에러 생략", suppressed_errors));
    }

    Ok(FolderIndexResult {
        folder_path: folder_str,
        indexed_count: indexed,
        failed_count: failed,
        vectors_count: 0, // FTS만이므로 0
        errors,
        was_cancelled,
        ocr_image_count,
    })
}

/// 문서를 DB에 저장 - FTS만 (트랜잭션 없음, 배치용)
///
/// `tokenizer`: 형태소 분석기가 있으면 FTS에 형태소 토큰도 함께 인덱싱.
/// unicode61 토크나이저의 한국어 토큰화 한계를 보완하여 검색 재현율 향상.
pub(crate) fn save_document_to_db_fts_only_no_tx(
    conn: &Connection,
    path: &Path,
    document: ParsedDocument,
    tokenizer: Option<&dyn crate::tokenizer::TextTokenizer>,
) -> Result<usize, IndexError> {
    let path_str = path.to_string_lossy().to_string();

    let metadata = fs::metadata(path).map_err(|e| IndexError::IoError(e.to_string()))?;
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();
    let file_type = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let size = i64::try_from(metadata.len()).unwrap_or(i64::MAX);
    let modified_at = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    // upsert_file_fts_only 사용 (vector_indexed_at = NULL)
    let file_id =
        db::upsert_file_fts_only(conn, &path_str, &file_name, &file_type, size, modified_at)
            .map_err(|e| IndexError::DbError(e.to_string()))?;

    // Lineage 부여 — 같은 stem/폴더의 기존 canonical과 점수 비교 후 승자 지정
    if let Err(e) = crate::indexer::lineage::assign_for_file(
        conn, file_id, &path_str, &file_name, Some(modified_at),
    ) {
        tracing::warn!("lineage assign failed for {}: {}", path_str, e);
    }

    // _no_tx 버전 사용: 호출자(index_folder_fts_only)가 이미 트랜잭션을 관리하므로
    // 중첩 BEGIN 방지 (SQLite는 중첩 트랜잭션 미지원)
    db::delete_chunks_for_file_no_tx(conn, file_id)
        .map_err(|e| IndexError::DbError(e.to_string()))?;

    let chunks_count = document.chunks.len();

    for (idx, chunk) in document.chunks.into_iter().enumerate() {
        // 형태소 분석기가 있으면 FTS에 형태소 토큰도 함께 저장
        let extra_tokens = tokenizer.map(|tok| {
            let morphemes = tok.tokenize(&chunk.content);
            morphemes.join(" ")
        });

        db::insert_chunk(
            conn,
            file_id,
            idx,
            &chunk.content,
            chunk.start_offset,
            chunk.end_offset,
            chunk.page_number,
            chunk.page_end,
            chunk.location_hint.as_deref(),
            extra_tokens.as_deref(),
        )
        .map_err(|e| IndexError::DbError(e.to_string()))?;
    }

    tracing::debug!("[FTS] Indexed: {} ({} chunks)", path_str, chunks_count);

    Ok(chunks_count)
}

// ==================== Phase 2: 메타데이터 전용 스캔 ====================

/// 메타데이터 스캔 진행률
#[derive(Debug, Clone, serde::Serialize)]
pub struct MetadataScanProgress {
    pub phase: String,
    pub scanned_files: usize,
    pub folder_path: String,
}

/// 메타데이터 스캔 결과
#[derive(Debug, Clone)]
pub struct MetadataScanResult {
    pub folder_path: String,
    pub files_found: usize,
    pub errors: Vec<String>,
    pub was_cancelled: bool,
}

/// 메타데이터 전용 스캔 (파일 열지 않음, < 2초 목표)
/// 파일명 검색 즉시 가능하게 함
pub fn scan_metadata_only(
    conn: &Connection,
    folder_path: &Path,
    recursive: bool,
    cancel_flag: Arc<AtomicBool>,
    progress_callback: Option<Box<dyn Fn(MetadataScanProgress) + Send + Sync>>,
    _max_file_size_mb: u64,
    excluded_dirs: &[String],
) -> Result<MetadataScanResult, IndexError> {
    let folder_str = folder_path.to_string_lossy().to_string();

    // 진행률 throttling
    use std::cell::Cell;
    let last_progress_time = Cell::new(std::time::Instant::now());
    let last_progress_count = Cell::new(0usize);
    const PROGRESS_THROTTLE_MS: u64 = 100;
    const PROGRESS_THROTTLE_FILES: usize = 100; // 메타 스캔은 빠르므로 100개 단위

    let send_progress = |phase: &str, count: usize, force: bool| {
        if let Some(ref cb) = progress_callback {
            let now = std::time::Instant::now();
            let elapsed = now.duration_since(last_progress_time.get()).as_millis() as u64;
            let files_since = count.saturating_sub(last_progress_count.get());

            if force || elapsed >= PROGRESS_THROTTLE_MS || files_since >= PROGRESS_THROTTLE_FILES {
                cb(MetadataScanProgress {
                    phase: phase.to_string(),
                    scanned_files: count,
                    folder_path: folder_str.clone(),
                });
                last_progress_time.set(now);
                last_progress_count.set(count);
            }
        }
    };

    send_progress("scanning", 0, true);

    let mut count = 0;
    let mut errors: Vec<String> = Vec::new();
    let mut suppressed_errors: usize = 0;

    // 배치 트랜잭션 (성능 최적화)
    conn.execute_batch("BEGIN")
        .map_err(|e| IndexError::DbError(e.to_string()))?;

    let mut batch_count = 0;
    const BATCH_SIZE: usize = 100;

    // WalkDir 직접 순회 (collect_files보다 메모리 효율적)
    let walker = if recursive {
        walkdir::WalkDir::new(folder_path)
    } else {
        walkdir::WalkDir::new(folder_path).max_depth(1)
    };

    // filter_entry로 제외 디렉토리 하위 전체를 건너뛰기
    for entry in walker
        .into_iter()
        .filter_entry(|e| {
            if e.file_type().is_dir() {
                let name = e.file_name().to_str().unwrap_or("");
                // 숨김 폴더 제외
                if name.starts_with('.') {
                    return false;
                }
                // 제외 디렉토리 목록 체크
                if is_excluded_dir(e.path(), excluded_dirs) {
                    return false;
                }
            }
            true
        })
        .filter_map(|e| e.ok())
    {
        if cancel_flag.load(Ordering::Acquire) {
            let _ = conn.execute_batch("COMMIT");
            send_progress("cancelled", count, true);
            return Ok(MetadataScanResult {
                folder_path: folder_str,
                files_found: count,
                errors: vec![],
                was_cancelled: true,
            });
        }

        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();

        // 임시 파일 제외
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if file_name.starts_with("~$") || file_name.starts_with('.') {
            continue;
        }

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        // 파일 크기 체크 (metadata 접근 - 파일 열지 않음)
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(e) => {
                if errors.len() < MAX_INDEXING_ERRORS {
                    errors.push(format!("{}\t{}", clean_path_display(path), e));
                } else {
                    suppressed_errors += 1;
                }
                continue;
            }
        };

        // 시스템 바이너리/임시 파일은 메타데이터 저장 제외
        // (DLL/EXE/SYS 수십만 개로 인한 DB 급팽창 + 검색 노이즈 방지)
        if METADATA_EXCLUDED_EXTENSIONS.contains(&ext.as_str()) {
            continue;
        }

        // DB에 메타데이터만 저장 (문서/데이터 파일 — 파일명 검색용)
        let path_str = path.to_string_lossy().to_string();
        let file_type = ext.clone();
        let size = i64::try_from(metadata.len()).unwrap_or(i64::MAX);
        let modified_at = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        if let Err(e) =
            db::insert_file_metadata_only(conn, &path_str, file_name, &file_type, size, modified_at)
        {
            if errors.len() < MAX_INDEXING_ERRORS {
                errors.push(format!("{}\t{}", clean_path_str(&path_str), e));
            } else {
                suppressed_errors += 1;
            }
            continue;
        }

        count += 1;
        batch_count += 1;
        send_progress("scanning", count, false);

        // 배치 커밋
        if batch_count >= BATCH_SIZE {
            if let Err(e) = conn.execute_batch("COMMIT; BEGIN") {
                tracing::warn!("Batch commit failed: {}", e);
                if conn.is_autocommit() {
                    let _ = conn.execute_batch("BEGIN");
                }
            }
            batch_count = 0;
        }
    }

    // 최종 커밋
    if let Err(e) = conn.execute_batch("COMMIT") {
        tracing::warn!("Final commit failed: {}", e);
    }

    send_progress("completed", count, true);
    tracing::info!("[MetadataScan] {} files found in {:?}", count, folder_path);

    if suppressed_errors > 0 {
        errors.push(format!("... 외 {}건 에러 생략", suppressed_errors));
    }

    Ok(MetadataScanResult {
        folder_path: folder_str,
        files_found: count,
        errors,
        was_cancelled: false,
    })
}

// ==================== 단일 파일 FTS 인덱싱 (manager용) ====================

/// 단일 파일 FTS 인덱싱 (트랜잭션 없음) - WatchManager 배치 처리용
///
/// 호출자가 BEGIN/COMMIT을 관리해야 함.
pub(crate) fn index_file_fts_only_no_tx(
    conn: &Connection,
    path: &Path,
    ocr_engine: Option<&OcrEngine>,
) -> Result<IndexResult, IndexError> {
    let document =
        parse_file(path, ocr_engine).map_err(|e| IndexError::ParseError(e.to_string()))?;
    let total_chars = document.content.len();

    let chunks_count = save_document_to_db_fts_only_no_tx(
        conn,
        path,
        document,
        FTS_TOKENIZER.as_ref().map(|t| t as &dyn TextTokenizer),
    )?;

    Ok(IndexResult {
        file_path: path.to_string_lossy().to_string(),
        chunks_count,
        vectors_count: 0,
        total_chars,
    })
}

/// 단일 파일 FTS 인덱싱 (벡터 제외) - 트랜잭션 포함 독립 버전
#[allow(dead_code)]
pub fn index_file_fts_only(
    conn: &Connection,
    path: &Path,
    ocr_engine: Option<&OcrEngine>,
) -> Result<IndexResult, IndexError> {
    let document =
        parse_file(path, ocr_engine).map_err(|e| IndexError::ParseError(e.to_string()))?;
    let total_chars = document.content.len();

    conn.execute_batch("BEGIN")
        .map_err(|e| IndexError::DbError(e.to_string()))?;

    let chunks_count = match save_document_to_db_fts_only_no_tx(
        conn,
        path,
        document,
        FTS_TOKENIZER.as_ref().map(|t| t as &dyn TextTokenizer),
    ) {
        Ok(c) => c,
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            return Err(e);
        }
    };

    if let Err(e) = conn.execute_batch("COMMIT") {
        let _ = conn.execute_batch("ROLLBACK");
        return Err(IndexError::DbError(e.to_string()));
    }

    Ok(IndexResult {
        file_path: path.to_string_lossy().to_string(),
        chunks_count,
        vectors_count: 0,
        total_chars,
    })
}

#[derive(Debug)]
#[allow(dead_code)] // 인덱싱 결과 메타데이터 (일부 필드만 현재 사용)
pub struct IndexResult {
    pub file_path: String,
    pub chunks_count: usize,
    pub vectors_count: usize,
    pub total_chars: usize,
}

#[derive(Debug)]
pub struct FolderIndexResult {
    pub folder_path: String,
    pub indexed_count: usize,
    pub failed_count: usize,
    pub vectors_count: usize,
    pub errors: Vec<String>,
    /// 사용자에 의해 취소되었는지 여부
    pub was_cancelled: bool,
    /// OCR로 인덱싱된 이미지 파일 수
    pub ocr_image_count: usize,
}

#[derive(Debug, thiserror::Error)]
#[allow(dead_code, clippy::enum_variant_names)]
pub enum IndexError {
    #[error("IO error: {0}")]
    IoError(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Database error: {0}")]
    DbError(String),
    #[error("Embedding error: {0}")]
    EmbeddingError(String),
    #[error("Vector error: {0}")]
    VectorError(String),
}
