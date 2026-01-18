//! 2단계 벡터 인덱싱 워커
//!
//! FTS 인덱싱 완료 후 백그라운드에서 벡터 임베딩 수행
//! - 별도 스레드에서 실행
//! - 32청크씩 배치 임베딩
//! - 주기적 진행률 이벤트 emit
//! - 취소 지원

use crate::db;
use crate::embedder::Embedder;
use crate::search::vector::VectorIndex;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread::JoinHandle;

/// 벡터 인덱싱 배치 크기
const EMBEDDING_BATCH_SIZE: usize = 32;

/// 벡터 인덱스 저장 주기 (청크 수) - I/O 최적화를 위해 500으로 증가
const SAVE_INTERVAL: usize = 500;

/// 벡터 인덱싱 상태
#[derive(Debug, Clone, serde::Serialize)]
pub struct VectorIndexingStatus {
    pub is_running: bool,
    pub total_chunks: usize,
    pub processed_chunks: usize,
    pub current_file: Option<String>,
    pub error: Option<String>,
}

impl Default for VectorIndexingStatus {
    fn default() -> Self {
        Self {
            is_running: false,
            total_chunks: 0,
            processed_chunks: 0,
            current_file: None,
            error: None,
        }
    }
}

/// 벡터 인덱싱 진행률 이벤트
#[derive(Debug, Clone, serde::Serialize)]
pub struct VectorIndexingProgress {
    pub total_chunks: usize,
    pub processed_chunks: usize,
    pub current_file: Option<String>,
    pub is_complete: bool,
}

/// 진행률 콜백 타입
pub type VectorProgressCallback = Arc<dyn Fn(VectorIndexingProgress) + Send + Sync>;

/// 벡터 인덱싱 워커
pub struct VectorWorker {
    thread: Option<JoinHandle<()>>,
    cancel_flag: Arc<AtomicBool>,
    status: Arc<RwLock<VectorIndexingStatus>>,
}

impl VectorWorker {
    /// 새 VectorWorker 생성
    pub fn new() -> Self {
        Self {
            thread: None,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            status: Arc::new(RwLock::new(VectorIndexingStatus::default())),
        }
    }

    /// 벡터 인덱싱 시작
    pub fn start(
        &mut self,
        db_path: PathBuf,
        embedder: Arc<Mutex<Embedder>>,
        vector_index: Arc<VectorIndex>,
        progress_callback: Option<VectorProgressCallback>,
    ) -> Result<(), String> {
        // 이미 실행 중이면 에러
        if self.is_running() {
            return Err("Vector indexing already running".to_string());
        }

        // 취소 플래그 리셋
        self.cancel_flag.store(false, Ordering::Relaxed);

        // 상태 초기화
        if let Ok(mut status) = self.status.write() {
            *status = VectorIndexingStatus {
                is_running: true,
                total_chunks: 0,
                processed_chunks: 0,
                current_file: None,
                error: None,
            };
        }

        let cancel_flag = self.cancel_flag.clone();
        let status = self.status.clone();

        // 백그라운드 스레드 시작
        let handle = std::thread::spawn(move || {
            let result = run_vector_indexing(
                &db_path,
                &embedder,
                &vector_index,
                &cancel_flag,
                &status,
                progress_callback,
            );

            // 완료/에러 상태 업데이트
            if let Ok(mut s) = status.write() {
                s.is_running = false;
                if let Err(e) = result {
                    s.error = Some(e);
                }
            }
        });

        self.thread = Some(handle);
        Ok(())
    }

    /// 현재 상태 조회
    pub fn get_status(&self) -> VectorIndexingStatus {
        self.status
            .read()
            .map(|s| s.clone())
            .unwrap_or_default()
    }

    /// 실행 중 여부
    pub fn is_running(&self) -> bool {
        self.status
            .read()
            .map(|s| s.is_running)
            .unwrap_or(false)
    }

    /// 취소 요청
    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::Relaxed);
    }

    /// 스레드 종료 대기
    pub fn join(&mut self) {
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

impl Default for VectorWorker {
    fn default() -> Self {
        Self::new()
    }
}

/// 벡터 인덱싱 실행 (내부 함수)
fn run_vector_indexing(
    db_path: &PathBuf,
    embedder: &Arc<Mutex<Embedder>>,
    vector_index: &Arc<VectorIndex>,
    cancel_flag: &Arc<AtomicBool>,
    status: &Arc<RwLock<VectorIndexingStatus>>,
    progress_callback: Option<VectorProgressCallback>,
) -> Result<(), String> {
    let conn = db::get_connection(db_path)
        .map_err(|e| format!("DB connection failed: {}", e))?;

    // 통계 조회
    let stats = db::get_vector_indexing_stats(&conn)
        .map_err(|e| format!("Failed to get stats: {}", e))?;

    let total_chunks = stats.pending_chunks;

    tracing::info!("[VectorWorker] Starting. {} chunks pending", total_chunks);

    // 상태 업데이트
    if let Ok(mut s) = status.write() {
        s.total_chunks = total_chunks;
    }

    // 진행률 알림
    let send_progress = |processed: usize, current_file: Option<&str>, is_complete: bool| {
        if let Some(ref cb) = progress_callback {
            cb(VectorIndexingProgress {
                total_chunks,
                processed_chunks: processed,
                current_file: current_file.map(|s| s.to_string()),
                is_complete,
            });
        }
    };

    let mut processed = 0;
    let mut last_save = 0;

    // 파일 단위로 처리 (벡터 완료 표시를 위해)
    let pending_file_ids = db::get_pending_vector_file_ids(&conn)
        .map_err(|e| format!("Failed to get pending files: {}", e))?;

    for file_id in pending_file_ids {
        // 취소 확인
        if cancel_flag.load(Ordering::Relaxed) {
            tracing::info!("[VectorWorker] Cancelled");
            send_progress(processed, None, false);
            return Ok(());
        }

        // 해당 파일의 청크 조회
        let chunks = db::get_pending_vector_chunks(&conn, EMBEDDING_BATCH_SIZE * 10)
            .map_err(|e| format!("Failed to get chunks: {}", e))?;

        // 해당 파일의 청크만 필터링
        let file_chunks: Vec<_> = chunks.into_iter().filter(|c| c.file_id == file_id).collect();

        if file_chunks.is_empty() {
            continue;
        }

        let current_file = file_chunks.first().map(|c| c.file_path.as_str());

        // 상태 업데이트
        if let Ok(mut s) = status.write() {
            s.current_file = current_file.map(|s| s.to_string());
            s.processed_chunks = processed;
        }

        send_progress(processed, current_file, false);

        // 배치 단위로 임베딩
        for batch in file_chunks.chunks(EMBEDDING_BATCH_SIZE) {
            // 취소 확인
            if cancel_flag.load(Ordering::Relaxed) {
                tracing::info!("[VectorWorker] Cancelled during batch");
                send_progress(processed, None, false);
                return Ok(());
            }

            let contents: Vec<String> = batch.iter().map(|c| c.content.clone()).collect();
            let chunk_ids: Vec<i64> = batch.iter().map(|c| c.chunk_id).collect();

            // 임베딩 생성
            let embeddings = match embedder.lock() {
                Ok(mut emb) => match emb.embed_batch(&contents) {
                    Ok(emb) => emb,
                    Err(e) => {
                        tracing::warn!("[VectorWorker] Embedding failed: {}", e);
                        continue;
                    }
                },
                Err(e) => {
                    tracing::warn!("[VectorWorker] Embedder lock failed: {}", e);
                    continue;
                }
            };

            // 벡터 인덱스에 추가
            for (chunk_id, embedding) in chunk_ids.iter().zip(embeddings.iter()) {
                if let Err(e) = vector_index.add(*chunk_id, embedding) {
                    tracing::warn!("[VectorWorker] Failed to add vector {}: {}", chunk_id, e);
                }
            }

            processed += batch.len();

            // 상태 업데이트
            if let Ok(mut s) = status.write() {
                s.processed_chunks = processed;
            }

            // 주기적 저장
            if processed - last_save >= SAVE_INTERVAL {
                if let Err(e) = vector_index.save() {
                    tracing::warn!("[VectorWorker] Failed to save index: {}", e);
                }
                last_save = processed;
            }
        }

        // 파일 완료 표시
        if let Err(e) = db::mark_file_vector_indexed(&conn, file_id) {
            tracing::warn!("[VectorWorker] Failed to mark file {}: {}", file_id, e);
        }
    }

    // 최종 저장
    if let Err(e) = vector_index.save() {
        tracing::warn!("[VectorWorker] Final save failed: {}", e);
    }

    tracing::info!("[VectorWorker] Completed. {} chunks processed", processed);

    // 상태 업데이트
    if let Ok(mut s) = status.write() {
        s.processed_chunks = processed;
        s.current_file = None;
    }

    send_progress(processed, None, true);

    Ok(())
}
