//! 2단계 벡터 인덱싱 워커
//!
//! FTS 인덱싱 완료 후 백그라운드에서 벡터 임베딩 수행
//! - 파이프라인 병렬화: DB 프리페치 + 임베딩 동시 진행
//! - 128청크씩 배치 임베딩 (SIMD 효율 극대화)
//! - 주기적 진행률 이벤트 emit
//! - 취소 지원

use crate::commands::settings::IndexingIntensity;
use crate::db::{self, PendingChunk};
use crate::embedder::Embedder;
use crate::search::vector::VectorIndex;
use crossbeam_channel::{bounded, RecvTimeoutError};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread::JoinHandle;
use std::time::Duration;

/// 벡터 인덱싱 배치 크기 (128로 증가하여 ONNX SIMD 효율 극대화)
const EMBEDDING_BATCH_SIZE: usize = 128;

/// 벡터 인덱스 저장 주기 (청크 수) - I/O 최적화를 위해 1000으로 증가
const SAVE_INTERVAL: usize = 1000;

/// 프리페치 버퍼 크기 (배치 4개 분량)
const PREFETCH_BUFFER_SIZE: usize = 4;

/// 프리페치된 배치 데이터
struct PrefetchedBatch {
    file_id: i64,
    file_path: String,
    chunks: Vec<PendingChunk>,
}

/// 벡터 인덱싱 상태
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct VectorIndexingStatus {
    pub is_running: bool,
    pub total_chunks: usize,
    pub processed_chunks: usize,
    pub pending_chunks: usize,
    pub current_file: Option<String>,
    pub error: Option<String>,
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
        embedder: Arc<Embedder>,
        vector_index: Arc<VectorIndex>,
        progress_callback: Option<VectorProgressCallback>,
        intensity: Option<IndexingIntensity>,
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
                pending_chunks: 0,
                current_file: None,
                error: None,
            };
        }

        let cancel_flag = self.cancel_flag.clone();
        let status = self.status.clone();
        let intensity = intensity.unwrap_or(IndexingIntensity::Balanced);

        // 백그라운드 스레드 시작
        let handle = std::thread::spawn(move || {
            // 스레드 우선순위 설정 (Windows)
            #[cfg(target_os = "windows")]
            set_thread_priority(&intensity);

            let result = run_vector_indexing(
                &db_path,
                &embedder,
                &vector_index,
                &cancel_flag,
                &status,
                progress_callback,
                &intensity,
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

/// 벡터 인덱싱 실행 (파이프라인 병렬화)
///
/// 구조: [DB 프리페치 스레드] --배치--> [메인 스레드: 임베딩 + 저장]
/// DB I/O와 임베딩이 병렬로 진행되어 처리량 향상
fn run_vector_indexing(
    db_path: &Path,
    embedder: &Arc<Embedder>,
    vector_index: &Arc<VectorIndex>,
    cancel_flag: &Arc<AtomicBool>,
    status: &Arc<RwLock<VectorIndexingStatus>>,
    progress_callback: Option<VectorProgressCallback>,
    intensity: &IndexingIntensity,
) -> Result<(), String> {
    let conn = db::get_connection(db_path)
        .map_err(|e| format!("DB connection failed: {}", e))?;

    // 통계 조회
    let stats = db::get_vector_indexing_stats(&conn)
        .map_err(|e| format!("Failed to get stats: {}", e))?;

    let total_chunks = stats.pending_chunks;

    tracing::info!("[VectorWorker] Starting pipeline. {} chunks pending", total_chunks);

    // 상태 업데이트
    if let Ok(mut s) = status.write() {
        s.total_chunks = total_chunks;
        s.pending_chunks = total_chunks;
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

    // 파이프라인 채널 생성
    let (batch_tx, batch_rx) = bounded::<PrefetchedBatch>(PREFETCH_BUFFER_SIZE);

    // 프리페치 스레드용 DB 경로 복사
    let prefetch_db_path = db_path.to_path_buf();
    let prefetch_cancel = cancel_flag.clone();

    // 프리페치 스레드 시작
    let prefetch_handle = std::thread::spawn(move || {
        run_prefetch_thread(prefetch_db_path, batch_tx, prefetch_cancel);
    });

    // 메인 루프: 임베딩 + 저장
    let mut processed = 0;
    let mut last_save = 0;
    let recv_timeout = Duration::from_millis(100);

    loop {
        // 취소 확인
        if cancel_flag.load(Ordering::Relaxed) {
            tracing::info!("[VectorWorker] Cancelled");
            send_progress(processed, None, false);
            break;
        }

        match batch_rx.recv_timeout(recv_timeout) {
            Ok(prefetched) => {
                let current_file = Some(prefetched.file_path.as_str());

                // 상태 업데이트
                if let Ok(mut s) = status.write() {
                    s.current_file = Some(prefetched.file_path.clone());
                    s.processed_chunks = processed;
                    s.pending_chunks = total_chunks.saturating_sub(processed);
                }

                send_progress(processed, current_file, false);

                // 배치 단위로 임베딩
                for batch in prefetched.chunks.chunks(EMBEDDING_BATCH_SIZE) {
                    // 취소 확인
                    if cancel_flag.load(Ordering::Relaxed) {
                        break;
                    }

                    let contents: Vec<String> = batch.iter().map(|c| c.content.clone()).collect();
                    let chunk_ids: Vec<i64> = batch.iter().map(|c| c.chunk_id).collect();

                    // 임베딩 생성
                    let embeddings = match embedder.embed_batch(&contents) {
                        Ok(emb) => emb,
                        Err(e) => {
                            tracing::warn!("[VectorWorker] Embedding failed: {}", e);
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

                    // 인덱싱 강도에 따른 쓰로틀링
                    match intensity {
                        IndexingIntensity::Fast => {} // sleep 없음
                        IndexingIntensity::Balanced => std::thread::sleep(Duration::from_millis(200)),
                        IndexingIntensity::Background => std::thread::sleep(Duration::from_millis(500)),
                    }

                    // 상태 업데이트
                    if let Ok(mut s) = status.write() {
                        s.processed_chunks = processed;
                        s.pending_chunks = total_chunks.saturating_sub(processed);
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
                if let Err(e) = db::mark_file_vector_indexed(&conn, prefetched.file_id) {
                    tracing::warn!("[VectorWorker] Failed to mark file {}: {}", prefetched.file_id, e);
                }
            }
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }

    // batch_rx를 먼저 drop해야 프리페치 스레드의 batch_tx.send()가
    // SendError를 반환하여 스레드가 종료될 수 있음 (데드락 방지)
    drop(batch_rx);

    // 프리페치 스레드 종료 대기
    let _ = prefetch_handle.join();

    // 최종 저장
    if let Err(e) = vector_index.save() {
        tracing::warn!("[VectorWorker] Final save failed: {}", e);
    }

    tracing::info!("[VectorWorker] Completed. {} chunks processed", processed);

    // 상태 업데이트
    if let Ok(mut s) = status.write() {
        s.processed_chunks = processed;
        s.current_file = None;
        s.pending_chunks = 0;
    }

    send_progress(processed, None, true);

    Ok(())
}

/// 프리페치 스레드: DB에서 청크를 미리 로드하여 채널로 전송
fn run_prefetch_thread(
    db_path: PathBuf,
    batch_tx: crossbeam_channel::Sender<PrefetchedBatch>,
    cancel_flag: Arc<AtomicBool>,
) {
    let conn = match db::get_connection(&db_path) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("[Prefetch] DB connection failed: {}", e);
            return;
        }
    };

    // 대기 중인 파일 ID 목록
    let pending_file_ids = match db::get_pending_vector_file_ids(&conn) {
        Ok(ids) => ids,
        Err(e) => {
            tracing::error!("[Prefetch] Failed to get pending files: {}", e);
            return;
        }
    };

    for file_id in pending_file_ids {
        // 취소 확인
        if cancel_flag.load(Ordering::Relaxed) {
            tracing::debug!("[Prefetch] Cancelled");
            break;
        }

        // 해당 파일의 청크 로드 (DB 레벨 필터링)
        let file_chunks = match db::get_pending_vector_chunks_for_file(&conn, file_id, EMBEDDING_BATCH_SIZE * 10) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("[Prefetch] Failed to get chunks for file {}: {}", file_id, e);
                continue;
            }
        };

        if file_chunks.is_empty() {
            continue;
        }

        let file_path = file_chunks
            .first()
            .map(|c| c.file_path.clone())
            .unwrap_or_default();

        // 배치 전송
        let batch = PrefetchedBatch {
            file_id,
            file_path,
            chunks: file_chunks,
        };

        if batch_tx.send(batch).is_err() {
            // 수신자가 종료됨
            tracing::debug!("[Prefetch] Receiver dropped, stopping");
            break;
        }
    }

    tracing::debug!("[Prefetch] Thread completed");
}

/// Windows 스레드 우선순위 설정
#[cfg(target_os = "windows")]
fn set_thread_priority(intensity: &IndexingIntensity) {
    use windows_sys::Win32::System::Threading::{
        GetCurrentThread, SetThreadPriority,
        THREAD_PRIORITY_BELOW_NORMAL, THREAD_PRIORITY_IDLE, THREAD_PRIORITY_NORMAL,
    };

    let priority = match intensity {
        IndexingIntensity::Fast => THREAD_PRIORITY_NORMAL,
        IndexingIntensity::Balanced => THREAD_PRIORITY_BELOW_NORMAL,
        IndexingIntensity::Background => THREAD_PRIORITY_IDLE,
    };

    unsafe {
        let handle = GetCurrentThread();
        if SetThreadPriority(handle, priority) == 0 {
            tracing::warn!("[VectorWorker] Failed to set thread priority");
        } else {
            tracing::info!("[VectorWorker] Thread priority set to {:?}", intensity);
        }
    }
}
