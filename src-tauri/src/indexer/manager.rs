//! 파일 감시 + 증분 인덱싱 매니저
//!
//! FileWatcher 이벤트를 받아서 증분 인덱싱 수행
//!
//! 🔴 Critical 버그 수정: 앱 종료 시 worker thread 정상 종료

use crate::constants::SUPPORTED_EXTENSIONS;
use crate::db;
use crate::embedder::Embedder;
use crate::indexer::pipeline;
use crate::search::filename_cache::{FilenameCache, FilenameEntry};
use crate::search::vector::VectorIndex;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// 파일 감시 + 인덱싱 매니저
pub struct WatchManager {
    watcher: RecommendedWatcher,
    stop_tx: Sender<()>,
    watched_folders: HashSet<PathBuf>,
    /// 🔴 Critical 버그 수정: worker thread handle 저장
    worker_thread: Option<JoinHandle<()>>,
}

/// 인덱싱에 필요한 컨텍스트
pub struct IndexContext {
    pub db_path: PathBuf,
    pub embedder: Option<Arc<Embedder>>,
    pub vector_index: Option<Arc<VectorIndex>>,
    /// 파일명 캐시 (증분 인덱싱 시 동기화)
    pub filename_cache: Arc<FilenameCache>,
    /// 파일 크기 제한 (MB) — 초과 시 메타데이터만 저장
    pub max_file_size_mb: u64,
    /// 증분 인덱싱 완료 시 호출되는 콜백 (프론트엔드 알림용)
    pub on_incremental_update: Option<Arc<dyn Fn(usize) + Send + Sync>>,
    /// 제외 디렉토리 목록 (대소문자 무시 비교)
    pub excluded_dirs: Vec<String>,
    /// 벡터 워커 트리거 콜백 (watcher 증분 인덱싱 후 벡터 백필 시작)
    pub on_vector_trigger: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl WatchManager {
    /// 새 WatchManager 생성 및 백그라운드 스레드 시작
    pub fn new(ctx: IndexContext) -> Result<Self, notify::Error> {
        let (event_tx, event_rx) = mpsc::channel::<Event>();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();

        // 파일 변경 이벤트를 채널로 전송
        let watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    let _ = event_tx.send(event);
                }
            },
            Config::default().with_poll_interval(Duration::from_secs(2)),
        )?;

        // 백그라운드 이벤트 처리 스레드 (handle 저장)
        let worker_thread = thread::spawn(move || {
            Self::event_loop(event_rx, stop_rx, ctx);
        });

        Ok(Self {
            watcher,
            stop_tx,
            watched_folders: HashSet::new(),
            worker_thread: Some(worker_thread),
        })
    }

    /// 폴더 감시 시작
    pub fn watch(&mut self, path: &Path) -> Result<(), notify::Error> {
        self.watcher.watch(path, RecursiveMode::Recursive)?;
        self.watched_folders.insert(path.to_path_buf());
        tracing::info!("Started watching: {:?}", path);
        Ok(())
    }

    /// 폴더 감시 중지
    pub fn unwatch(&mut self, path: &Path) -> Result<(), notify::Error> {
        self.watcher.unwatch(path)?;
        self.watched_folders.remove(path);
        tracing::info!("Stopped watching: {:?}", path);
        Ok(())
    }

    /// 현재 감시 중인 폴더 목록
    pub fn watched_folders(&self) -> Vec<PathBuf> {
        self.watched_folders.iter().cloned().collect()
    }

    /// 모든 폴더 감시 중지
    pub fn unwatch_all(&mut self) {
        for path in self.watched_folders.drain() {
            let _ = self.watcher.unwatch(&path);
            tracing::debug!("Stopped watching: {:?}", path);
        }
        tracing::info!("All watchers stopped");
    }

    /// 🔴 Critical 버그 수정: 명시적 종료 메서드
    ///
    /// stop 신호 전송 후 worker thread가 종료될 때까지 대기
    pub fn shutdown(&mut self) {
        tracing::info!("WatchManager shutdown requested");

        // stop 신호 전송
        let _ = self.stop_tx.send(());

        // worker thread 종료 대기
        if let Some(handle) = self.worker_thread.take() {
            tracing::debug!("Waiting for worker thread to finish...");
            if let Err(e) = handle.join() {
                tracing::warn!("Worker thread panicked: {:?}", e);
            } else {
                tracing::info!("Worker thread finished");
            }
        }
    }

    /// 이벤트 처리 루프
    fn event_loop(event_rx: Receiver<Event>, stop_rx: Receiver<()>, ctx: IndexContext) {
        // 디바운스를 위한 대기 중인 파일들
        let mut pending_files: HashSet<PathBuf> = HashSet::new();
        let mut last_event_time = std::time::Instant::now();
        let debounce_duration = Duration::from_millis(500);

        loop {
            // stop 신호 확인
            if stop_rx.try_recv().is_ok() {
                tracing::info!("WatchManager stopping");
                break;
            }

            // 이벤트 수신 (타임아웃 포함)
            match event_rx.recv_timeout(Duration::from_millis(100)) {
                Ok(event) => {
                    Self::collect_files_from_event(&event, &mut pending_files, &ctx.excluded_dirs);
                    last_event_time = std::time::Instant::now();
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // 디바운스 시간이 지났고 대기 중인 파일이 있으면 처리
                    if !pending_files.is_empty() && last_event_time.elapsed() >= debounce_duration {
                        Self::process_pending_files(&mut pending_files, &ctx);
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    tracing::info!("Event channel disconnected");
                    break;
                }
            }
        }
    }

    /// 이벤트에서 처리할 파일 수집 (모든 확장자, 임시/숨김 파일/제외 디렉토리만 제외)
    fn collect_files_from_event(
        event: &Event,
        pending: &mut HashSet<PathBuf>,
        excluded_dirs: &[String],
    ) {
        for path in &event.paths {
            // 숨김 파일 및 Office 임시 파일 (~$) 제외
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if file_name.starts_with('.') || file_name.starts_with("~$") {
                continue;
            }

            // 제외 디렉토리 하위 파일 스킵
            if !excluded_dirs.is_empty() {
                let is_excluded = path.ancestors().any(|ancestor| {
                    ancestor
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|name| excluded_dirs.iter().any(|ex| name.eq_ignore_ascii_case(ex)))
                        .unwrap_or(false)
                });
                if is_excluded {
                    continue;
                }
            }

            match &event.kind {
                EventKind::Remove(_) => {
                    // 삭제 이벤트: is_file() 체크 불필요 (파일이 이미 없음)
                    tracing::debug!("File removed: {:?}", path);
                    pending.insert(path.clone());
                }
                EventKind::Create(_) | EventKind::Modify(_) => {
                    // 생성/수정 이벤트: 파일 존재 확인 (디렉토리 제외)
                    if path.is_file() {
                        tracing::debug!("File changed: {:?}", path);
                        pending.insert(path.clone());
                    }
                }
                _ => {}
            }
        }
    }

    /// 대기 중인 파일들 처리
    fn process_pending_files(pending: &mut HashSet<PathBuf>, ctx: &IndexContext) {
        if pending.is_empty() {
            return;
        }

        let file_count = pending.len();
        tracing::info!("Processing {} changed files", file_count);

        let conn = match db::get_connection(&ctx.db_path) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("Failed to get DB connection: {}", e);
                return;
            }
        };

        for path in pending.drain() {
            if !path.exists() {
                // 파일이 삭제된 경우 - 벡터 인덱스, DB, FilenameCache에서 삭제
                let path_str = path.to_string_lossy().to_string();

                // 1. file_id 조회 (캐시 삭제용)
                let file_id: Option<i64> = conn
                    .query_row("SELECT id FROM files WHERE path = ?", [&path_str], |row| {
                        row.get(0)
                    })
                    .ok();

                // 2. 벡터 인덱스에서 삭제 (DB 삭제 전에 chunk_ids 조회 필요)
                if let Some(vi) = &ctx.vector_index {
                    if let Ok(chunk_ids) = db::get_chunk_ids_for_path(&conn, &path_str) {
                        for chunk_id in chunk_ids {
                            if let Err(e) = vi.remove(chunk_id) {
                                tracing::debug!("Failed to remove vector {}: {}", chunk_id, e);
                            }
                        }
                    }
                }

                // 3. DB에서 삭제
                if let Err(e) = db::delete_file(&conn, &path_str) {
                    tracing::warn!("Failed to delete file from DB: {}", e);
                } else {
                    // 4. FilenameCache에서 삭제
                    if let Some(fid) = file_id {
                        ctx.filename_cache.remove(fid);
                    }
                    tracing::info!("Deleted from index + cache: {}", path_str);
                }
                continue;
            }

            // 파일 크기 제한 체크 — 초과 시 메타데이터만 저장
            if ctx.max_file_size_mb > 0 {
                if let Ok(metadata) = std::fs::metadata(&path) {
                    let size_mb = metadata.len() / (1024 * 1024);
                    if size_mb > ctx.max_file_size_mb {
                        tracing::info!(
                            "[WatchManager] File too large ({}MB > {}MB), metadata only: {}",
                            size_mb,
                            ctx.max_file_size_mb,
                            path.display()
                        );
                        // stale 벡터 정리 (metadata-only 전환 시)
                        Self::cleanup_stale_vectors(&conn, &path, &ctx.vector_index);
                        match pipeline::save_file_metadata_and_cache(&conn, &path) {
                            Ok(file_path_str) => {
                                if let Ok(entry) =
                                    Self::get_filename_entry_from_db(&conn, &file_path_str)
                                {
                                    ctx.filename_cache.upsert(entry);
                                }
                            }
                            Err(e) => {
                                tracing::warn!("Failed to save metadata for {:?}: {}", path, e)
                            }
                        }
                        continue;
                    }
                }
            }

            // 확장자에 따라 파싱 인덱싱 또는 메타데이터만 저장
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            if SUPPORTED_EXTENSIONS.contains(&ext.as_str()) {
                // 파싱 가능: FTS 인덱싱 (벡터는 백그라운드 워커가 처리)
                match pipeline::index_file_fts_only(&conn, &path) {
                    Ok(result) => {
                        if let Ok(entry) =
                            Self::get_filename_entry_from_db(&conn, &result.file_path)
                        {
                            ctx.filename_cache.upsert(entry);
                        }
                        tracing::info!(
                            "[FTS] Indexed + cache updated: {} ({} chunks)",
                            result.file_path,
                            result.chunks_count
                        );
                    }
                    Err(e) => {
                        tracing::warn!("Failed to index {:?}: {}", path, e);
                    }
                }
            } else {
                // 파싱 불가: 메타데이터만 저장 (파일명 검색용)
                // stale 벡터 정리 (metadata-only 전환 시)
                Self::cleanup_stale_vectors(&conn, &path, &ctx.vector_index);
                match pipeline::save_file_metadata_and_cache(&conn, &path) {
                    Ok(file_path_str) => {
                        if let Ok(entry) = Self::get_filename_entry_from_db(&conn, &file_path_str) {
                            ctx.filename_cache.upsert(entry);
                        }
                        tracing::debug!("[Metadata] Stored: {}", file_path_str);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to save metadata for {:?}: {}", path, e);
                    }
                }
            }
        }
        // 프론트엔드에 증분 인덱싱 완료 알림
        if let Some(ref callback) = ctx.on_incremental_update {
            callback(file_count);
        }

        // 벡터 워커 트리거 (FTS 인덱싱 완료 후 pending 벡터 백필)
        if let Some(ref trigger) = ctx.on_vector_trigger {
            trigger();
        }
    }

    /// metadata-only 전환 시 기존 벡터 인덱스에서 stale 벡터 정리
    fn cleanup_stale_vectors(
        conn: &rusqlite::Connection,
        path: &Path,
        vector_index: &Option<Arc<VectorIndex>>,
    ) {
        let path_str = path.to_string_lossy().to_string();
        if let Some(vi) = vector_index {
            if let Ok(chunk_ids) = db::get_chunk_ids_for_path(conn, &path_str) {
                for chunk_id in chunk_ids {
                    if let Err(e) = vi.remove(chunk_id) {
                        tracing::debug!("Failed to remove stale vector {}: {}", chunk_id, e);
                    }
                }
            }
        }
    }

    /// DB에서 파일 정보 조회하여 FilenameEntry 생성
    fn get_filename_entry_from_db(
        conn: &rusqlite::Connection,
        path: &str,
    ) -> Result<FilenameEntry, rusqlite::Error> {
        conn.query_row(
            "SELECT id, path, name, file_type, COALESCE(size, 0), COALESCE(modified_at, 0)
             FROM files WHERE path = ?",
            [path],
            |row| {
                let name: String = row.get(2)?;
                let path: String = row.get(1)?;
                let file_type: String = row.get(3)?;
                Ok(FilenameEntry {
                    file_id: row.get(0)?,
                    path: path.into_boxed_str(),
                    name_lower: name.to_lowercase().into_boxed_str(),
                    file_type: file_type.into_boxed_str(),
                    size: row.get(4)?,
                    modified_at: row.get(5)?,
                })
            },
        )
    }
}

impl Drop for WatchManager {
    fn drop(&mut self) {
        // 🔴 Critical 버그 수정: Drop에서 shutdown 호출하여 thread join
        self.shutdown();
    }
}
