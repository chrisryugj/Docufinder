//! Index Commands - Thin Layer (Clean Architecture)
//!
//! Tauri commands that delegate to IndexService and FolderService.

mod data;
mod folder;
mod init;

pub use data::*;
pub use folder::*;
pub use init::*;

use super::settings::VectorIndexingMode;
use crate::application::dto::indexing::{
    AddFolderResult, ConvertHwpResult, FolderStats, IndexStatus, WatchedFolderInfo,
};
use crate::error::{ApiError, ApiResult};
use crate::indexer::pipeline::FtsIndexingProgress;
use crate::indexer::vector_worker::{VectorIndexingProgress, VectorIndexingStatus};
use crate::AppContainer;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tauri::{AppHandle, Emitter, Manager, State};

/// 인덱싱 명령에서 공통으로 필요한 설정/서비스 번들
pub(super) struct IndexingContext {
    pub(super) service: crate::application::services::IndexService,
    pub(super) include_subfolders: bool,
    pub(super) semantic_available: bool,
    pub(super) vector_mode: VectorIndexingMode,
    pub(super) semantic_enabled: bool,
    pub(super) intensity: super::settings::IndexingIntensity,
    pub(super) max_file_size_mb: u64,
    pub(super) db_path: PathBuf,
    pub(super) exclude_dirs: Vec<String>,
}

/// 단일 lock 스코프에서 인덱싱에 필요한 모든 설정/서비스를 추출
pub(super) fn extract_indexing_context(state: &State<'_, RwLock<AppContainer>>) -> ApiResult<IndexingContext> {
    let container = state.read()?;
    let settings = container.get_settings();
    let mut dirs: Vec<String> = crate::constants::DEFAULT_EXCLUDED_DIRS
        .iter()
        .map(|s| s.to_string())
        .collect();
    dirs.extend(settings.exclude_dirs.clone());
    Ok(IndexingContext {
        service: container.index_service(),
        include_subfolders: settings.include_subfolders,
        semantic_available: container.is_semantic_available(),
        vector_mode: settings.vector_indexing_mode.clone(),
        semantic_enabled: settings.semantic_search_enabled,
        intensity: settings.indexing_intensity.clone(),
        max_file_size_mb: settings.max_file_size_mb,
        db_path: container.db_path.clone(),
        exclude_dirs: dirs,
    })
}

pub(super) fn should_auto_vector(
    ctx: &IndexingContext,
    was_cancelled: bool,
    indexed_count: usize,
    skip_drive_root: bool,
) -> bool {
    ctx.semantic_enabled
        && ctx.semantic_available
        && ctx.vector_mode == VectorIndexingMode::Auto
        && !was_cancelled
        && indexed_count > 0
        && !skip_drive_root
}

/// 인덱싱 완료 후 벡터 자동 시작 여부 판단 + 실행
pub(super) fn maybe_start_auto_vector(
    ctx: &IndexingContext,
    app_handle: AppHandle,
    was_cancelled: bool,
    indexed_count: usize,
    skip_drive_root: bool,
    state: Option<&State<'_, RwLock<AppContainer>>>,
) -> bool {
    if !should_auto_vector(ctx, was_cancelled, indexed_count, skip_drive_root) {
        return false;
    }

    // 이미 paused 상태가 아니면 여기서 먼저 멈춘다.
    if let Some(s) = state {
        pause_watching(s);
    }

    let vector_callback = create_vector_progress_callback(app_handle, true);
    match ctx
        .service
        .start_vector_indexing(Some(vector_callback), Some(ctx.intensity.clone()))
    {
        Ok(true) => true,
        Ok(false) | Err(_) => {
            if let Some(s) = state {
                resume_watching(s, &ctx.db_path);
            }
            false
        }
    }
}

/// 파일 감시 일시 중지 (인덱싱 중 DB 동시 접근 방지)
pub(super) fn pause_watching(state: &State<'_, RwLock<AppContainer>>) {
    if let Ok(container) = state.read() {
        if let Ok(wm) = container.get_watch_manager() {
            if let Ok(mut wm) = wm.write() {
                wm.pause();
            }
        }
    }
}

/// 파일 감시 재개 (DB의 watched_folders 목록으로 전체 재등록)
/// 재개 전 WAL checkpoint 수행 (대량 인덱싱 후 WAL 파일 크기 관리)
pub(super) fn resume_watching(state: &State<'_, RwLock<AppContainer>>, db_path: &std::path::PathBuf) {
    crate::db::wal_checkpoint(db_path);
    if let Ok(container) = state.read() {
        if let Ok(wm) = container.get_watch_manager() {
            if let Ok(mut wm) = wm.write() {
                if let Ok(conn) = crate::db::get_connection(db_path) {
                    if let Ok(folders) = crate::db::get_watched_folders(&conn) {
                        let existing_folders: Vec<String> = folders
                            .into_iter()
                            .filter(|folder| Path::new(folder).exists())
                            .collect();
                        wm.resume_with_folders(&existing_folders);
                    }
                }
            }
        }
    }
}

/// 프론트엔드 이벤트용 인덱싱 진행률
#[derive(Debug, Clone, Serialize)]
pub(super) struct IndexingProgress {
    pub(super) phase: String,
    pub(super) total_files: usize,
    pub(super) processed_files: usize,
    pub(super) current_file: Option<String>,
    pub(super) folder_path: String,
    pub(super) error: Option<String>,
}

// ============================================
// FTS Progress Callback Helper
// ============================================

pub(super) fn create_fts_progress_callback(
    app_handle: AppHandle,
) -> Box<dyn Fn(FtsIndexingProgress) + Send + Sync> {
    Box::new(move |progress: FtsIndexingProgress| {
        let legacy_progress = IndexingProgress {
            phase: progress.phase,
            total_files: progress.total_files,
            processed_files: progress.processed_files,
            current_file: progress.current_file,
            folder_path: progress.folder_path,
            error: None,
        };
        if let Err(e) = app_handle.emit("indexing-progress", &legacy_progress) {
            tracing::warn!("Failed to emit progress: {}", e);
        }
    })
}

pub(super) fn create_vector_progress_callback(
    app_handle: AppHandle,
    resume_on_complete: bool,
) -> Arc<dyn Fn(VectorIndexingProgress) + Send + Sync> {
    Arc::new(move |progress: VectorIndexingProgress| {
        if let Err(e) = app_handle.emit("vector-indexing-progress", &progress) {
            tracing::warn!("Failed to emit vector progress: {}", e);
        }
        // 벡터 완료 시 파일 감시 재개 (auto 모드에서 pause_watching 호출된 경우)
        if progress.is_complete && resume_on_complete {
            let state = app_handle.state::<RwLock<AppContainer>>();
            let db_path = state.read().ok().map(|c| c.db_path.clone());
            if let Some(db_path) = db_path {
                resume_watching(&state, &db_path);
            }
        }
    })
}

pub(super) fn stop_file_watching(state: &State<'_, RwLock<AppContainer>>, path: &Path) -> ApiResult<()> {
    let container = state.read()?;
    if let Ok(wm) = container.get_watch_manager() {
        if let Ok(mut wm) = wm.write() {
            let _ = wm.unwatch(path);
        }
    }
    Ok(())
}

/// FilenameCache 갱신 (인덱싱 완료 후 호출)
pub(super) fn refresh_filename_cache(state: &State<'_, RwLock<AppContainer>>) {
    if let Ok(container) = state.read() {
        match container.load_filename_cache() {
            Ok(count) => tracing::info!("FilenameCache refreshed: {} entries", count),
            Err(e) => tracing::warn!("Failed to refresh FilenameCache: {}", e),
        }
    }
}

pub(super) fn build_result_message(
    result: &crate::indexer::pipeline::FolderIndexResult,
    was_cancelled: bool,
    semantic_available: bool,
    is_reindex: bool,
) -> String {
    let action = if is_reindex {
        "재인덱싱"
    } else {
        "인덱싱"
    };

    if was_cancelled {
        format!("{}이 취소되었습니다", action)
    } else if result.failed_count > 0 {
        format!(
            "{} 파일 {} 완료, {} 실패{}",
            result.indexed_count,
            action,
            result.failed_count,
            if semantic_available {
                " (시맨틱 검색 준비 중...)"
            } else {
                ""
            }
        )
    } else if semantic_available {
        format!(
            "{} 파일 {} 완료 (시맨틱 검색 준비 중...)",
            result.indexed_count, action
        )
    } else {
        format!("{} 파일 {} 완료", result.indexed_count, action)
    }
}

pub(super) fn log_indexing_errors(errors: &[String]) {
    if !errors.is_empty() {
        tracing::warn!("Indexing errors ({}):", errors.len());
        for (i, err) in errors.iter().take(10).enumerate() {
            tracing::warn!("  {}: {}", i + 1, err);
        }
        if errors.len() > 10 {
            tracing::warn!("  ... and {} more errors", errors.len() - 10);
        }
    }
}

// ============================================
// Index Status Commands
// ============================================

/// 인덱스 상태 조회
#[tauri::command]
pub async fn get_index_status(state: State<'_, RwLock<AppContainer>>) -> ApiResult<IndexStatus> {
    let (service, model_available, filename_cache) = {
        let container = state.read()?;
        (
            container.index_service(),
            container.is_semantic_available(),
            container.get_filename_cache(),
        )
    };
    let mut status = service.get_status().await.map_err(ApiError::from)?;
    // OnceCell 초기화 여부가 아닌 모델 파일 존재 여부로 판단
    status.semantic_available = model_available;
    status.filename_cache_truncated = filename_cache.is_truncated();
    Ok(status)
}

/// 벡터 인덱싱 상태 조회
#[tauri::command]
pub async fn get_vector_indexing_status(
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<VectorIndexingStatus> {
    let service = {
        let container = state.read()?;
        container.index_service()
    };
    service.get_vector_status().map_err(ApiError::from)
}

// ============================================
// Manual Vector Indexing
// ============================================

/// 수동 벡터 인덱싱 시작
#[tauri::command]
pub async fn start_vector_indexing(
    app_handle: AppHandle,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<()> {
    tracing::info!("Manual vector indexing requested");

    let (service, semantic_enabled, intensity, db_path) = {
        let container = state.read()?;
        let settings = container.get_settings();
        (
            container.index_service(),
            settings.semantic_search_enabled,
            settings.indexing_intensity.clone(),
            container.db_path.clone(),
        )
    };

    if !semantic_enabled {
        return Err(ApiError::SemanticSearchDisabled);
    }

    if service
        .get_vector_status()
        .map_err(ApiError::from)?
        .is_running
    {
        return Ok(());
    }

    pause_watching(&state);

    let vector_callback = create_vector_progress_callback(app_handle, true);
    match service.start_vector_indexing(Some(vector_callback), Some(intensity)) {
        Ok(true) => Ok(()),
        Ok(false) => {
            resume_watching(&state, &db_path);
            Ok(())
        }
        Err(e) => {
            resume_watching(&state, &db_path);
            Err(ApiError::from(e))
        }
    }
}

// ============================================
// Cancel Commands
// ============================================

/// 인덱싱 취소
#[tauri::command]
pub async fn cancel_indexing(state: State<'_, RwLock<AppContainer>>) -> ApiResult<()> {
    tracing::info!("Cancelling indexing...");
    let service = {
        let container = state.read()?;
        container.index_service()
    };
    service.cancel_indexing();
    Ok(())
}

/// 벡터 인덱싱 취소
#[tauri::command]
pub async fn cancel_vector_indexing(state: State<'_, RwLock<AppContainer>>) -> ApiResult<()> {
    tracing::info!("Cancelling vector indexing...");
    let service = {
        let container = state.read()?;
        container.index_service()
    };
    service.cancel_vector_indexing().map_err(ApiError::from)
}
