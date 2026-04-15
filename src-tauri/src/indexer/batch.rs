//! Batch Indexing - 다중 폴더 순차 인덱싱 상태 관리
//!
//! 여러 드라이브/폴더를 하나의 배치로 묶어 진행 상태를 추적.
//! Rust 측에서 순차 실행을 전담하여 프론트 IPC 타임아웃 문제를 원천 차단.

use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BatchJobStatus {
    Pending,
    Running,
    Committing,
    Done,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize)]
pub struct BatchJob {
    pub index: usize,
    pub path: String,
    pub status: BatchJobStatus,
    /// 세부 단계: "scanning" | "parsing" | "indexing" | "fts_commit" | "wal_checkpoint" | "cache_refresh"
    pub stage: Option<String>,
    pub processed: usize,
    pub total: usize,
    pub current_file: Option<String>,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
    pub indexed_count: usize,
    pub failed_count: usize,
    pub error: Option<String>,
}

impl BatchJob {
    pub fn new(index: usize, path: String) -> Self {
        Self {
            index,
            path,
            status: BatchJobStatus::Pending,
            stage: None,
            processed: 0,
            total: 0,
            current_file: None,
            started_at: None,
            finished_at: None,
            indexed_count: 0,
            failed_count: 0,
            error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct BatchState {
    pub batch_id: String,
    pub jobs: Vec<BatchJob>,
    pub current_index: usize,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub is_running: bool,
}

/// 배치 실행 컨트롤러 - AppContainer에 주입되어 싱글톤 상태 관리
pub struct BatchController {
    state: Mutex<Option<BatchState>>,
    cancel_flag: AtomicBool,
}

impl BatchController {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(None),
            cancel_flag: AtomicBool::new(false),
        }
    }

    pub fn snapshot(&self) -> Option<BatchState> {
        self.state.lock().ok().and_then(|s| s.clone())
    }

    pub fn is_running(&self) -> bool {
        self.state
            .lock()
            .ok()
            .and_then(|s| s.as_ref().map(|b| b.is_running))
            .unwrap_or(false)
    }

    pub fn start(&self, batch_id: String, paths: Vec<String>) -> BatchState {
        let jobs = paths
            .into_iter()
            .enumerate()
            .map(|(i, p)| BatchJob::new(i, p))
            .collect();
        let now = chrono::Utc::now().timestamp();
        let state = BatchState {
            batch_id,
            jobs,
            current_index: 0,
            started_at: now,
            finished_at: None,
            is_running: true,
        };
        self.cancel_flag.store(false, Ordering::Release);
        if let Ok(mut guard) = self.state.lock() {
            *guard = Some(state.clone());
        }
        state
    }

    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel_flag.load(Ordering::Acquire)
    }

    /// 특정 job 필드 업데이트 후 스냅샷 반환
    pub fn update_job<F: FnOnce(&mut BatchJob)>(&self, index: usize, f: F) -> Option<BatchJob> {
        let mut guard = self.state.lock().ok()?;
        let state = guard.as_mut()?;
        let job = state.jobs.get_mut(index)?;
        f(job);
        Some(job.clone())
    }

    pub fn set_current_index(&self, index: usize) {
        if let Ok(mut guard) = self.state.lock() {
            if let Some(state) = guard.as_mut() {
                state.current_index = index;
            }
        }
    }

    pub fn finish(&self) {
        if let Ok(mut guard) = self.state.lock() {
            if let Some(state) = guard.as_mut() {
                state.is_running = false;
                state.finished_at = Some(chrono::Utc::now().timestamp());
            }
        }
    }

    /// 완료/취소 후 일정 시간 뒤 상태를 클리어 (UI에서 페이드아웃 처리 이후 호출 가정)
    #[allow(dead_code)]
    pub fn clear(&self) {
        if let Ok(mut guard) = self.state.lock() {
            *guard = None;
        }
    }
}

impl Default for BatchController {
    fn default() -> Self {
        Self::new()
    }
}
