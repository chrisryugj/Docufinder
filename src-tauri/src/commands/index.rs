//! Index Commands - Thin Layer (Clean Architecture)
//!
//! Tauri commands that delegate to IndexService and FolderService.

use crate::application::dto::indexing::{AddFolderResult, FolderStats, IndexStatus, WatchedFolderInfo};
use crate::error::{ApiError, ApiResult};
use crate::indexer::pipeline::FtsIndexingProgress;
use crate::indexer::vector_worker::{VectorIndexingProgress, VectorIndexingStatus};
use crate::AppContainer;
use super::settings::get_settings_sync;
use serde::Serialize;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};

/// 프론트엔드 이벤트용 인덱싱 진행률
#[derive(Debug, Clone, Serialize)]
struct IndexingProgress {
    phase: String,
    total_files: usize,
    processed_files: usize,
    current_file: Option<String>,
    folder_path: String,
    error: Option<String>,
}

// ============================================
// FTS Progress Callback Helper
// ============================================

fn create_fts_progress_callback(app_handle: AppHandle) -> Box<dyn Fn(FtsIndexingProgress) + Send + Sync> {
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

fn create_vector_progress_callback(app_handle: AppHandle) -> Arc<dyn Fn(VectorIndexingProgress) + Send + Sync> {
    Arc::new(move |progress: VectorIndexingProgress| {
        if let Err(e) = app_handle.emit("vector-indexing-progress", &progress) {
            tracing::warn!("Failed to emit vector progress: {}", e);
        }
    })
}

// ============================================
// Folder Commands
// ============================================

/// 감시 폴더 추가 및 인덱싱 (2단계: FTS → 벡터 백그라운드)
#[tauri::command]
pub async fn add_folder(
    path: String,
    app_handle: AppHandle,
    state: State<'_, Mutex<AppContainer>>,
) -> ApiResult<AddFolderResult> {
    tracing::info!("Adding folder to watch: {}", path);

    // 경로 존재 확인
    let folder_path = Path::new(&path);
    if !folder_path.exists() {
        return Err(ApiError::PathNotFound(path));
    }

    // 경로 정규화
    let canonical_path = folder_path
        .canonicalize()
        .map_err(|e| ApiError::InvalidPath(format!("'{}': {}", path, e)))?;
    let path = canonical_path.to_string_lossy().to_string();

    // 설정 및 서비스 준비
    let (service, include_subfolders, semantic_available) = {
        let container = state.lock()?;
        let app_data_dir = container.db_path.parent().map(|p| p.to_path_buf());
        let include_subfolders = app_data_dir
            .map(|dir| get_settings_sync(&dir).include_subfolders)
            .unwrap_or(true);
        (container.index_service(), include_subfolders, container.is_semantic_available())
    };

    // 1. 감시 폴더 등록
    service.add_watched_folder(&path).map_err(ApiError::from)?;

    // 2. FTS 인덱싱
    let progress_callback = create_fts_progress_callback(app_handle.clone());
    let result = service
        .index_folder_fts(&canonical_path, include_subfolders, Some(progress_callback))
        .await
        .map_err(ApiError::from)?;

    // 3. 파일 감시 시작
    start_file_watching(&state, &canonical_path)?;

    // 4. 벡터 인덱싱 (백그라운드)
    let was_cancelled = result.errors.iter().any(|e| e.contains("Cancelled"));
    if semantic_available && !was_cancelled && result.indexed_count > 0 {
        let vector_callback = create_vector_progress_callback(app_handle);
        let _ = service.start_vector_indexing(Some(vector_callback));
    }

    // 결과 메시지 생성
    let message = build_result_message(&result, was_cancelled, semantic_available, false);
    log_indexing_errors(&result.errors);

    Ok(AddFolderResult {
        success: true,
        indexed_count: result.indexed_count,
        failed_count: result.failed_count,
        vectors_count: 0,
        message,
        errors: result.errors,
    })
}

/// 감시 폴더 제거
#[tauri::command]
pub async fn remove_folder(
    path: String,
    state: State<'_, Mutex<AppContainer>>,
) -> ApiResult<()> {
    tracing::info!("Removing folder from watch: {}", path);

    // 파일 감시 중지
    stop_file_watching(&state, Path::new(&path))?;

    // FolderService로 DB/벡터 삭제 위임
    let service = {
        let container = state.lock()?;
        container.folder_service()
    };

    service.remove_folder(&path).await.map_err(ApiError::from)
}

/// 폴더 재인덱싱
#[tauri::command]
pub async fn reindex_folder(
    path: String,
    app_handle: AppHandle,
    state: State<'_, Mutex<AppContainer>>,
) -> ApiResult<AddFolderResult> {
    tracing::info!("Reindexing folder: {}", path);

    let folder_path = Path::new(&path);
    if !folder_path.exists() {
        return Err(ApiError::PathNotFound(path));
    }

    let canonical_path = folder_path
        .canonicalize()
        .map_err(|e| ApiError::InvalidPath(format!("'{}': {}", path, e)))?;

    let (service, include_subfolders, semantic_available) = {
        let container = state.lock()?;
        let app_data_dir = container.db_path.parent().map(|p| p.to_path_buf());
        let include_subfolders = app_data_dir
            .map(|dir| get_settings_sync(&dir).include_subfolders)
            .unwrap_or(true);
        (container.index_service(), include_subfolders, container.is_semantic_available())
    };

    // IndexService로 재인덱싱 위임
    let progress_callback = create_fts_progress_callback(app_handle.clone());
    let result = service
        .reindex_folder(&canonical_path, include_subfolders, Some(progress_callback))
        .await
        .map_err(ApiError::from)?;

    // 벡터 인덱싱 (백그라운드)
    let was_cancelled = result.errors.iter().any(|e| e.contains("Cancelled"));
    if semantic_available && !was_cancelled && result.indexed_count > 0 {
        let vector_callback = create_vector_progress_callback(app_handle);
        let _ = service.start_vector_indexing(Some(vector_callback));
    }

    let message = build_result_message(&result, was_cancelled, semantic_available, true);

    Ok(AddFolderResult {
        success: true,
        indexed_count: result.indexed_count,
        failed_count: result.failed_count,
        vectors_count: 0,
        message,
        errors: result.errors,
    })
}

// ============================================
// Index Status Commands
// ============================================

/// 인덱스 상태 조회
#[tauri::command]
pub async fn get_index_status(state: State<'_, Mutex<AppContainer>>) -> ApiResult<IndexStatus> {
    let service = {
        let container = state.lock()?;
        container.index_service()
    };
    service.get_status().await.map_err(ApiError::from)
}

/// 벡터 인덱싱 상태 조회
#[tauri::command]
pub async fn get_vector_indexing_status(
    state: State<'_, Mutex<AppContainer>>,
) -> ApiResult<VectorIndexingStatus> {
    let service = {
        let container = state.lock()?;
        container.index_service()
    };
    service.get_vector_status().map_err(ApiError::from)
}

// ============================================
// Cancel Commands
// ============================================

/// 인덱싱 취소
#[tauri::command]
pub async fn cancel_indexing(state: State<'_, Mutex<AppContainer>>) -> ApiResult<()> {
    tracing::info!("Cancelling indexing...");
    let service = {
        let container = state.lock()?;
        container.index_service()
    };
    service.cancel_indexing();
    Ok(())
}

/// 벡터 인덱싱 취소
#[tauri::command]
pub async fn cancel_vector_indexing(
    state: State<'_, Mutex<AppContainer>>,
) -> ApiResult<()> {
    tracing::info!("Cancelling vector indexing...");
    let service = {
        let container = state.lock()?;
        container.index_service()
    };
    service.cancel_vector_indexing().map_err(ApiError::from)
}

// ============================================
// Data Management Commands
// ============================================

/// 모든 데이터 초기화
#[tauri::command]
pub async fn clear_all_data(
    state: State<'_, Mutex<AppContainer>>,
) -> ApiResult<()> {
    tracing::info!("Clearing all data...");

    // 파일 감시 모두 중지
    {
        let container = state.lock()?;
        if let Ok(wm) = container.get_watch_manager() {
            if let Ok(mut wm) = wm.write() {
                wm.unwatch_all();
                tracing::info!("All watchers stopped");
            }
        }
    }

    // 잠시 대기 후 IndexService로 클리어
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let service = {
        let container = state.lock()?;
        container.index_service()
    };
    service.clear_all().map_err(ApiError::from)
}

// ============================================
// Folder Info Commands (FolderService 위임)
// ============================================

/// 폴더별 인덱싱 통계 조회
#[tauri::command]
pub async fn get_folder_stats(
    path: String,
    state: State<'_, Mutex<AppContainer>>,
) -> ApiResult<FolderStats> {
    let service = {
        let container = state.lock()?;
        container.folder_service()
    };
    service.get_folder_stats(&path).await.map_err(ApiError::from)
}

/// 감시 폴더 목록 조회
#[tauri::command]
pub async fn get_folders_with_info(
    state: State<'_, Mutex<AppContainer>>,
) -> ApiResult<Vec<WatchedFolderInfo>> {
    let service = {
        let container = state.lock()?;
        container.folder_service()
    };
    service.get_folders_with_info().await.map_err(ApiError::from)
}

/// 즐겨찾기 토글
#[tauri::command]
pub async fn toggle_favorite(
    path: String,
    state: State<'_, Mutex<AppContainer>>,
) -> ApiResult<bool> {
    let service = {
        let container = state.lock()?;
        container.folder_service()
    };
    service.toggle_favorite(&path).await.map_err(ApiError::from)
}

// ============================================
// Debug Commands
// ============================================

#[derive(Debug, Serialize)]
pub struct DbDebugInfo {
    pub files_count: usize,
    pub chunks_count: usize,
    pub chunks_fts_count: usize,
    pub files_fts_count: usize,
    pub fts_match_count: usize,
    pub filename_match_count: usize,
    pub test_query: String,
}

#[tauri::command]
pub async fn get_db_debug_info(
    query: String,
    state: State<'_, Mutex<AppContainer>>,
) -> ApiResult<DbDebugInfo> {
    use crate::db;

    let db_path = {
        let container = state.lock()?;
        container.db_path.clone()
    };

    let conn = db::get_connection(&db_path)
        .map_err(|e| ApiError::DatabaseConnection(e.to_string()))?;

    let files_count: usize = conn.query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0)).unwrap_or(0);
    let chunks_count: usize = conn.query_row("SELECT COUNT(*) FROM chunks", [], |r| r.get(0)).unwrap_or(0);
    let chunks_fts_count: usize = conn.query_row("SELECT COUNT(*) FROM chunks_fts", [], |r| r.get(0)).unwrap_or(0);
    let files_fts_count: usize = conn.query_row("SELECT COUNT(*) FROM files_fts", [], |r| r.get(0)).unwrap_or(0);

    let safe_query = format!("\"{}\"*", query.replace('"', "\"\""));
    let fts_match_count: usize = conn
        .query_row("SELECT COUNT(*) FROM chunks_fts WHERE chunks_fts MATCH ?", [&safe_query], |r| r.get(0))
        .unwrap_or(0);
    let filename_match_count: usize = conn
        .query_row("SELECT COUNT(*) FROM files_fts WHERE files_fts MATCH ?", [&safe_query], |r| r.get(0))
        .unwrap_or(0);

    tracing::info!(
        "DB Debug: files={}, chunks={}, chunks_fts={}, files_fts={}, content_match('{}')={}, filename_match={}",
        files_count, chunks_count, chunks_fts_count, files_fts_count, query, fts_match_count, filename_match_count
    );

    Ok(DbDebugInfo {
        files_count,
        chunks_count,
        chunks_fts_count,
        files_fts_count,
        fts_match_count,
        filename_match_count,
        test_query: safe_query,
    })
}

// ============================================
// Private Helpers
// ============================================

fn start_file_watching(state: &State<'_, Mutex<AppContainer>>, path: &Path) -> ApiResult<()> {
    let container = state.lock()?;
    if let Ok(wm) = container.get_watch_manager() {
        if let Ok(mut wm) = wm.write() {
            if let Err(e) = wm.watch(path) {
                tracing::warn!("Failed to start watching {}: {}", path.display(), e);
            }
        }
    }
    Ok(())
}

fn stop_file_watching(state: &State<'_, Mutex<AppContainer>>, path: &Path) -> ApiResult<()> {
    let container = state.lock()?;
    if let Ok(wm) = container.get_watch_manager() {
        if let Ok(mut wm) = wm.write() {
            let _ = wm.unwatch(path);
        }
    }
    Ok(())
}

fn build_result_message(
    result: &crate::indexer::pipeline::FolderIndexResult,
    was_cancelled: bool,
    semantic_available: bool,
    is_reindex: bool,
) -> String {
    let action = if is_reindex { "재인덱싱" } else { "인덱싱" };

    if was_cancelled {
        format!("{}이 취소되었습니다", action)
    } else if result.failed_count > 0 {
        format!(
            "{} 파일 {} 완료, {} 실패{}",
            result.indexed_count,
            action,
            result.failed_count,
            if semantic_available { " (시맨틱 검색 준비 중...)" } else { "" }
        )
    } else if semantic_available {
        format!("{} 파일 {} 완료 (시맨틱 검색 준비 중...)", result.indexed_count, action)
    } else {
        format!("{} 파일 {} 완료", result.indexed_count, action)
    }
}

fn log_indexing_errors(errors: &[String]) {
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
