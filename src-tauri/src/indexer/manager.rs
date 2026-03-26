//! 파일 감시 + 증분 인덱싱 매니저
//!
//! FileWatcher 이벤트를 받아서 증분 인덱싱 수행
//!
//! 🔴 Critical 버그 수정: 앱 종료 시 worker thread 정상 종료

use crate::constants::{OCR_IMAGE_EXTENSIONS, SUPPORTED_EXTENSIONS};
use crate::db;
use crate::embedder::Embedder;
use crate::indexer::exclusions::is_excluded_dir;
use crate::indexer::pipeline;
use crate::ocr::OcrEngine;
use crate::search::filename_cache::{FilenameCache, FilenameEntry};
use crate::search::vector::VectorIndex;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[derive(Clone)]
pub struct WatchPauseHandle {
    paused: Arc<std::sync::atomic::AtomicBool>,
}

impl WatchPauseHandle {
    pub fn new() -> Self {
        Self {
            paused: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    pub fn shared_flag(&self) -> Arc<std::sync::atomic::AtomicBool> {
        self.paused.clone()
    }

    pub fn pause_processing(&self) {
        self.paused
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn resume_processing(&self) {
        self.paused
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }
}

/// 파일 감시 + 인덱싱 매니저
pub struct WatchManager {
    watcher: RecommendedWatcher,
    stop_tx: Sender<()>,
    watched_folders: HashSet<PathBuf>,
    /// 🔴 Critical 버그 수정: worker thread handle 저장
    worker_thread: Option<JoinHandle<()>>,
    /// 일시 중지 플래그 (인덱싱 중 DB 동시 접근 방지)
    /// true이면 debounce 후 pending_files 처리를 건너뜀
    paused: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// pause()가 반환될 때 watcher DB 작업이 완전히 끝났음을 보장하는 게이트
    processing_lock: Arc<Mutex<()>>,
}

/// 인덱싱에 필요한 컨텍스트
pub struct IndexContext {
    pub db_path: PathBuf,
    pub embedder: Option<Arc<Embedder>>,
    pub vector_index: Option<Arc<VectorIndex>>,
    /// 파일명 캐시 (증분 인덱싱 시 동기화)
    pub filename_cache: Arc<FilenameCache>,
    /// 파일 크기 제한 (MB) — 초과 시 메타데이터만 저장
    pub runtime_settings: WatchRuntimeSettingsProvider,
    /// 증분 인덱싱 완료 시 호출되는 콜백 (프론트엔드 알림용)
    pub on_incremental_update: Option<Arc<dyn Fn(usize) + Send + Sync>>,
    /// 벡터 워커 트리거 콜백 (watcher 증분 인덱싱 후 벡터 백필 시작)
    pub on_vector_trigger: Option<Arc<dyn Fn() + Send + Sync>>,
    /// HWP 파일 감지 콜백 (증분 인덱싱 시 새 HWP 파일 발견 알림)
    pub on_hwp_detected: Option<Arc<dyn Fn(Vec<String>) + Send + Sync>>,
    /// OCR 엔진 (이미지 파일 텍스트 인식)
    pub ocr_engine: Option<Arc<OcrEngine>>,
}

#[derive(Debug, Clone)]
pub struct WatchRuntimeSettings {
    pub max_file_size_mb: u64,
    pub excluded_dirs: Vec<String>,
    pub hwp_auto_detect: bool,
}

pub type WatchRuntimeSettingsProvider = Arc<dyn Fn() -> WatchRuntimeSettings + Send + Sync>;

impl WatchManager {
    /// 새 WatchManager 생성 및 백그라운드 스레드 시작
    pub fn new(
        ctx: IndexContext,
        paused: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<Self, notify::Error> {
        let (event_tx, event_rx) = mpsc::channel::<Event>();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();

        let paused_for_loop = paused.clone();
        let processing_lock = Arc::new(Mutex::new(()));
        let processing_lock_for_loop = processing_lock.clone();

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
            Self::event_loop(
                event_rx,
                stop_rx,
                ctx,
                paused_for_loop,
                processing_lock_for_loop,
            );
        });

        Ok(Self {
            watcher,
            stop_tx,
            watched_folders: HashSet::new(),
            worker_thread: Some(worker_thread),
            paused,
            processing_lock,
        })
    }

    /// 폴더 감시 시작
    pub fn watch(&mut self, path: &Path) -> Result<(), notify::Error> {
        if self.watched_folders.contains(path) {
            tracing::debug!("Already watching: {:?}", path);
            return Ok(());
        }
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

    /// 파일 감시 일시 중지 (인덱싱 중 DB 동시 접근 방지)
    ///
    /// 1. paused=true → worker_thread가 debounce 후 pending_files를 버림
    /// 2. unwatch_all() → 새 이벤트 수신 차단
    pub fn pause(&mut self) {
        self.paused
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.unwatch_all();
        drop(
            self.processing_lock
                .lock()
                .unwrap_or_else(|e| e.into_inner()),
        );
        tracing::info!("File watching paused");
    }

    /// 파일 감시 재개 (폴더 목록으로 재등록)
    ///
    /// watch() 등록 완료 후 paused=false 설정하여
    /// 이벤트가 처리 준비된 후에만 활성화
    pub fn resume_with_folders(&mut self, folders: &[String]) {
        for folder in folders {
            if let Err(e) = self.watch(Path::new(folder)) {
                tracing::warn!("Failed to resume watching {:?}: {}", folder, e);
            }
        }
        self.paused
            .store(false, std::sync::atomic::Ordering::Relaxed);
        tracing::info!("File watching resumed ({} folders)", folders.len());
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
    fn event_loop(
        event_rx: Receiver<Event>,
        stop_rx: Receiver<()>,
        ctx: IndexContext,
        paused: std::sync::Arc<std::sync::atomic::AtomicBool>,
        processing_lock: Arc<Mutex<()>>,
    ) {
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
                    let runtime_settings = (ctx.runtime_settings)();
                    Self::collect_files_from_event(
                        &event,
                        &mut pending_files,
                        &runtime_settings.excluded_dirs,
                    );
                    last_event_time = std::time::Instant::now();
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // 디바운스 시간이 지났고 대기 중인 파일이 있으면 처리
                    if !pending_files.is_empty() && last_event_time.elapsed() >= debounce_duration {
                        let _guard = processing_lock.lock().unwrap_or_else(|e| e.into_inner());
                        if paused.load(std::sync::atomic::Ordering::Relaxed) {
                            // 일시 중지 상태: pending_files 버림 (인덱싱 완료 후 resume 시 재감지됨)
                            let count = pending_files.len();
                            pending_files.clear();
                            tracing::debug!(
                                "File watching paused: discarded {} pending changes",
                                count
                            );
                        } else {
                            Self::process_pending_files(&mut pending_files, &ctx);
                        }
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
                let is_excluded = path
                    .ancestors()
                    .any(|ancestor| is_excluded_dir(ancestor, excluded_dirs));
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
        let mut hwp_files: Vec<String> = Vec::new();

        let conn = match db::get_connection(&ctx.db_path) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("Failed to get DB connection: {}", e);
                return;
            }
        };
        let runtime_settings = (ctx.runtime_settings)();

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
            if runtime_settings.max_file_size_mb > 0 {
                if let Ok(metadata) = std::fs::metadata(&path) {
                    let size_mb = metadata.len() / (1024 * 1024);
                    if size_mb > runtime_settings.max_file_size_mb {
                        tracing::info!(
                            "[WatchManager] File too large ({}MB > {}MB), metadata only: {}",
                            size_mb,
                            runtime_settings.max_file_size_mb,
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
            let is_ocr_image =
                ctx.ocr_engine.is_some() && OCR_IMAGE_EXTENSIONS.contains(&ext.as_str());
            if SUPPORTED_EXTENSIONS.contains(&ext.as_str()) || is_ocr_image {
                // 파싱 가능: FTS 인덱싱 (벡터는 백그라운드 워커가 처리)
                let ocr_ref = ctx.ocr_engine.as_deref();
                match pipeline::index_file_fts_only(&conn, &path, ocr_ref) {
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
                // HWP 파일 수집 (변환 알림용)
                if ext == "hwp" && runtime_settings.hwp_auto_detect {
                    hwp_files.push(path.to_string_lossy().to_string());
                }
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

        // HWP 파일 감지 알림 (설정 활성 시)
        if !hwp_files.is_empty() {
            if let Some(ref callback) = ctx.on_hwp_detected {
                callback(hwp_files);
            }
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
                    path_lower: path.to_lowercase().into_boxed_str(),
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
