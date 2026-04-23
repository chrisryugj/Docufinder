//! Batch Indexing Commands - 멀티 폴더 순차 인덱싱
//!
//! 프론트 IPC 타임아웃 문제를 원천 차단하기 위해 Rust 측에서 전체 루프를 소유.
//! 한 번의 `start_indexing_batch` 호출로 여러 드라이브/폴더를 순차 처리하고,
//! 각 단계를 이벤트로 프론트에 통지.

use super::*;
use crate::indexer::batch::{BatchJobStatus, BatchState};
use crate::indexer::pipeline::FtsIndexingProgress;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct BatchJobProgressPayload {
    pub batch_id: String,
    pub job_index: usize,
    pub path: String,
    pub status: BatchJobStatus,
    pub stage: Option<String>,
    pub processed: usize,
    pub total: usize,
    pub current_file: Option<String>,
    pub indexed_count: usize,
    pub failed_count: usize,
    pub error: Option<String>,
}

/// 배치 인덱싱 시작 (즉시 반환, 백그라운드 실행)
#[tauri::command]
pub async fn start_indexing_batch(
    paths: Vec<String>,
    app_handle: AppHandle,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<String> {
    tracing::info!("Starting indexing batch: {} paths", paths.len());

    let controller = {
        let container = state.read()?;
        container.get_batch_controller()
    };

    // 이미 실행 중이면 거부
    if controller.is_running() {
        return Err(ApiError::IndexingFailed(
            "이미 배치 인덱싱이 실행 중입니다".to_string(),
        ));
    }

    // 경로 정규화 + 이미 감시 중인 폴더 필터
    let db_path = {
        let container = state.read()?;
        container.db_path.clone()
    };
    let mut canonical_paths: Vec<String> = Vec::new();
    let mut rejected: Vec<(String, String)> = Vec::new();
    for raw in &paths {
        let p = Path::new(raw);
        if !p.exists() {
            tracing::warn!("Batch: skipping non-existent path: {}", raw);
            rejected.push((raw.clone(), "경로가 존재하지 않습니다".to_string()));
            continue;
        }
        // UNC/네트워크 경로는 dunce 로 정규화해야 `\\server\share\...` 형태가 유지됨.
        // std::canonicalize 는 `\\?\UNC\server\share\...` 로 바꿔 DB 경로 매칭을 깨뜨린다.
        let canonical_buf = match dunce::canonicalize(p) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Batch: canonicalize failed for {}: {}", raw, e);
                rejected.push((raw.clone(), format!("경로 정규화 실패: {}", e)));
                continue;
            }
        };
        // 시스템 폴더 차단 (드라이브 루트는 허용 — Everything 스타일 전체 검색 지원)
        if let Err(msg) = crate::constants::validate_watch_path(&canonical_buf) {
            tracing::warn!("Batch: rejecting path {}: {}", raw, msg);
            rejected.push((raw.clone(), msg.to_string()));
            continue;
        }
        let canonical = canonical_buf.to_string_lossy().to_string();
        // 이미 감시 중이면 skip (재인덱싱은 사용자가 명시적으로)
        if let Ok(conn) = crate::db::get_connection(&db_path) {
            if crate::db::is_folder_watched(&conn, &canonical).unwrap_or(false) {
                tracing::info!("Batch: already watched, skipping: {}", canonical);
                rejected.push((raw.clone(), "이미 인덱싱된 폴더입니다".to_string()));
                continue;
            }
        }
        canonical_paths.push(canonical);
    }

    // 거부된 경로들을 프론트에 알림 (조용한 실패 방지)
    for (path, reason) in &rejected {
        let _ = app_handle.emit(
            "indexing-warning",
            &serde_json::json!({
                "type": "path_rejected",
                "folder_path": path,
                "message": reason,
            }),
        );
    }

    if canonical_paths.is_empty() {
        let reason = if rejected.is_empty() {
            "선택된 경로가 없습니다".to_string()
        } else {
            let details: Vec<String> = rejected
                .iter()
                .map(|(p, r)| format!("• {} — {}", p, r))
                .collect();
            format!("인덱싱 가능한 경로가 없습니다:\n{}", details.join("\n"))
        };
        return Err(ApiError::IndexingFailed(reason));
    }

    let batch_id = format!("batch-{}", chrono::Utc::now().timestamp_millis());
    let initial_state = controller.start(batch_id.clone(), canonical_paths.clone());

    // batch-started 이벤트
    let _ = app_handle.emit("indexing-batch-started", &initial_state);

    // 백그라운드 실행 (State는 Send가 아니므로 AppHandle만 캡처)
    let app_handle_task = app_handle.clone();
    let batch_id_task = batch_id.clone();
    tauri::async_runtime::spawn(async move {
        run_batch(batch_id_task, canonical_paths, app_handle_task).await;
    });

    Ok(batch_id)
}

/// 현재 배치 상태 조회 (모달 재진입/새로고침 시 복구용)
#[tauri::command]
pub async fn get_indexing_batch(
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<Option<BatchState>> {
    let controller = {
        let container = state.read()?;
        container.get_batch_controller()
    };
    Ok(controller.snapshot())
}

/// 배치 인덱싱 취소
#[tauri::command]
pub async fn cancel_indexing_batch(state: State<'_, RwLock<AppContainer>>) -> ApiResult<()> {
    tracing::info!("Cancelling indexing batch...");
    let (controller, service) = {
        let container = state.read()?;
        (container.get_batch_controller(), container.index_service())
    };
    controller.cancel();
    service.cancel_indexing();
    Ok(())
}

// ============================================================
// 배치 실행 루프
// ============================================================

async fn run_batch(batch_id: String, paths: Vec<String>, app_handle: AppHandle) {
    // 초기 상태 추출 (State는 Send가 아니므로 필요 시점마다 state() 호출)
    let state_handle = app_handle.state::<RwLock<AppContainer>>();

    let controller = match state_handle.read() {
        Ok(c) => c.get_batch_controller(),
        Err(_) => {
            tracing::error!("run_batch: failed to acquire AppContainer read lock");
            return;
        }
    };

    // 1회만 pause — 배치 전체 동안 watcher 중지
    pause_watching(&state_handle);

    for (idx, path) in paths.iter().enumerate() {
        if controller.is_cancelled() {
            tracing::info!("Batch cancelled, marking remaining jobs");
            mark_remaining_cancelled(&controller, &batch_id, idx, &app_handle);
            break;
        }

        controller.set_current_index(idx);

        // job 시작
        let now = chrono::Utc::now().timestamp();
        let started_job = controller.update_job(idx, |job| {
            job.status = BatchJobStatus::Running;
            job.stage = Some("scanning".to_string());
            job.started_at = Some(now);
        });
        if let Some(job) = started_job {
            emit_job_progress(&app_handle, &batch_id, &job);
        }

        // 실제 인덱싱 실행
        let job_result = run_folder_index_job_batch(
            &state_handle,
            path,
            &controller,
            &batch_id,
            idx,
            &app_handle,
        )
        .await;

        // 경계 구간: committing (fts_commit → wal_checkpoint → cache_refresh)
        let committing_job = controller.update_job(idx, |job| {
            job.status = BatchJobStatus::Committing;
            job.stage = Some("wal_checkpoint".to_string());
        });
        if let Some(job) = committing_job {
            emit_job_progress(&app_handle, &batch_id, &job);
        }
        let db_path = state_handle.read().ok().map(|c| c.db_path.clone());
        if let Some(db_path) = db_path {
            crate::db::wal_checkpoint(&db_path);
        }

        let refresh_job = controller.update_job(idx, |job| {
            job.stage = Some("cache_refresh".to_string());
        });
        if let Some(job) = refresh_job {
            emit_job_progress(&app_handle, &batch_id, &job);
        }
        refresh_filename_cache(&state_handle);

        // 최종 완료 처리
        let finish_now = chrono::Utc::now().timestamp();
        let done_job = controller.update_job(idx, |job| {
            match &job_result {
                Ok((indexed, failed, was_cancelled)) => {
                    job.indexed_count = *indexed;
                    job.failed_count = *failed;
                    job.status = if *was_cancelled {
                        BatchJobStatus::Cancelled
                    } else {
                        BatchJobStatus::Done
                    };
                }
                Err(e) => {
                    job.status = BatchJobStatus::Failed;
                    job.error = Some(e.clone());
                }
            }
            job.stage = None;
            job.finished_at = Some(finish_now);
        });
        if let Some(job) = done_job {
            emit_job_progress(&app_handle, &batch_id, &job);
        }
    }

    // 배치 전체 종료 → watcher 재개 (1회)
    let db_path = state_handle.read().ok().map(|c| c.db_path.clone());
    if let Some(db_path) = db_path {
        resume_watching(&state_handle, &db_path);
    }

    controller.finish();
    let final_state = controller.snapshot();
    let _ = app_handle.emit("indexing-batch-completed", &final_state);
    tracing::info!("Batch {} completed", batch_id);
}

fn mark_remaining_cancelled(
    controller: &crate::indexer::batch::BatchController,
    batch_id: &str,
    from_idx: usize,
    app_handle: &AppHandle,
) {
    let snapshot = match controller.snapshot() {
        Some(s) => s,
        None => return,
    };
    for i in from_idx..snapshot.jobs.len() {
        let job = controller.update_job(i, |job| {
            if job.status == BatchJobStatus::Pending || job.status == BatchJobStatus::Running {
                job.status = BatchJobStatus::Cancelled;
                job.stage = None;
            }
        });
        if let Some(job) = job {
            emit_job_progress(app_handle, batch_id, &job);
        }
    }
}

fn emit_job_progress(
    app_handle: &AppHandle,
    batch_id: &str,
    job: &crate::indexer::batch::BatchJob,
) {
    let payload = BatchJobProgressPayload {
        batch_id: batch_id.to_string(),
        job_index: job.index,
        path: job.path.clone(),
        status: job.status,
        stage: job.stage.clone(),
        processed: job.processed,
        total: job.total,
        current_file: job.current_file.clone(),
        indexed_count: job.indexed_count,
        failed_count: job.failed_count,
        error: job.error.clone(),
    };
    let _ = app_handle.emit("indexing-batch-job-progress", &payload);
}

/// 배치 job 결과: (indexed_count, failed_count, was_cancelled)
type JobOutcome = (usize, usize, bool);

/// 단일 폴더 인덱싱 실행 (배치 전용)
///
/// 기존 `add_folder` 커맨드의 핵심 로직을 배치용으로 슬림화:
/// - pause/resume은 배치 루프에서 1회만 처리
/// - 벡터 자동 시작 없음 (드라이브 인덱싱은 수동 모드)
/// - 결과는 Result<(indexed, failed), String>로 반환
async fn run_folder_index_job_batch(
    state: &State<'_, RwLock<AppContainer>>,
    path: &str,
    controller: &crate::indexer::batch::BatchController,
    batch_id: &str,
    job_index: usize,
    app_handle: &AppHandle,
) -> Result<JobOutcome, String> {
    let ctx = extract_indexing_context(state).map_err(|e| e.to_string())?;
    let folder_path = Path::new(path);
    // UNC 보존: dunce 로 통일 (add_folder 와 동일 규칙).
    let canonical_path = dunce::canonicalize(folder_path)
        .map_err(|e| format!("canonicalize failed: {}", e))?;
    let path_str = canonical_path.to_string_lossy().to_string();

    // watch folder 등록 + 인덱싱 상태 마킹
    ctx.service
        .add_watched_folder(&path_str)
        .map_err(|e| e.to_string())?;
    if let Ok(conn) = crate::db::get_connection(&ctx.db_path) {
        let _ = crate::db::set_folder_indexing_status(&conn, &path_str, "indexing");
    }

    // 메타데이터 스캔 (파일명 즉시 검색 가능)
    let _ = ctx
        .service
        .scan_metadata_only(
            &canonical_path,
            ctx.include_subfolders,
            None,
            ctx.max_file_size_mb,
            ctx.exclude_dirs.clone(),
        )
        .await;

    // 스캔 도중 취소된 경우 FTS 스킵 (index_folder_fts는 내부 cancel_flag를 리셋하므로 별도 체크 필요)
    if controller.is_cancelled() {
        if let Ok(conn) = crate::db::get_connection(&ctx.db_path) {
            let _ = crate::db::set_folder_indexing_status(&conn, &path_str, "cancelled");
        }
        return Ok((0, 0, true));
    }

    // FTS 인덱싱 - 진행률 콜백이 batch_controller와 이벤트 양쪽 업데이트
    let progress_callback = create_batch_fts_progress_callback(
        app_handle.clone(),
        controller_clone_handle(state),
        batch_id.to_string(),
        job_index,
    );

    let result = ctx
        .service
        .index_folder_fts(
            &canonical_path,
            ctx.include_subfolders,
            Some(progress_callback),
            ctx.max_file_size_mb,
            ctx.exclude_dirs.clone(),
        )
        .await;

    refresh_filename_cache(state);

    match result {
        Ok(r) => {
            let was_cancelled = r.was_cancelled;
            if let Ok(conn) = crate::db::get_connection(&ctx.db_path) {
                let status = if was_cancelled {
                    "cancelled"
                } else {
                    "completed"
                };
                let _ = crate::db::set_folder_indexing_status(&conn, &path_str, status);
                if !was_cancelled {
                    let _ = crate::db::update_last_synced_at(&conn, &path_str);
                }
            }
            log_indexing_errors(&r.errors);
            Ok((r.indexed_count, r.failed_count, was_cancelled))
        }
        Err(e) => {
            if let Ok(conn) = crate::db::get_connection(&ctx.db_path) {
                let _ = crate::db::set_folder_indexing_status(&conn, &path_str, "failed");
            }
            Err(e.to_string())
        }
    }
}

fn controller_clone_handle(
    state: &State<'_, RwLock<AppContainer>>,
) -> std::sync::Arc<crate::indexer::batch::BatchController> {
    state
        .read()
        .map(|c| c.get_batch_controller())
        .unwrap_or_else(|_| std::sync::Arc::new(crate::indexer::batch::BatchController::new()))
}

fn create_batch_fts_progress_callback(
    app_handle: AppHandle,
    controller: std::sync::Arc<crate::indexer::batch::BatchController>,
    batch_id: String,
    job_index: usize,
) -> Box<dyn Fn(FtsIndexingProgress) + Send + Sync> {
    Box::new(move |progress: FtsIndexingProgress| {
        // 1. batch_controller의 job 상태 업데이트
        let stage = match progress.phase.as_str() {
            "preparing" | "scanning" => "scanning",
            "parsing" => "parsing",
            "indexing" => "indexing",
            "completed" => "cache_refresh",
            _ => "indexing",
        };
        let updated = controller.update_job(job_index, |job| {
            job.status = BatchJobStatus::Running;
            job.stage = Some(stage.to_string());
            job.processed = progress.processed_files;
            job.total = progress.total_files;
            job.current_file = progress.current_file.clone();
        });
        if let Some(job) = updated {
            emit_job_progress(&app_handle, &batch_id, &job);
        }
    })
}
