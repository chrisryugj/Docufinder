//! 파일 감시 + 증분 인덱싱 매니저
//!
//! FileWatcher 이벤트를 받아서 증분 인덱싱 수행

use crate::constants::SUPPORTED_EXTENSIONS;
use crate::db;
use crate::embedder::Embedder;
use crate::indexer::pipeline;
use crate::search::vector::VectorIndex;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// 파일 감시 + 인덱싱 매니저
pub struct WatchManager {
    watcher: RecommendedWatcher,
    stop_tx: Sender<()>,
    watched_folders: HashSet<PathBuf>,
}

/// 인덱싱에 필요한 컨텍스트
pub struct IndexContext {
    pub db_path: PathBuf,
    pub embedder: Option<Arc<Mutex<Embedder>>>,
    pub vector_index: Option<Arc<VectorIndex>>,
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

        // 백그라운드 이벤트 처리 스레드
        thread::spawn(move || {
            Self::event_loop(event_rx, stop_rx, ctx);
        });

        Ok(Self {
            watcher,
            stop_tx,
            watched_folders: HashSet::new(),
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
                    Self::collect_files_from_event(&event, &mut pending_files);
                    last_event_time = std::time::Instant::now();
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // 디바운스 시간이 지났고 대기 중인 파일이 있으면 처리
                    if !pending_files.is_empty()
                        && last_event_time.elapsed() >= debounce_duration
                    {
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

    /// 이벤트에서 처리할 파일 수집
    fn collect_files_from_event(event: &Event, pending: &mut HashSet<PathBuf>) {
        for path in &event.paths {
            // 지원 확장자 확인 (삭제된 파일도 확장자로 판단 가능)
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            if !SUPPORTED_EXTENSIONS.contains(&ext.as_str()) {
                continue;
            }

            // 숨김 파일 제외
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with('.'))
                .unwrap_or(false)
            {
                continue;
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

        tracing::info!("Processing {} changed files", pending.len());

        let conn = match db::get_connection(&ctx.db_path) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("Failed to get DB connection: {}", e);
                return;
            }
        };

        for path in pending.drain() {
            if !path.exists() {
                // 파일이 삭제된 경우 - 벡터 인덱스와 DB에서 삭제
                let path_str = path.to_string_lossy().to_string();

                // 1. 벡터 인덱스에서 삭제 (DB 삭제 전에 chunk_ids 조회 필요)
                if let Some(vi) = &ctx.vector_index {
                    if let Ok(chunk_ids) = db::get_chunk_ids_for_path(&conn, &path_str) {
                        for chunk_id in chunk_ids {
                            if let Err(e) = vi.remove(chunk_id) {
                                tracing::debug!("Failed to remove vector {}: {}", chunk_id, e);
                            }
                        }
                    }
                }

                // 2. DB에서 삭제
                if let Err(e) = db::delete_file(&conn, &path_str) {
                    tracing::warn!("Failed to delete file from DB: {}", e);
                } else {
                    tracing::info!("Deleted from index: {}", path_str);
                }
                continue;
            }

            // 파일 인덱싱
            match pipeline::index_file(
                &conn,
                &path,
                ctx.embedder.as_ref(),
                ctx.vector_index.as_ref(),
            ) {
                Ok(result) => {
                    tracing::info!(
                        "Indexed: {} ({} chunks, {} vectors)",
                        result.file_path,
                        result.chunks_count,
                        result.vectors_count
                    );
                }
                Err(e) => {
                    tracing::warn!("Failed to index {:?}: {}", path, e);
                }
            }
        }

        // 벡터 인덱스 저장
        if let Some(vi) = &ctx.vector_index {
            if let Err(e) = vi.save() {
                tracing::warn!("Failed to save vector index: {}", e);
            }
        }
    }
}

impl Drop for WatchManager {
    fn drop(&mut self) {
        let _ = self.stop_tx.send(());
    }
}
