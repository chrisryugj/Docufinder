//! 백그라운드 컨텐츠 파서
//!
//! 메타데이터만 스캔된 파일들을 유휴 시간에 순차적으로 파싱.
//! HDD 환경에서 랜덤 I/O를 최소화하고, 사용자 활동 시 일시정지.
//!
//! NOTE: 현재 미사용 (향후 백그라운드 파싱 기능 통합 예정)

#![allow(dead_code)]

use crate::db;
use crate::parsers::parse_file;
use crate::utils::disk_info::detect_disk_type;
use crate::utils::idle_detector;

use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// 백그라운드 파싱 진행률
#[derive(Debug, Clone, serde::Serialize)]
pub struct BackgroundParsingProgress {
    pub phase: String,
    pub total_pending: usize,
    pub processed: usize,
    pub current_file: Option<String>,
    pub is_idle: bool,
}

/// 백그라운드 파서 설정
#[derive(Debug, Clone)]
pub struct BackgroundParserConfig {
    /// 유휴 판정 기준 (ms)
    pub idle_threshold_ms: u64,
    /// HDD 모드: 파일 간 대기 시간 (ms)
    pub hdd_throttle_ms: u64,
    /// 배치 크기 (한 번에 처리할 파일 수)
    pub batch_size: usize,
}

impl Default for BackgroundParserConfig {
    fn default() -> Self {
        Self {
            idle_threshold_ms: 3000,  // 3초 유휴
            hdd_throttle_ms: 50,      // HDD: 50ms 대기
            batch_size: 10,
        }
    }
}

/// 백그라운드 파서
pub struct BackgroundParser {
    config: BackgroundParserConfig,
    cancel_flag: Arc<AtomicBool>,
    worker_handle: Option<JoinHandle<()>>,
}

impl BackgroundParser {
    pub fn new(config: BackgroundParserConfig) -> Self {
        Self {
            config,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            worker_handle: None,
        }
    }

    /// 백그라운드 파싱 시작
    pub fn start(
        &mut self,
        db_path: PathBuf,
        folder_path: PathBuf,
        progress_callback: Option<Arc<dyn Fn(BackgroundParsingProgress) + Send + Sync>>,
    ) {
        if self.worker_handle.is_some() {
            tracing::warn!("BackgroundParser already running");
            return;
        }

        self.cancel_flag.store(false, Ordering::SeqCst);
        let cancel_flag = self.cancel_flag.clone();
        let config = self.config.clone();

        let handle = thread::spawn(move || {
            Self::worker_loop(db_path, folder_path, config, cancel_flag, progress_callback);
        });

        self.worker_handle = Some(handle);
        tracing::info!("BackgroundParser started");
    }

    /// 파싱 취소
    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
    }

    /// 실행 중인지 확인
    pub fn is_running(&self) -> bool {
        self.worker_handle.as_ref().is_some_and(|h| !h.is_finished())
    }

    /// 종료 대기
    pub fn join(&mut self) {
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
    }

    /// 워커 루프
    fn worker_loop(
        db_path: PathBuf,
        folder_path: PathBuf,
        config: BackgroundParserConfig,
        cancel_flag: Arc<AtomicBool>,
        progress_callback: Option<Arc<dyn Fn(BackgroundParsingProgress) + Send + Sync>>,
    ) {
        // 디스크 유형 감지
        let disk_type = detect_disk_type(&folder_path);
        let is_hdd = disk_type.is_hdd();
        tracing::info!(
            "BackgroundParser: folder={:?}, disk_type={:?}, is_hdd={}",
            folder_path, disk_type, is_hdd
        );

        let conn = match db::get_connection(&db_path) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("BackgroundParser: failed to get DB connection: {}", e);
                return;
            }
        };

        let send_progress = |phase: &str, total: usize, processed: usize, current: Option<&str>, is_idle: bool| {
            if let Some(ref cb) = progress_callback {
                cb(BackgroundParsingProgress {
                    phase: phase.to_string(),
                    total_pending: total,
                    processed,
                    current_file: current.map(String::from),
                    is_idle,
                });
            }
        };

        // 미처리 파일 수 조회
        let total_pending = match get_pending_files_count(&conn, &folder_path) {
            Ok(count) => count,
            Err(e) => {
                tracing::error!("BackgroundParser: failed to get pending count: {}", e);
                return;
            }
        };

        if total_pending == 0 {
            tracing::info!("BackgroundParser: no pending files");
            send_progress("completed", 0, 0, None, true);
            return;
        }

        tracing::info!("BackgroundParser: {} pending files", total_pending);
        send_progress("starting", total_pending, 0, None, false);

        let mut processed = 0;

        loop {
            if cancel_flag.load(Ordering::Relaxed) {
                tracing::info!("BackgroundParser: cancelled");
                send_progress("cancelled", total_pending, processed, None, false);
                break;
            }

            // 유휴 대기
            if !idle_detector::is_user_idle(config.idle_threshold_ms) {
                send_progress("waiting_idle", total_pending, processed, None, false);
                idle_detector::wait_for_idle_sync(config.idle_threshold_ms, 500);
                if cancel_flag.load(Ordering::Relaxed) {
                    break;
                }
            }

            // 미처리 파일 1개 가져오기
            let pending_file = match get_next_pending_file(&conn, &folder_path) {
                Ok(Some(f)) => f,
                Ok(None) => {
                    tracing::info!("BackgroundParser: all files processed");
                    send_progress("completed", total_pending, processed, None, true);
                    break;
                }
                Err(e) => {
                    tracing::error!("BackgroundParser: failed to get pending file: {}", e);
                    break;
                }
            };

            send_progress("parsing", total_pending, processed, Some(&pending_file.name), true);

            // 파일 파싱 + FTS 인덱싱
            let path = Path::new(&pending_file.path);
            match parse_and_index_file(&conn, path, pending_file.id) {
                Ok(chunks) => {
                    tracing::debug!(
                        "BackgroundParser: indexed {} ({} chunks)",
                        pending_file.name, chunks
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        "BackgroundParser: failed to parse {}: {}",
                        pending_file.path, e
                    );
                    // 실패해도 fts_indexed_at 마킹하여 재시도 방지
                    let _ = mark_file_fts_indexed(&conn, pending_file.id);
                }
            }

            processed += 1;

            // HDD 모드: throttle
            if is_hdd && config.hdd_throttle_ms > 0 {
                thread::sleep(Duration::from_millis(config.hdd_throttle_ms));
            }
        }

        tracing::info!("BackgroundParser: finished ({} files processed)", processed);
    }
}

/// 미처리 파일 정보
struct PendingFile {
    id: i64,
    path: String,
    name: String,
}

/// 미처리 파일 수 조회 (fts_indexed_at IS NULL)
fn get_pending_files_count(conn: &Connection, folder_path: &Path) -> Result<usize, rusqlite::Error> {
    let folder_str = folder_path.to_string_lossy();
    let pattern = format!("{}%", folder_str);

    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM files WHERE path LIKE ? AND fts_indexed_at IS NULL",
        rusqlite::params![pattern],
        |row| row.get(0),
    )?;

    Ok(count as usize)
}

/// 다음 미처리 파일 조회
fn get_next_pending_file(conn: &Connection, folder_path: &Path) -> Result<Option<PendingFile>, rusqlite::Error> {
    let folder_str = folder_path.to_string_lossy();
    let pattern = format!("{}%", folder_str);

    let mut stmt = conn.prepare(
        "SELECT id, path, name FROM files WHERE path LIKE ? AND fts_indexed_at IS NULL LIMIT 1"
    )?;

    let mut rows = stmt.query(rusqlite::params![pattern])?;

    if let Some(row) = rows.next()? {
        Ok(Some(PendingFile {
            id: row.get(0)?,
            path: row.get(1)?,
            name: row.get(2)?,
        }))
    } else {
        Ok(None)
    }
}

/// 파일 파싱 + FTS 인덱싱
fn parse_and_index_file(conn: &Connection, path: &Path, file_id: i64) -> Result<usize, String> {
    // 파싱
    let document = parse_file(path).map_err(|e| e.to_string())?;

    // 기존 청크 삭제
    db::delete_chunks_for_file(conn, file_id).map_err(|e| e.to_string())?;

    // 청크 저장 + FTS 인덱싱
    let chunks_count = document.chunks.len();
    for (idx, chunk) in document.chunks.iter().enumerate() {
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
        ).map_err(|e| e.to_string())?;
    }

    // fts_indexed_at 마킹
    mark_file_fts_indexed(conn, file_id).map_err(|e| e.to_string())?;

    Ok(chunks_count)
}

/// 파일 FTS 인덱싱 완료 마킹
fn mark_file_fts_indexed(conn: &Connection, file_id: i64) -> Result<(), rusqlite::Error> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    conn.execute(
        "UPDATE files SET fts_indexed_at = ?, indexed_at = ? WHERE id = ?",
        rusqlite::params![now, now, file_id],
    )?;

    Ok(())
}

impl Drop for BackgroundParser {
    fn drop(&mut self) {
        self.cancel();
        self.join();
    }
}
