//! Index Commands - Thin Layer (Clean Architecture)
//!
//! Tauri commands that delegate to IndexService and FolderService.

use crate::application::dto::indexing::{AddFolderResult, FolderStats, IndexStatus, WatchedFolderInfo};
use crate::error::{ApiError, ApiResult};
use crate::indexer::pipeline::FtsIndexingProgress;
use crate::indexer::vector_worker::{VectorIndexingProgress, VectorIndexingStatus};
use crate::AppContainer;
use super::settings::VectorIndexingMode;
use serde::Serialize;
use std::path::Path;
use std::sync::{Arc, RwLock};
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
    state: State<'_, RwLock<AppContainer>>,
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

    // 설정 및 서비스 준비 (단일 lock 스코프에서 필요한 데이터 전부 추출)
    let (service, include_subfolders, semantic_available, vector_mode, semantic_enabled, intensity, max_file_size_mb, db_path) = {
        let container = state.read()?;
        let settings = container.get_settings();
        (
            container.index_service(),
            settings.include_subfolders,
            container.is_semantic_available(),
            settings.vector_indexing_mode.clone(),
            settings.semantic_search_enabled,
            settings.indexing_intensity.clone(),
            settings.max_file_size_mb,
            container.db_path.clone(),
        )
    };

    // 이미 등록된 폴더면 인덱싱 스킵
    if let Ok(conn) = crate::db::get_connection(&db_path) {
        if crate::db::is_folder_watched(&conn, &path).unwrap_or(false) {
            tracing::info!("Folder already watched, skipping: {}", path);
            return Ok(AddFolderResult {
                success: true,
                indexed_count: 0,
                failed_count: 0,
                vectors_count: 0,
                message: "이미 등록된 폴더입니다. 재인덱싱하려면 '다시 인덱싱' 버튼을 사용하세요.".to_string(),
                errors: vec![],
            });
        }
    }

    // 1. 감시 폴더 등록
    service.add_watched_folder(&path).map_err(ApiError::from)?;

    // 인덱싱 상태를 'indexing'으로 설정 (db_path로 직접 접근, 재잠금 불필요)
    if let Ok(conn) = crate::db::get_connection(&db_path) {
        let _ = crate::db::set_folder_indexing_status(&conn, &path, "indexing");
    }

    // UI에 준비 중 상태 알림 (메타데이터 스캔 전)
    let _ = app_handle.emit("indexing-progress", &IndexingProgress {
        phase: "preparing".to_string(),
        total_files: 0,
        processed_files: 0,
        current_file: None,
        folder_path: path.clone(),
        error: None,
    });

    // 2. 메타데이터 스캔 (파일명 검색 즉시 가능)
    let metadata_result = service
        .scan_metadata_only(&canonical_path, include_subfolders, None, max_file_size_mb)
        .await;

    // 3. FilenameCache 즉시 갱신 + 메타 스캔에서 수집한 파일 목록 재사용
    let pre_collected = if let Ok(ref meta) = metadata_result {
        refresh_filename_cache(&state);
        tracing::info!("FilenameCache ready: {} files (metadata scan)", meta.files_found);
        Some(meta.file_paths.clone())
    } else {
        None
    };

    // 4. FTS 인덱싱 (메타 스캔에서 수집한 파일 목록 재사용 → 이중 FS 순회 방지)
    let progress_callback = create_fts_progress_callback(app_handle.clone());
    let result = match service
        .index_folder_fts(&canonical_path, include_subfolders, Some(progress_callback), max_file_size_mb, pre_collected)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            // 실패 시 폴더 상태를 "failed"로 복구
            if let Ok(conn) = crate::db::get_connection(&db_path) {
                let _ = crate::db::set_folder_indexing_status(&conn, &path, "failed");
            }
            return Err(ApiError::from(e));
        }
    };

    // 5. 파일 감시 시작
    start_file_watching(&state, &canonical_path)?;

    // 6. FilenameCache 최종 갱신 (FTS 인덱싱 후)
    refresh_filename_cache(&state);

    // 취소 여부 먼저 확인 후 상태 결정
    let was_cancelled = result.errors.iter().any(|e| e.contains("Cancelled"));

    // 인덱싱 상태 업데이트: 취소 시 "cancelled" 유지 → 자동 재개 대상
    if let Ok(conn) = crate::db::get_connection(&db_path) {
        let status = if was_cancelled { "cancelled" } else { "completed" };
        let _ = crate::db::set_folder_indexing_status(&conn, &path, status);
        if !was_cancelled {
            let _ = crate::db::update_last_synced_at(&conn, &path);
        }
    }

    // 5. 벡터 인덱싱 (백그라운드) — 자동 모드 + 시맨틱 활성화일 때만
    let auto_vector = semantic_enabled
        && semantic_available
        && vector_mode == VectorIndexingMode::Auto
        && !was_cancelled
        && result.indexed_count > 0;
    if auto_vector {
        let vector_callback = create_vector_progress_callback(app_handle);
        let _ = service.start_vector_indexing(Some(vector_callback), Some(intensity));
    }

    // 결과 메시지 생성
    let message = build_result_message(&result, was_cancelled, semantic_available && semantic_enabled, false);
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
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<()> {
    tracing::info!("Removing folder from watch: {}", path);

    // 파일 감시 중지
    stop_file_watching(&state, Path::new(&path))?;

    // FolderService로 DB/벡터 삭제 위임
    let service = {
        let container = state.read()?;
        container.folder_service()
    };

    let result = service.remove_folder(&path).await.map_err(ApiError::from);

    // FilenameCache 갱신
    refresh_filename_cache(&state);

    result
}

/// 폴더 재인덱싱
#[tauri::command]
pub async fn reindex_folder(
    path: String,
    app_handle: AppHandle,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<AddFolderResult> {
    tracing::info!("Reindexing folder: {}", path);

    let folder_path = Path::new(&path);
    if !folder_path.exists() {
        return Err(ApiError::PathNotFound(path));
    }

    let canonical_path = folder_path
        .canonicalize()
        .map_err(|e| ApiError::InvalidPath(format!("'{}': {}", path, e)))?;

    let (service, include_subfolders, semantic_available, vector_mode, semantic_enabled, intensity, max_file_size_mb, db_path) = {
        let container = state.read()?;
        let settings = container.get_settings();
        (
            container.index_service(),
            settings.include_subfolders,
            container.is_semantic_available(),
            settings.vector_indexing_mode.clone(),
            settings.semantic_search_enabled,
            settings.indexing_intensity.clone(),
            settings.max_file_size_mb,
            container.db_path.clone(),
        )
    };

    // 인덱싱 상태를 'indexing'으로 설정
    let path_str = canonical_path.to_string_lossy().to_string();
    if let Ok(conn) = crate::db::get_connection(&db_path) {
        let _ = crate::db::set_folder_indexing_status(&conn, &path_str, "indexing");
    }

    // IndexService로 재인덱싱 위임
    let progress_callback = create_fts_progress_callback(app_handle.clone());
    let result = match service
        .reindex_folder(&canonical_path, include_subfolders, Some(progress_callback), max_file_size_mb)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            if let Ok(conn) = crate::db::get_connection(&db_path) {
                let _ = crate::db::set_folder_indexing_status(&conn, &path_str, "failed");
            }
            return Err(ApiError::from(e));
        }
    };

    // FilenameCache 갱신
    refresh_filename_cache(&state);

    // 취소 여부 먼저 확인 후 상태 결정
    let was_cancelled = result.errors.iter().any(|e| e.contains("Cancelled"));

    // 인덱싱 상태 업데이트: 취소 시 "cancelled" 유지 → 자동 재개 대상
    if let Ok(conn) = crate::db::get_connection(&db_path) {
        let status = if was_cancelled { "cancelled" } else { "completed" };
        let _ = crate::db::set_folder_indexing_status(&conn, &path_str, status);
        if !was_cancelled {
            let _ = crate::db::update_last_synced_at(&conn, &path_str);
        }
    }
    let auto_vector = semantic_enabled
        && semantic_available
        && vector_mode == VectorIndexingMode::Auto
        && !was_cancelled
        && result.indexed_count > 0;
    if auto_vector {
        let vector_callback = create_vector_progress_callback(app_handle);
        let _ = service.start_vector_indexing(Some(vector_callback), Some(intensity));
    }

    let message = build_result_message(&result, was_cancelled, semantic_available && semantic_enabled, true);

    Ok(AddFolderResult {
        success: true,
        indexed_count: result.indexed_count,
        failed_count: result.failed_count,
        vectors_count: 0,
        message,
        errors: result.errors,
    })
}

/// 미완료 인덱싱 재개 (sync 기반: 추가/수정/삭제 감지)
#[tauri::command]
pub async fn resume_indexing(
    path: String,
    app_handle: AppHandle,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<AddFolderResult> {
    tracing::info!("Syncing folder (resume): {}", path);

    let folder_path = Path::new(&path);
    if !folder_path.exists() {
        return Err(ApiError::PathNotFound(path));
    }

    let canonical_path = folder_path
        .canonicalize()
        .map_err(|e| ApiError::InvalidPath(format!("'{}': {}", path, e)))?;

    let (service, include_subfolders, semantic_available, vector_mode, semantic_enabled, intensity, max_file_size_mb, db_path) = {
        let container = state.read()?;
        let settings = container.get_settings();
        (
            container.index_service(),
            settings.include_subfolders,
            container.is_semantic_available(),
            settings.vector_indexing_mode.clone(),
            settings.semantic_search_enabled,
            settings.indexing_intensity.clone(),
            settings.max_file_size_mb,
            container.db_path.clone(),
        )
    };

    // UI에 준비 중 상태 알림
    let path_str = canonical_path.to_string_lossy().to_string();
    let _ = app_handle.emit("indexing-progress", &IndexingProgress {
        phase: "preparing".to_string(),
        total_files: 0,
        processed_files: 0,
        current_file: None,
        folder_path: path_str.clone(),
        error: None,
    });

    // sync 기반 인덱싱 (추가/수정/삭제 감지)
    let progress_callback = create_fts_progress_callback(app_handle.clone());
    let sync_result = match service
        .sync_folder(&canonical_path, include_subfolders, Some(progress_callback), max_file_size_mb)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            if let Ok(conn) = crate::db::get_connection(&db_path) {
                let _ = crate::db::set_folder_indexing_status(&conn, &path_str, "failed");
            }
            return Err(ApiError::from(e));
        }
    };

    // FilenameCache 갱신
    refresh_filename_cache(&state);

    // 파일 감시 시작 (미완료 폴더는 아직 감시 안 했을 수 있음)
    let _ = start_file_watching(&state, &canonical_path);

    // 취소 여부 먼저 확인 후 상태 결정
    let was_cancelled = sync_result.errors.iter().any(|e| e.contains("Cancelled"));

    // 인덱싱 상태 업데이트: 취소 시 "cancelled" 유지 → 자동 재개 대상
    if let Ok(conn) = crate::db::get_connection(&db_path) {
        let status = if was_cancelled { "cancelled" } else { "completed" };
        let _ = crate::db::set_folder_indexing_status(&conn, &path_str, status);
        if !was_cancelled {
            let _ = crate::db::update_last_synced_at(&conn, &path_str);
        }
    }
    let indexed_count = sync_result.added + sync_result.modified;
    let auto_vector = semantic_enabled
        && semantic_available
        && vector_mode == VectorIndexingMode::Auto
        && !was_cancelled
        && indexed_count > 0;
    if auto_vector {
        let vector_callback = create_vector_progress_callback(app_handle);
        let _ = service.start_vector_indexing(Some(vector_callback), Some(intensity));
    }

    let message = format!(
        "동기화 완료: +{}개, -{}개, 변경없음 {}개{}",
        sync_result.added,
        sync_result.deleted,
        sync_result.unchanged,
        if sync_result.failed > 0 { format!(", {}개 실패", sync_result.failed) } else { String::new() }
    );

    Ok(AddFolderResult {
        success: true,
        indexed_count,
        failed_count: sync_result.failed,
        vectors_count: 0,
        message,
        errors: sync_result.errors,
    })
}

// ============================================
// Index Status Commands
// ============================================

/// 인덱스 상태 조회
#[tauri::command]
pub async fn get_index_status(state: State<'_, RwLock<AppContainer>>) -> ApiResult<IndexStatus> {
    let service = {
        let container = state.read()?;
        container.index_service()
    };
    service.get_status().await.map_err(ApiError::from)
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

    let (service, semantic_enabled, intensity) = {
        let container = state.read()?;
        let settings = container.get_settings();
        (container.index_service(), settings.semantic_search_enabled, settings.indexing_intensity.clone())
    };

    if !semantic_enabled {
        return Err(ApiError::SemanticSearchDisabled);
    }

    let vector_callback = create_vector_progress_callback(app_handle);
    service
        .start_vector_indexing(Some(vector_callback), Some(intensity))
        .map_err(ApiError::from)
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
pub async fn cancel_vector_indexing(
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<()> {
    tracing::info!("Cancelling vector indexing...");
    let service = {
        let container = state.read()?;
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
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<()> {
    tracing::info!("Clearing all data...");

    // 파일 감시 모두 중지
    {
        let container = state.read()?;
        if let Ok(wm) = container.get_watch_manager() {
            if let Ok(mut wm) = wm.write() {
                wm.unwatch_all();
                tracing::info!("All watchers stopped");
            }
        }
    }

    // 단일 lock 스코프에서 service와 filename_cache 추출 (worker.join()이 동기적 대기를 보장)
    let (service, filename_cache) = {
        let container = state.read()?;
        (container.index_service(), container.get_filename_cache())
    };
    let result = service.clear_all().map_err(ApiError::from);

    filename_cache.clear();
    tracing::info!("FilenameCache cleared");

    result
}

// ============================================
// Folder Info Commands (FolderService 위임)
// ============================================

/// 폴더별 인덱싱 통계 조회
#[tauri::command]
pub async fn get_folder_stats(
    path: String,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<FolderStats> {
    let service = {
        let container = state.read()?;
        container.folder_service()
    };
    service.get_folder_stats(&path).await.map_err(ApiError::from)
}

/// 감시 폴더 목록 조회
#[tauri::command]
pub async fn get_folders_with_info(
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<Vec<WatchedFolderInfo>> {
    let service = {
        let container = state.read()?;
        container.folder_service()
    };
    service.get_folders_with_info().await.map_err(ApiError::from)
}

/// 즐겨찾기 토글
#[tauri::command]
pub async fn toggle_favorite(
    path: String,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<bool> {
    let service = {
        let container = state.read()?;
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
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<DbDebugInfo> {
    // 프로덕션에서 디버그 커맨드 차단
    if !cfg!(debug_assertions) {
        return Err(ApiError::IndexingFailed("Debug command not available in release build".to_string()));
    }

    use crate::db;

    let db_path = {
        let container = state.read()?;
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

fn start_file_watching(state: &State<'_, RwLock<AppContainer>>, path: &Path) -> ApiResult<()> {
    let container = state.read()?;
    if let Ok(wm) = container.get_watch_manager() {
        if let Ok(mut wm) = wm.write() {
            if let Err(e) = wm.watch(path) {
                tracing::warn!("Failed to start watching {}: {}", path.display(), e);
            }
        }
    }
    Ok(())
}

fn stop_file_watching(state: &State<'_, RwLock<AppContainer>>, path: &Path) -> ApiResult<()> {
    let container = state.read()?;
    if let Ok(wm) = container.get_watch_manager() {
        if let Ok(mut wm) = wm.write() {
            let _ = wm.unwatch(path);
        }
    }
    Ok(())
}

/// FilenameCache 갱신 (인덱싱 완료 후 호출)
fn refresh_filename_cache(state: &State<'_, RwLock<AppContainer>>) {
    if let Ok(container) = state.read() {
        match container.load_filename_cache() {
            Ok(count) => tracing::info!("FilenameCache refreshed: {} entries", count),
            Err(e) => tracing::warn!("Failed to refresh FilenameCache: {}", e),
        }
    }
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
