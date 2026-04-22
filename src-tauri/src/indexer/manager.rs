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
use notify::{Config, Event, EventKind, PollWatcher, RecommendedWatcher, RecursiveMode, Watcher};
use once_cell::sync::OnceCell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[derive(Clone)]
pub struct WatchPauseHandle {
    pause_count: Arc<std::sync::atomic::AtomicUsize>,
}

impl WatchPauseHandle {
    pub fn new() -> Self {
        Self {
            pause_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    pub fn shared_counter(&self) -> Arc<std::sync::atomic::AtomicUsize> {
        self.pause_count.clone()
    }

    /// Soft pause: 카운터만 증가 (unwatch 없음, 중첩 가능)
    #[allow(dead_code)]
    pub fn pause_processing(&self) {
        let prev = self
            .pause_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        tracing::debug!(
            "[WatchPauseHandle] pause_processing: {} → {}",
            prev,
            prev + 1
        );
    }

    /// Soft resume: 카운터 감소 (compare_exchange로 underflow 방지)
    #[allow(dead_code)]
    pub fn resume_processing(&self) {
        loop {
            let current = self.pause_count.load(std::sync::atomic::Ordering::SeqCst);
            if current == 0 {
                tracing::warn!("[WatchPauseHandle] resume_processing called but was not paused");
                return;
            }
            if self
                .pause_count
                .compare_exchange(
                    current,
                    current - 1,
                    std::sync::atomic::Ordering::SeqCst,
                    std::sync::atomic::Ordering::SeqCst,
                )
                .is_ok()
            {
                tracing::debug!(
                    "[WatchPauseHandle] resume_processing: {} → {}",
                    current,
                    current - 1
                );
                return;
            }
        }
    }

    #[allow(dead_code)]
    pub fn is_paused(&self) -> bool {
        self.pause_count.load(std::sync::atomic::Ordering::SeqCst) > 0
    }
}

/// 파일 감시 + 인덱싱 매니저
pub struct WatchManager {
    watcher: RecommendedWatcher,
    /// 네트워크 폴더(UNC) 전용 폴링 watcher.
    /// notify 의 RecommendedWatcher 는 SMB 위에서 inotify/ReadDirectoryChangesW 가 없어
    /// 이벤트가 누락되거나 아예 동작하지 않으므로, 네트워크 경로는 30초 주기 폴링으로 대체한다.
    poll_watcher: PollWatcher,
    /// 어느 watcher 에 등록되었는지 추적 (unwatch 시 분기용)
    poll_watched: HashSet<PathBuf>,
    stop_tx: Sender<()>,
    watched_folders: HashSet<PathBuf>,
    /// 🔴 Critical 버그 수정: worker thread handle 저장
    worker_thread: Option<JoinHandle<()>>,
    /// 일시 중지 카운터 (인덱싱 중 DB 동시 접근 방지)
    /// > 0 이면 debounce 후 pending_files 처리를 건너뜀.
    /// > AtomicUsize로 중첩 pause/resume 지원.
    pause_count: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    /// pause()가 반환될 때 watcher DB 작업이 완전히 끝났음을 보장하는 게이트
    processing_lock: Arc<Mutex<()>>,
}

/// 인덱싱에 필요한 컨텍스트
///
/// 벡터/임베더는 AppContainer의 OnceCell을 **공유**한다.
/// WatchManager 생성 시점에는 아직 init되지 않을 수 있으므로,
/// 매번 `.get()`으로 최신 상태를 읽어야 한다 (orphan 벡터 방지).
pub struct IndexContext {
    pub db_path: PathBuf,
    pub embedder: Arc<OnceCell<Arc<Embedder>>>,
    pub vector_index: Arc<OnceCell<Arc<VectorIndex>>>,
    /// 파일명 캐시 (증분 인덱싱 시 동기화)
    pub filename_cache: Arc<FilenameCache>,
    /// 파일 크기 제한 (MB) — 초과 시 메타데이터만 저장
    pub runtime_settings: WatchRuntimeSettingsProvider,
    /// 증분 인덱싱 완료 시 호출되는 콜백 (프론트엔드 알림용)
    pub on_incremental_update: Option<Arc<dyn Fn(usize) + Send + Sync>>,
    /// 벡터 워커 트리거 콜백 (watcher 증분 인덱싱 후 벡터 백필 시작)
    pub on_vector_trigger: Option<Arc<dyn Fn() + Send + Sync>>,
    /// OCR 엔진 (이미지 파일 텍스트 인식)
    pub ocr_engine: Option<Arc<OcrEngine>>,
}

#[derive(Debug, Clone)]
pub struct WatchRuntimeSettings {
    pub max_file_size_mb: u64,
    pub excluded_dirs: Vec<String>,
}

pub type WatchRuntimeSettingsProvider = Arc<dyn Fn() -> WatchRuntimeSettings + Send + Sync>;

impl WatchManager {
    /// 새 WatchManager 생성 및 백그라운드 스레드 시작
    pub fn new(
        ctx: IndexContext,
        pause_count: Arc<std::sync::atomic::AtomicUsize>,
    ) -> Result<Self, notify::Error> {
        // bounded 채널: 전체 드라이브 감시 시 이벤트 폭주로 인한 메모리 무한 증가 방지
        // 10_000: 500ms 디바운스 내 최대 이벤트 수 (초과 시 watcher 백프레셔로 자동 조절)
        let (event_tx, event_rx) = mpsc::sync_channel::<Event>(10_000);
        let (stop_tx, stop_rx) = mpsc::channel::<()>();

        let paused_for_loop = pause_count.clone();
        let processing_lock = Arc::new(Mutex::new(()));
        let processing_lock_for_loop = processing_lock.clone();

        // 두 watcher 가 같은 채널로 이벤트를 보낸다 (consumer 입장에선 출처 무관).
        let event_tx_for_recommended = event_tx.clone();
        let watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    let _ = event_tx_for_recommended.send(event);
                }
            },
            Config::default().with_poll_interval(Duration::from_secs(2)),
        )?;

        // 네트워크 폴더 전용 PollWatcher (30초 주기). compare_contents=false 로 두어
        // 메타데이터(mtime/size)만 비교 — 내용 다이제스트 계산이 SMB 왕복을 무한 늘리는 것을 방지.
        let poll_watcher = PollWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    let _ = event_tx.send(event);
                }
            },
            Config::default()
                .with_poll_interval(Duration::from_secs(30))
                .with_compare_contents(false),
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
            poll_watcher,
            poll_watched: HashSet::new(),
            stop_tx,
            watched_folders: HashSet::new(),
            worker_thread: Some(worker_thread),
            pause_count,
            processing_lock,
        })
    }

    /// 폴더 감시 시작.
    /// UNC 경로(`\\server\share\...`)는 PollWatcher 30초 주기로, 그 외는 RecommendedWatcher 로 등록한다.
    pub fn watch(&mut self, path: &Path) -> Result<(), notify::Error> {
        if self.watched_folders.contains(path) {
            tracing::debug!("Already watching: {:?}", path);
            return Ok(());
        }
        if crate::utils::network_path::is_network(path) {
            self.poll_watcher.watch(path, RecursiveMode::Recursive)?;
            self.poll_watched.insert(path.to_path_buf());
            tracing::info!("Started watching (PollWatcher 30s, network): {:?}", path);
        } else {
            self.watcher.watch(path, RecursiveMode::Recursive)?;
            tracing::info!("Started watching (RecommendedWatcher): {:?}", path);
        }
        self.watched_folders.insert(path.to_path_buf());
        // git 프로젝트 루트면 gitignore 매처 등록 (startup sync에서도 이 경로 통과)
        if path.join(".git").exists() {
            crate::indexer::gitignore_matcher::global().register_root(path);
        }
        Ok(())
    }

    /// 폴더 감시 중지
    pub fn unwatch(&mut self, path: &Path) -> Result<(), notify::Error> {
        if self.poll_watched.remove(path) {
            self.poll_watcher.unwatch(path)?;
        } else {
            self.watcher.unwatch(path)?;
        }
        self.watched_folders.remove(path);
        crate::indexer::gitignore_matcher::global().unregister_root(path);
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
            if self.poll_watched.remove(&path) {
                let _ = self.poll_watcher.unwatch(&path);
            } else {
                let _ = self.watcher.unwatch(&path);
            }
            tracing::debug!("Stopped watching: {:?}", path);
        }
        tracing::info!("All watchers stopped");
    }

    /// 파일 감시 일시 중지 (인덱싱 중 DB 동시 접근 방지)
    ///
    /// 1. pause_count 증가 → worker_thread가 debounce 후 pending_files를 버림
    /// 2. unwatch_all() → 새 이벤트 수신 차단
    /// 3. processing_lock 획득/해제 → 현재 배치 처리 완료 보장 (quiesce, 30초 타임아웃)
    pub fn pause(&mut self) {
        self.pause_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.unwatch_all();
        // 현재 배치 완료 대기 (타임아웃 30초 — 거대 PDF 파싱 hang 방지)
        let lock_result =
            self.try_lock_with_timeout(&self.processing_lock, Duration::from_secs(30));
        if !lock_result {
            tracing::warn!(
                "[WatchManager] processing_lock timeout (30s). Proceeding without quiesce."
            );
        }
        tracing::info!(
            "File watching paused (count={})",
            self.pause_count.load(std::sync::atomic::Ordering::SeqCst)
        );
    }

    /// processing_lock을 타임아웃 부여하여 획득 시도
    fn try_lock_with_timeout(&self, lock: &Arc<Mutex<()>>, timeout: Duration) -> bool {
        let start = std::time::Instant::now();
        loop {
            match lock.try_lock() {
                Ok(_guard) => return true,
                Err(std::sync::TryLockError::WouldBlock) => {
                    if start.elapsed() >= timeout {
                        return false;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(std::sync::TryLockError::Poisoned(e)) => {
                    drop(e.into_inner());
                    return true;
                }
            }
        }
    }

    /// 파일 감시 재개 (폴더 목록으로 재등록)
    ///
    /// 카운터 감소 후 0이 되었을 때만 실제로 폴더를 재등록.
    /// 중첩된 pause가 있으면 마지막 resume에서만 활성화.
    pub fn resume_with_folders(&mut self, folders: &[String]) {
        loop {
            let current = self.pause_count.load(std::sync::atomic::Ordering::SeqCst);
            if current == 0 {
                tracing::warn!("resume_with_folders called but was not paused");
                return;
            }
            let new_val = current - 1;
            if self
                .pause_count
                .compare_exchange(
                    current,
                    new_val,
                    std::sync::atomic::Ordering::SeqCst,
                    std::sync::atomic::Ordering::SeqCst,
                )
                .is_ok()
            {
                if new_val == 0 {
                    // 마지막 resume: 실제로 폴더 재등록
                    for folder in folders {
                        if let Err(e) = self.watch(Path::new(folder)) {
                            tracing::warn!("Failed to resume watching {:?}: {}", folder, e);
                        }
                    }
                    tracing::info!("File watching resumed ({} folders)", folders.len());
                } else {
                    tracing::debug!("File watching still paused (count={})", new_val);
                }
                return;
            }
        }
    }

    /// 현재 pause 상태 여부 (중첩 카운터 > 0 이면 paused).
    ///
    /// 다른 sync 작업(startup/periodic)이 진행 중인지 확인하는 용도로도 쓰인다.
    pub fn is_paused(&self) -> bool {
        self.pause_count.load(std::sync::atomic::Ordering::SeqCst) > 0
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
        pause_count: std::sync::Arc<std::sync::atomic::AtomicUsize>,
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
                        if pause_count.load(std::sync::atomic::Ordering::SeqCst) > 0 {
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

            // Windows 시스템 파일 (ntuser.dat.LOG2 등이 끊임없이 이벤트 발생시킴)
            if crate::indexer::exclusions::is_excluded_system_file(file_name) {
                continue;
            }

            // .gitignore 매치 — 개발 프로젝트의 node_modules/target/dist 등이 발생시키는
            // 반복 변경 이벤트 차단 (루트는 collector에서 등록됨)
            let is_dir_hint = matches!(event.kind, EventKind::Remove(_)) && !path.exists();
            if crate::indexer::gitignore_matcher::global().is_ignored(path, is_dir_hint) {
                tracing::debug!("Skipping gitignored path: {:?}", path);
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
                EventKind::Create(_) | EventKind::Modify(_)
                    // 생성/수정 이벤트: 파일 존재 확인 (디렉토리 제외)
                    if path.is_file() => {
                        tracing::debug!("File changed: {:?}", path);
                        pending.insert(path.clone());
                    }
                _ => {}
            }
        }
    }

    /// 대기 중인 파일들 처리
    ///
    /// 삭제 파일은 개별 처리, 존재 파일은 배치 트랜잭션으로 묶어 fsync 횟수 감소
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
        let runtime_settings = (ctx.runtime_settings)();

        // 삭제 파일과 존재 파일 분리
        let mut to_delete: Vec<PathBuf> = Vec::new();
        let mut to_index: Vec<PathBuf> = Vec::new();
        for path in pending.drain() {
            if !path.exists() {
                to_delete.push(path);
            } else {
                to_index.push(path);
            }
        }

        // 1. 삭제 파일 처리 (각각 개별 처리)
        for path in &to_delete {
            let path_str = path.to_string_lossy().to_string();

            let file_id: Option<i64> = conn
                .query_row("SELECT id FROM files WHERE path = ?", [&path_str], |row| {
                    row.get(0)
                })
                .ok();

            if let Some(vi) = ctx.vector_index.get() {
                if let Ok(chunk_ids) = db::get_chunk_ids_for_path(&conn, &path_str) {
                    for chunk_id in chunk_ids {
                        if let Err(e) = vi.remove(chunk_id) {
                            tracing::debug!("Failed to remove vector {}: {}", chunk_id, e);
                        }
                    }
                }
            }

            let path_str_ref = &path_str;
            if let Err(e) = db::retry_on_busy(|| db::delete_file(&conn, path_str_ref)) {
                tracing::warn!("Failed to delete file from DB: {}", e);
            } else {
                if let Some(fid) = file_id {
                    ctx.filename_cache.remove(fid);
                }
                tracing::info!("Deleted from index + cache: {}", path_str);
            }
        }

        // 2. 존재하는 파일들 배치 트랜잭션 (파일당 개별 fsync → 50개 배치 1회 fsync)
        if !to_index.is_empty() {
            const INCREMENTAL_BATCH_SIZE: usize = 50;
            let _ = conn.execute_batch("BEGIN");
            let mut batch_count = 0;
            let mut indexed_paths: Vec<String> = Vec::new();

            for path in &to_index {
                // 파일 크기 제한 체크 — 초과 시 메타데이터만 저장
                if runtime_settings.max_file_size_mb > 0 {
                    if let Ok(metadata) = std::fs::metadata(path) {
                        let size_mb = metadata.len() / (1024 * 1024);
                        if size_mb > runtime_settings.max_file_size_mb {
                            tracing::info!(
                                "[WatchManager] File too large ({}MB > {}MB), metadata only: {}",
                                size_mb,
                                runtime_settings.max_file_size_mb,
                                path.display()
                            );
                            Self::cleanup_stale_vectors(&conn, path, ctx.vector_index.get());
                            if let Ok(path_str) =
                                pipeline::save_file_metadata_and_cache(&conn, path)
                            {
                                indexed_paths.push(path_str);
                            }
                            batch_count += 1;
                            if batch_count >= INCREMENTAL_BATCH_SIZE {
                                if let Err(e) = conn.execute_batch("COMMIT; BEGIN") {
                                    tracing::warn!("Incremental batch commit failed: {}", e);
                                    if conn.is_autocommit() {
                                        let _ = conn.execute_batch("BEGIN");
                                    }
                                }
                                batch_count = 0;
                            }
                            continue;
                        }
                    }
                }

                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                let is_ocr_image =
                    ctx.ocr_engine.is_some() && OCR_IMAGE_EXTENSIONS.contains(&ext.as_str());

                if SUPPORTED_EXTENSIONS.contains(&ext.as_str()) || is_ocr_image {
                    let ocr_ref = ctx.ocr_engine.as_deref();
                    match pipeline::index_file_fts_only_no_tx(&conn, path, ocr_ref) {
                        Ok(result) => {
                            indexed_paths.push(result.file_path.clone());
                            tracing::info!(
                                "[FTS] Indexed: {} ({} chunks)",
                                result.file_path,
                                result.chunks_count
                            );
                        }
                        Err(e) => {
                            tracing::warn!("Failed to index {:?}: {}", path, e);
                        }
                    }
                } else {
                    Self::cleanup_stale_vectors(&conn, path, ctx.vector_index.get());
                    if let Ok(path_str) = pipeline::save_file_metadata_and_cache(&conn, path) {
                        indexed_paths.push(path_str);
                    }
                }

                batch_count += 1;
                if batch_count >= INCREMENTAL_BATCH_SIZE {
                    if let Err(e) = conn.execute_batch("COMMIT; BEGIN") {
                        tracing::warn!("Incremental batch commit failed: {}", e);
                        if conn.is_autocommit() {
                            let _ = conn.execute_batch("BEGIN");
                        }
                    }
                    batch_count = 0;
                }
            }

            if let Err(e) = conn.execute_batch("COMMIT") {
                tracing::warn!("Incremental final commit failed: {}", e);
            }

            // FilenameCache 일괄 갱신 (배치 COMMIT 후)
            for path_str in &indexed_paths {
                if let Ok(entry) = Self::get_filename_entry_from_db(&conn, path_str) {
                    ctx.filename_cache.upsert(entry);
                }
            }
            tracing::info!(
                "[WatchManager] Batch committed {} files",
                indexed_paths.len()
            );
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
        vector_index: Option<&Arc<VectorIndex>>,
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
                let path_lower = crate::utils::folder_scope::normalize_for_scope(&path);
                Ok(FilenameEntry {
                    file_id: row.get(0)?,
                    path_lower: path_lower.into_boxed_str(),
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
