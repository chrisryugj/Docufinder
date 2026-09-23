//! 폴더 동기화 로직
//!
//! DB와 파일시스템 간 변경분 감지 및 증분 인덱싱

use crate::constants::{METADATA_EXCLUDED_EXTENSIONS, OCR_IMAGE_EXTENSIONS, SUPPORTED_EXTENSIONS};
use crate::db;
use crate::indexer::collector::save_file_metadata_only;
use crate::indexer::exclusions::is_excluded_dir;
use crate::indexer::pipeline::{
    save_document_to_db_fts_only_no_tx, FtsIndexingProgress, FtsProgressCallback, IndexError,
    ParseResult, SharedOcrEngine, CHANNEL_BUFFER_SIZE, FTS_TOKENIZER, MAX_INDEXING_ERRORS,
    TRANSACTION_BATCH_SIZE,
};
use crate::parsers::parse_file;
use crate::tokenizer::TextTokenizer;
use crate::utils::idle_detector;

use crossbeam_channel::{bounded, RecvTimeoutError};
use rayon::prelude::*;
use rusqlite::{params, Connection};
use std::fs;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

/// 폴더 동기화 결과
#[derive(Debug)]
pub struct SyncResult {
    pub folder_path: String,
    pub added: usize,
    pub modified: usize,
    pub deleted: usize,
    pub failed: usize,
    pub unchanged: usize,
    pub errors: Vec<String>,
    /// 사용자에 의해 취소되었는지 여부
    pub was_cancelled: bool,
}

/// 폴더 동기화 - 변경분만 인덱싱 (추가/수정/삭제 감지)
#[allow(clippy::too_many_arguments)]
pub fn sync_folder_fts(
    conn: &Connection,
    folder_path: &Path,
    recursive: bool,
    cancel_flag: Arc<AtomicBool>,
    progress_callback: Option<FtsProgressCallback>,
    max_file_size_mb: u64,
    excluded_dirs: &[String],
    ocr_engine: SharedOcrEngine,
    vector_index: Option<Arc<crate::search::vector::VectorIndex>>,
) -> Result<SyncResult, IndexError> {
    use crate::utils::disk_info::{detect_disk_type, DiskSettings};

    // 경로 표현 수렴 (이슈 #34): DB에 다른 표현(매핑드라이브 ↔ UNC)으로 저장된
    // rows를 canonical로 먼저 이관해야 아래 exact-비교 diff가 전 파일을 "신규"로
    // 오판(→ 전체 재파싱 + 중복 row)하지 않는다. 이후 걷기도 canonical로 수행.
    let folder_path = &crate::indexer::path_reconcile::reconcile_folder_representation(
        conn,
        folder_path,
        vector_index.as_deref(),
    );

    let folder_str = folder_path.to_string_lossy().to_string();

    let max_file_size_bytes = if max_file_size_mb > 0 {
        max_file_size_mb * 1_048_576
    } else {
        0
    };

    // 1. DB 임시 테이블로 파일시스템 스냅샷 적재
    //
    // 기존: db_files HashMap (~200MB) + fs_path_set HashSet (~200MB) = ~400MB
    // 개선: SQLite TEMP TABLE에 FS 경로 적재 → SQL JOIN으로 diff 계산 (~40-80MB)
    conn.execute_batch(
        "CREATE TEMP TABLE IF NOT EXISTS _sync_fs \
         (path TEXT PRIMARY KEY NOT NULL, modified_at INTEGER NOT NULL)",
    )
    .map_err(|e| IndexError::DbError(e.to_string()))?;
    conn.execute_batch("DELETE FROM _sync_fs")
        .map_err(|e| IndexError::DbError(e.to_string()))?;

    let walker = if recursive {
        walkdir::WalkDir::new(folder_path)
    } else {
        walkdir::WalkDir::new(folder_path).max_depth(1)
    };

    // 배치 INSERT (5_000개마다 COMMIT)
    conn.execute_batch("BEGIN")
        .map_err(|e| IndexError::DbError(e.to_string()))?;
    let mut insert_stmt = conn
        .prepare("INSERT OR REPLACE INTO _sync_fs (path, modified_at) VALUES (?1, ?2)")
        .map_err(|e| IndexError::DbError(e.to_string()))?;
    let mut insert_batch_count = 0usize;

    'walk: for entry in walker
        .into_iter()
        .filter_entry(|e| {
            if e.file_type().is_dir() {
                let name = e.file_name().to_str().unwrap_or("");
                if name.starts_with('.') {
                    return false;
                }
                if is_excluded_dir(e.path(), excluded_dirs) {
                    return false;
                }
            }
            true
        })
        .filter_map(|e| e.ok())
    {
        if cancel_flag.load(Ordering::Acquire) {
            drop(insert_stmt);
            let _ = conn.execute_batch("ROLLBACK");
            let _ = conn.execute_batch("DROP TABLE IF EXISTS _sync_fs");
            return Ok(SyncResult {
                folder_path: folder_str,
                added: 0,
                modified: 0,
                deleted: 0,
                failed: 0,
                unchanged: 0,
                errors: vec![],
                was_cancelled: true,
            });
        }

        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();

        // 임시 파일 / 숨김 파일 제외
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if file_name.starts_with("~$") || file_name.starts_with('.') {
            continue 'walk;
        }

        // 메타데이터 저장 제외 확장자 (DLL/EXE/SYS 등)
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if METADATA_EXCLUDED_EXTENSIONS.contains(&ext.as_str()) {
            continue 'walk;
        }

        let path_str = path.to_string_lossy();
        let modified_at = fs::metadata(path)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        if let Err(e) = insert_stmt.execute(params![path_str.as_ref(), modified_at]) {
            tracing::warn!("Failed to insert FS path into temp table: {}", e);
            continue 'walk;
        }
        insert_batch_count += 1;

        #[allow(clippy::manual_is_multiple_of)]
        if insert_batch_count % 5_000 == 0 {
            if let Err(e) = conn.execute_batch("COMMIT; BEGIN") {
                tracing::warn!("Sync temp table batch commit failed: {}", e);
                if conn.is_autocommit() {
                    let _ = conn.execute_batch("BEGIN");
                }
            }
        }
    }

    drop(insert_stmt);
    conn.execute_batch("COMMIT")
        .map_err(|e| IndexError::DbError(e.to_string()))?;

    // 2. SQL diff: 추가/수정 대상 파일
    //    LEFT JOIN files → DB에 없거나(is_new) modified_at 불일치(수정됨)
    let mut update_stmt = conn
        .prepare(
            "SELECT t.path FROM _sync_fs t \
             LEFT JOIN files f ON t.path = f.path \
             WHERE f.path IS NULL OR t.modified_at != f.modified_at",
        )
        .map_err(|e| IndexError::DbError(e.to_string()))?;
    let to_update: Vec<PathBuf> = update_stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| IndexError::DbError(e.to_string()))?
        .filter_map(|r| r.ok())
        .map(PathBuf::from)
        .collect();
    drop(update_stmt);

    // 3. SQL diff: 삭제된 파일 (DB에 있으나 FS 임시 테이블에 없음)
    let (pattern_unix, pattern_win) = folder_like_patterns(&folder_str);

    let mut delete_stmt = conn
        .prepare(
            "SELECT f.path FROM files f \
             WHERE (f.path LIKE ?1 ESCAPE '\\' OR f.path LIKE ?2 ESCAPE '\\') \
             AND NOT EXISTS (SELECT 1 FROM _sync_fs t WHERE t.path = f.path)",
        )
        .map_err(|e| IndexError::DbError(e.to_string()))?;
    let to_delete: Vec<String> = delete_stmt
        .query_map(params![pattern_unix, pattern_win], |row| {
            row.get::<_, String>(0)
        })
        .map_err(|e| IndexError::DbError(e.to_string()))?
        .filter_map(|r| r.ok())
        .collect();
    drop(delete_stmt);

    // 4. unchanged 카운트 (전체 FS - to_update)
    let total_fs_count = conn
        .query_row("SELECT COUNT(*) FROM _sync_fs", [], |row| {
            row.get::<_, usize>(0)
        })
        .unwrap_or(0);
    let unchanged = total_fs_count.saturating_sub(to_update.len());

    // 임시 테이블 정리
    let _ = conn.execute_batch("DROP TABLE IF EXISTS _sync_fs");

    // 파싱 가능 / 메타데이터 전용 분리
    let has_ocr = ocr_engine.is_some();
    let (to_index, to_metadata): (Vec<_>, Vec<_>) = to_update.into_iter().partition(|p| {
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
        if max_file_size_bytes > 0 {
            if let Ok(meta) = p.metadata() {
                if meta.len() > max_file_size_bytes {
                    return false;
                }
            }
        }
        true
    });

    let added_count = to_index.len();
    let metadata_count = to_metadata.len();
    let delete_count = to_delete.len();

    tracing::info!(
        "[Sync] {} - to_index: {}, metadata_only: {}, to_delete: {}, unchanged: {}",
        folder_str,
        added_count,
        metadata_count,
        delete_count,
        unchanged
    );

    // 4. 삭제 처리 — 스캔에 없다고 다 지우지 않는다. 폴더 스캔은 읽기 오류(권한 거부·잠깐 끊긴
    // 네트워크 공유)를 건너뛰어 그 아래 파일도 후보에 들어온다. 없다고 확인된 파일만 지운다
    // (앱 시작 정리 prune 과 같은 기준). 윈도우 경로 패턴 수리로 삭제 감지가 처음 실제로 돈다.
    let mut deleted = 0;
    for path in &to_delete {
        if cancel_flag.load(Ordering::Acquire) {
            break;
        }
        if !crate::utils::confirmed_missing(Path::new(path)) {
            continue;
        }
        // 벡터를 먼저 제거 — watcher 삭제 경로(manager.rs)와 동일. 안 하면 유령 벡터가
        // 남고, 해제된 chunks.id가 재사용될 때 타 파일 임베딩으로 오귀속된다(이슈 #34 후속).
        if let Some(vi) = vector_index.as_deref() {
            if let Ok(chunk_ids) = db::get_chunk_ids_for_path(conn, path) {
                for chunk_id in chunk_ids {
                    if let Err(e) = vi.remove(chunk_id) {
                        tracing::debug!("Failed to remove vector {}: {}", chunk_id, e);
                    }
                }
            }
        }
        if let Err(e) = db::delete_file(conn, path) {
            tracing::warn!("Failed to delete stale file {}: {}", path, e);
        } else {
            deleted += 1;
        }
    }

    // 4.5 메타데이터 전용 파일 저장
    if !to_metadata.is_empty() {
        let _ = conn.execute_batch("BEGIN");
        for (i, path) in to_metadata.iter().enumerate() {
            if cancel_flag.load(Ordering::Acquire) {
                break;
            }
            let _ = save_file_metadata_only(conn, path, vector_index.as_deref());
            if (i + 1) % TRANSACTION_BATCH_SIZE == 0 {
                if let Err(e) = conn.execute_batch("COMMIT; BEGIN") {
                    tracing::warn!("Sync metadata batch commit failed: {}", e);
                    if conn.is_autocommit() {
                        let _ = conn.execute_batch("BEGIN");
                    }
                }
            }
        }
        let _ = conn.execute_batch("COMMIT");
    }

    // 5. 인덱싱할 파일이 없으면 바로 완료 (progress 이벤트 없이 조용히)
    if to_index.is_empty() {
        return Ok(SyncResult {
            folder_path: folder_str,
            added: 0,
            modified: 0,
            deleted,
            failed: 0,
            unchanged,
            errors: vec![],
            was_cancelled: false,
        });
    }

    // 6. 변경된 파일만 인덱싱 (기존 파이프라인 재사용)
    let disk_type = detect_disk_type(folder_path);
    let disk_settings = DiskSettings::for_disk_type(disk_type);
    let total = to_index.len();

    // 진행률 throttling
    use std::cell::Cell;
    let last_progress_time = Cell::new(std::time::Instant::now());
    let last_progress_count = Cell::new(0usize);

    let send_progress =
        |phase: &str, total: usize, processed: usize, current: Option<&str>, force: bool| {
            if let Some(ref cb) = progress_callback {
                let now = std::time::Instant::now();
                let elapsed = now.duration_since(last_progress_time.get()).as_millis() as u64;
                let files_since = processed.saturating_sub(last_progress_count.get());
                if force || elapsed >= 100 || files_since >= 10 {
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

    send_progress("indexing", total, 0, None, true);

    // Producer: 파싱
    let (sender, receiver) = bounded::<ParseResult>(CHANNEL_BUFFER_SIZE);
    let cancel_flag_producer = cancel_flag.clone();
    let parallel_threads = disk_settings.parallel_threads;
    let throttle_ms = disk_settings.throttle_ms;

    let producer_handle = std::thread::spawn(move || {
        let pool = match rayon::ThreadPoolBuilder::new()
            .num_threads(parallel_threads)
            .build()
            .or_else(|_| rayon::ThreadPoolBuilder::new().num_threads(2).build())
        {
            Ok(pool) => pool,
            Err(e) => {
                tracing::error!("Failed to create thread pool for sync: {}", e);
                let _ = sender.send(ParseResult::Failure {
                    path: to_index.first().cloned().unwrap_or_default(),
                    error: format!("Thread pool creation failed: {}", e),
                });
                return;
            }
        };

        // 파일마다 `.get()` — 워밍업이 동기화 도중 끝나도 남은 파일부터 반영된다(이슈 #35).
        let ocr_cell = ocr_engine.as_deref();

        pool.install(|| {
            let _ = to_index.par_iter().try_for_each(|path| {
                if cancel_flag_producer.load(Ordering::Acquire) {
                    return Err(());
                }

                let path_clone = path.clone();
                let ocr_deref = ocr_cell.and_then(|c| c.get()).map(|e| e.as_ref());
                let result =
                    match catch_unwind(AssertUnwindSafe(|| parse_file(&path_clone, ocr_deref))) {
                        Ok(Ok(doc)) => {
                            // T3-3: 형태소 토큰 선계산 (배치 파이프라인과 동일)
                            let chunk_tokens = super::pipeline::tokenize_chunks_in_parse_pool(&doc);
                            ParseResult::Success {
                                path: path.clone(),
                                document: doc,
                                chunk_tokens,
                            }
                        }
                        Ok(Err(crate::parsers::ParseError::CloudPlaceholder(_))) => {
                            ParseResult::CloudSkipped { path: path.clone() }
                        }
                        Ok(Err(e)) => ParseResult::Failure {
                            path: path.clone(),
                            error: e.to_string(),
                        },
                        Err(_) => ParseResult::Failure {
                            path: path.clone(),
                            error: "Parser panicked".to_string(),
                        },
                    };

                if throttle_ms > 0 {
                    std::thread::sleep(std::time::Duration::from_millis(throttle_ms));
                }

                sender.send(result).map_err(|_| ())
            });
        });
    });

    // Consumer: DB 저장
    let mut indexed = 0;
    let mut failed = 0;
    let mut errors: Vec<String> = Vec::new();
    let mut suppressed_errors: usize = 0;
    let mut processed = 0;
    let recv_timeout = Duration::from_millis(100);

    if let Err(e) = conn.execute_batch("BEGIN") {
        return Err(IndexError::DbError(format!(
            "Failed to begin transaction: {}",
            e
        )));
    }

    let mut batch_count = 0;
    loop {
        if cancel_flag.load(Ordering::Acquire) {
            let _ = conn.execute_batch("COMMIT");
            break;
        }

        match receiver.recv_timeout(recv_timeout) {
            Ok(result) => {
                processed += 1;
                batch_count += 1;

                match result {
                    ParseResult::Success {
                        path,
                        document,
                        chunk_tokens,
                    } => {
                        let file_name = path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown");
                        send_progress("indexing", total, processed, Some(file_name), false);
                        match save_document_isolated(
                            conn,
                            &path,
                            document,
                            vector_index.as_deref(),
                            chunk_tokens,
                        ) {
                            Ok(()) => indexed += 1,
                            Err(e) => {
                                failed += 1;
                                if errors.len() < MAX_INDEXING_ERRORS {
                                    errors.push(format!("{:?}: {}", path, e));
                                } else {
                                    suppressed_errors += 1;
                                }
                            }
                        }
                    }
                    ParseResult::Failure { path, error } => {
                        if let Err(e) =
                            save_file_metadata_only(conn, &path, vector_index.as_deref())
                        {
                            tracing::warn!("Failed to save metadata for {:?}: {}", path, e);
                        }
                        failed += 1;
                        if errors.len() < MAX_INDEXING_ERRORS {
                            errors.push(format!("{:?}: {}", path, error));
                        } else {
                            suppressed_errors += 1;
                        }
                    }
                    ParseResult::CloudSkipped { path } => {
                        // 클라우드 placeholder: 메타데이터만 저장 (파일명 검색은 가능),
                        // 본문 다운로드는 회피.
                        if let Err(e) =
                            save_file_metadata_only(conn, &path, vector_index.as_deref())
                        {
                            tracing::warn!(
                                "Failed to save metadata for cloud placeholder {:?}: {}",
                                path,
                                e
                            );
                        }
                    }
                }

                if batch_count >= TRANSACTION_BATCH_SIZE {
                    if let Err(e) = conn.execute_batch("COMMIT; BEGIN") {
                        tracing::warn!("Batch commit failed: {}", e);
                        if conn.is_autocommit() {
                            let _ = conn.execute_batch("BEGIN");
                        }
                    }
                    batch_count = 0;

                    // 유휴 감지 기반 throttle: 사용자 활동 감지 시 배치당 50ms 대기
                    // (startup sync가 사용자 작업을 방해하지 않도록)
                    if !idle_detector::is_user_idle(2000) {
                        std::thread::sleep(Duration::from_millis(50));
                    }
                }
            }
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }

    if let Err(e) = conn.execute_batch("COMMIT") {
        tracing::warn!("Final commit failed: {}", e);
    }
    // receiver 를 먼저 버려 가득 찬 채널에 막힌 파서 스레드를 풀어 준다 (취소 시 join 영구 대기 방지)
    drop(receiver);
    let _ = producer_handle.join();

    send_progress("completed", total, processed, None, true);

    if suppressed_errors > 0 {
        errors.push(format!("... 외 {}건 에러 생략", suppressed_errors));
    }

    Ok(SyncResult {
        folder_path: folder_str,
        added: indexed,
        modified: 0, // added에 포함됨 (구분은 로그로)
        deleted,
        failed,
        unchanged,
        errors,
        was_cancelled: false,
    })
}

/// 문서 하나를 SAVEPOINT 안에서 저장한다. 실패나 패닉이면 그 문서의 부분 쓰기만 되돌리고 같은
/// 배치의 다른 문서는 살린다 (종전엔 저장 패닉이 동기화 스레드를 끝냈고, 실패한 문서의 반쯤 쓴
/// 행이 배치와 함께 커밋될 수 있었다). 호출자는 이미 BEGIN 한 트랜잭션 안에서 부른다.
pub(crate) fn save_document_isolated(
    conn: &Connection,
    path: &Path,
    document: crate::parsers::ParsedDocument,
    vector_index: Option<&crate::search::vector::VectorIndex>,
    chunk_tokens: Option<Vec<Option<String>>>,
) -> Result<(), String> {
    conn.execute_batch("SAVEPOINT doc_save")
        .map_err(|e| e.to_string())?;
    let result = catch_unwind(AssertUnwindSafe(|| {
        save_document_to_db_fts_only_no_tx(
            conn,
            path,
            document,
            FTS_TOKENIZER.as_ref().map(|t| t as &dyn TextTokenizer),
            vector_index,
            chunk_tokens,
        )
    }));
    let outcome = match result {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(e)) => Err(e.to_string()),
        Err(_) => Err("저장 중 내부 오류 (이 파일만 건너뜀)".to_string()),
    };
    let finish = if outcome.is_ok() {
        "RELEASE doc_save"
    } else {
        "ROLLBACK TO doc_save; RELEASE doc_save"
    };
    if let Err(e) = conn.execute_batch(finish) {
        tracing::warn!("SAVEPOINT 정리 실패 ({}): {}", path.display(), e);
    }
    outcome
}

/// 폴더 아래 경로를 고르는 LIKE 패턴 (유닉스 구분자, 윈도우 구분자). ESCAPE '\\' 와 함께 쓴다.
///
/// 윈도우 쪽 꼬리는 `\\\\%`(이스케이프된 역슬래시 + 와일드카드)여야 한다. `\\%` 로 쓰면 ESCAPE 아래서
/// 리터럴 `%` 가 되어 윈도우 경로가 하나도 안 걸렸다(삭제된 파일이 검색에 계속 남음).
/// 끝 구분자는 잘라 드라이브 루트(`C:\\`) 감시 폴더도 걸리게 한다 (db::delete_files_in_folder 와 동일).
fn folder_like_patterns(folder: &str) -> (String, String) {
    let folder = folder.trim_end_matches(['/', '\\']);
    let escaped_unix = db::escape_like_pattern(&folder.replace('\\', "/"));
    let escaped_win = db::escape_like_pattern(&folder.replace('/', "\\"));
    (
        format!("{}/%", escaped_unix),
        format!("{}\\\\%", escaped_win),
    )
}

#[cfg(test)]
mod like_pattern_tests {
    use super::folder_like_patterns;

    fn matches(pattern: &str, path: &str) -> bool {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.query_row("SELECT ?2 LIKE ?1 ESCAPE '\\'", [pattern, path], |r| {
            r.get(0)
        })
        .unwrap()
    }

    #[test]
    fn windows_paths_under_folder_match() {
        let (unix, win) = folder_like_patterns(r"C:\docs\보고서");
        assert!(matches(&win, r"C:\docs\보고서\a.hwpx"));
        assert!(matches(&win, r"C:\docs\보고서\sub\b.pdf"));
        assert!(
            !matches(&win, r"C:\docs\보고서-old\c.pdf"),
            "형제 폴더 오탐"
        );
        assert!(matches(&unix, "C:/docs/보고서/a.hwpx"));
    }

    #[test]
    fn drive_root_and_wildcard_chars() {
        let (_, win) = folder_like_patterns(r"D:\");
        assert!(matches(&win, r"D:\x.txt"));
        let (_, win) = folder_like_patterns(r"C:\100%_폴더");
        assert!(matches(&win, r"C:\100%_폴더\a.txt"));
        assert!(!matches(&win, r"C:\100x폴더\a.txt"), "% _ 는 리터럴이어야");
    }
}

#[cfg(test)]
mod save_isolation_tests {
    use super::save_document_isolated;
    use crate::parsers::{DocumentChunk, DocumentMetadata, ParsedDocument};
    use std::path::Path;

    fn doc(text: &str) -> ParsedDocument {
        ParsedDocument {
            content: text.to_string(),
            metadata: DocumentMetadata {
                title: None,
                author: None,
                created_at: None,
                page_count: None,
            },
            chunks: vec![DocumentChunk {
                content: text.to_string(),
                start_offset: 0,
                end_offset: text.len(),
                page_number: None,
                page_end: None,
                location_hint: None,
            }],
            garbled_hint: false,
        }
    }

    /// 본문을 못 읽어 메타데이터만 남길 때 예전 청크(와 FTS)는 지워져 옛 내용이 검색되지 않는다.
    #[test]
    fn metadata_fallback_clears_stale_chunks() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("stale.db");
        crate::db::init_database(&db_path).unwrap();
        let a = tmp.path().join("a.txt");
        std::fs::write(&a, "가").unwrap();

        let conn = crate::db::get_connection(&db_path).unwrap();
        conn.execute_batch("BEGIN").unwrap();
        save_document_isolated(&conn, &a, doc("비밀 예산 내역"), None, None).unwrap();
        conn.execute_batch("COMMIT").unwrap();
        let fts_hits = |q: &str| -> i64 {
            conn.query_row(
                "SELECT COUNT(*) FROM chunks_fts WHERE chunks_fts MATCH ?1",
                [q],
                |r| r.get(0),
            )
            .unwrap()
        };
        assert_eq!(fts_hits("비밀"), 1);

        crate::indexer::collector::save_file_metadata_only(&conn, &a, None).unwrap();
        assert_eq!(fts_hits("비밀"), 0, "옛 본문이 검색된다");
        let fts_at: Option<i64> = conn
            .query_row(
                "SELECT fts_indexed_at FROM files WHERE path = ?1",
                [a.to_string_lossy().to_string()],
                |r| r.get(0),
            )
            .unwrap();
        assert!(fts_at.is_none(), "다시 읽도록 색인 시각을 비워야 한다");
    }

    /// 메타데이터만 남길 때 예전 청크의 벡터도 지운다. 남기면 해제된 chunks.id 가 재사용될 때
    /// 벡터 워커가 "이미 있음" 으로 건너뛰어 다른 문서의 임베딩이 붙는다.
    #[test]
    fn metadata_fallback_removes_stale_vectors() {
        use crate::embedder::EMBEDDING_DIM;
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("vec.db");
        crate::db::init_database(&db_path).unwrap();
        let a = tmp.path().join("a.txt");
        std::fs::write(&a, "가").unwrap();
        let vi = crate::search::vector::VectorIndex::new(&tmp.path().join("v.usearch")).unwrap();

        let conn = crate::db::get_connection(&db_path).unwrap();
        conn.execute_batch("BEGIN").unwrap();
        save_document_isolated(&conn, &a, doc("예산 내역"), None, None).unwrap();
        conn.execute_batch("COMMIT").unwrap();
        let chunk_id: i64 = conn
            .query_row("SELECT id FROM chunks LIMIT 1", [], |r| r.get(0))
            .unwrap();
        let mut v = vec![0.0f32; EMBEDDING_DIM];
        v[0] = 1.0;
        vi.add(chunk_id, &v).unwrap();
        assert!(vi.contains_chunk(chunk_id));

        crate::indexer::collector::save_file_metadata_only(&conn, &a, Some(&vi)).unwrap();
        assert!(!vi.contains_chunk(chunk_id), "지운 청크의 벡터가 남았다");
    }

    /// 한 문서 저장이 중간에 실패해도 그 문서의 반쯤 쓴 행만 되돌리고, 같은 배치의 앞 문서는 남는다.
    #[test]
    fn failed_save_rolls_back_only_that_document() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("iso.db");
        crate::db::init_database(&db_path).unwrap();
        let a = tmp.path().join("a.txt");
        let b = tmp.path().join("b.txt");
        std::fs::write(&a, "가").unwrap();
        std::fs::write(&b, "나").unwrap();

        let conn = crate::db::get_connection(&db_path).unwrap();
        conn.execute_batch("BEGIN").unwrap();
        save_document_isolated(&conn, &a, doc("예산 집행"), None, None).unwrap();
        // 청크 테이블을 치워 b 저장을 files 행 쓴 뒤에 실패시킨다
        conn.execute_batch("ALTER TABLE chunks RENAME TO chunks_gone")
            .unwrap();
        assert!(save_document_isolated(&conn, &b, doc("계획 수립"), None, None).is_err());
        conn.execute_batch("ALTER TABLE chunks_gone RENAME TO chunks; COMMIT")
            .unwrap();

        let count = |p: &Path| -> i64 {
            conn.query_row(
                "SELECT COUNT(*) FROM files WHERE path = ?1",
                [p.to_string_lossy().to_string()],
                |r| r.get(0),
            )
            .unwrap()
        };
        assert_eq!(count(&a), 1, "앞 문서는 커밋돼야 한다");
        assert_eq!(
            count(&b),
            0,
            "실패한 문서의 반쯤 쓴 files 행은 되돌려져야 한다"
        );
    }
}

// 권한 비트로 폴더를 잠가 재현하는 테스트라 unix 전용 (윈도우에서는 헬퍼째 뺀다)
#[cfg(all(test, unix))]
mod sync_delete_tests {
    use super::sync_folder_fts;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    fn indexed_paths(conn: &rusqlite::Connection) -> Vec<String> {
        let mut stmt = conn
            .prepare("SELECT path FROM files ORDER BY path")
            .unwrap();
        let rows = stmt.query_map([], |r| r.get(0)).unwrap();
        rows.collect::<Result<_, _>>().unwrap()
    }

    fn sync(conn: &rusqlite::Connection, root: &std::path::Path) -> super::SyncResult {
        sync_folder_fts(
            conn,
            root,
            true,
            Arc::new(AtomicBool::new(false)),
            None,
            0,
            &[],
            None,
            None,
        )
        .unwrap()
    }

    /// 스캔 중 읽지 못한 하위 폴더(권한 거부 등)의 파일은 지우지 않고, 정말 지워진 파일만 지운다.
    #[test]
    fn unreadable_subfolder_is_not_treated_as_deleted() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("docs");
        let locked = root.join("locked");
        std::fs::create_dir_all(&locked).unwrap();
        std::fs::write(root.join("a.txt"), "예산 집행 계획").unwrap();
        std::fs::write(root.join("gone.txt"), "지울 문서").unwrap();
        std::fs::write(locked.join("b.txt"), "잠긴 폴더 문서").unwrap();

        let db_path = tmp.path().join("sync.db");
        crate::db::init_database(&db_path).unwrap();
        let conn = crate::db::get_connection(&db_path).unwrap();
        sync(&conn, &root);
        assert_eq!(indexed_paths(&conn).len(), 3);

        std::fs::remove_file(root.join("gone.txt")).unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
        let r = sync(&conn, &root);
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();

        let paths = indexed_paths(&conn);
        assert_eq!(r.deleted, 1, "정말 지운 파일 한 건만: {paths:?}");
        assert!(
            paths.iter().any(|p| p.ends_with("b.txt")),
            "읽지 못한 폴더의 문서가 지워졌다: {paths:?}"
        );
        assert!(!paths.iter().any(|p| p.ends_with("gone.txt")));
    }
}
