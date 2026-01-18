use crate::constants::BLOCKED_PATH_PATTERNS;
use crate::db;
use crate::error::{ApiError, ApiResult};
use crate::indexer::pipeline::{self, IndexingProgress};
use crate::AppState;
use super::settings::get_settings_sync;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};

/// 시스템 폴더 블랙리스트 검증 (Path Traversal 방지)
fn is_safe_path(path: &Path) -> bool {
    let path_str = path.to_string_lossy().to_lowercase();
    !BLOCKED_PATH_PATTERNS.iter().any(|b| path_str.contains(b))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IndexStatus {
    pub total_files: usize,
    pub indexed_files: usize,
    pub watched_folders: Vec<String>,
    pub vectors_count: usize,
    pub semantic_available: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddFolderResult {
    pub success: bool,
    pub indexed_count: usize,
    pub failed_count: usize,
    pub vectors_count: usize,
    pub message: String,
    pub errors: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FolderStats {
    pub file_count: usize,
    pub last_indexed: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WatchedFolderInfo {
    pub path: String,
    pub is_favorite: bool,
    pub added_at: Option<i64>,
}

/// 감시 폴더 추가 및 인덱싱 (진행률 이벤트 포함)
#[tauri::command]
pub async fn add_folder(
    path: String,
    app_handle: AppHandle,
    state: State<'_, Mutex<AppState>>,
) -> ApiResult<AddFolderResult> {
    tracing::info!("Adding folder to watch: {}", path);

    let folder_path = Path::new(&path);
    if !folder_path.exists() {
        return Err(ApiError::PathNotFound(path));
    }

    // 경로 정규화 (심볼릭 링크 해결, .. 제거) - 경로 트래버설 방지
    let canonical_path = folder_path
        .canonicalize()
        .map_err(|e| ApiError::InvalidPath(format!("'{}': {}", path, e)))?;

    // 시스템 폴더 블랙리스트 검증
    if !is_safe_path(&canonical_path) {
        return Err(ApiError::AccessDenied(format!(
            "'{}' is a protected system folder",
            canonical_path.display()
        )));
    }

    let folder_path = canonical_path.as_path();
    let path = canonical_path.to_string_lossy().to_string();

    let (db_path, embedder, vector_index, cancel_flag) = {
        let state = state.lock()?;
        // 새 인덱싱 시작 시 취소 플래그 리셋
        state.reset_cancel_flag();
        tracing::info!("Getting embedder and vector index...");
        let emb = state.get_embedder();
        tracing::info!("Embedder result: {:?}", emb.is_ok());
        let vi = state.get_vector_index();
        tracing::info!("Vector index result: {:?}", vi.is_ok());
        (
            state.db_path.clone(),
            emb.ok(),
            vi.ok(),
            state.get_cancel_flag(),
        )
    };

    let conn = db::get_connection(&db_path)
        .map_err(|e| ApiError::DatabaseConnection(e.to_string()))?;

    // 설정에서 하위폴더 포함 여부 확인
    let app_data_dir = db_path.parent().map(|p| p.to_path_buf());
    let include_subfolders = app_data_dir
        .map(|dir| get_settings_sync(&dir).include_subfolders)
        .unwrap_or(true);

    tracing::info!("Include subfolders: {}", include_subfolders);

    // 1. 감시 폴더 등록
    db::add_watched_folder(&conn, &path)?;

    // 2. 진행률 콜백 설정
    let app_handle_clone = app_handle.clone();
    let progress_callback: pipeline::ProgressCallback = Box::new(move |progress: IndexingProgress| {
        // Tauri 이벤트로 진행률 전송
        if let Err(e) = app_handle_clone.emit("indexing-progress", &progress) {
            tracing::warn!("Failed to emit progress: {}", e);
        }
    });

    // 3. 폴더 인덱싱 (진행률 + 취소 지원)
    let folder_path_buf = folder_path.to_path_buf();
    let result = tokio::task::spawn_blocking(move || {
        pipeline::index_folder_with_progress(
            &conn,
            &folder_path_buf,
            embedder.as_ref(),
            vector_index.as_ref(),
            include_subfolders,
            cancel_flag,
            Some(progress_callback),
        )
    })
    .await?
    .map_err(|e| ApiError::IndexingFailed(e.to_string()))?;

    // 4. 파일 감시 시작
    {
        let state = state.lock()?;
        if let Ok(wm) = state.get_watch_manager() {
            if let Ok(mut wm) = wm.write() {
                if let Err(e) = wm.watch(folder_path) {
                    tracing::warn!("Failed to start watching {}: {}", path, e);
                }
            }
        }
    }

    let message = if result.errors.iter().any(|e| e.contains("Cancelled")) {
        "인덱싱이 취소되었습니다".to_string()
    } else if result.failed_count > 0 {
        format!(
            "Indexed {} files ({} vectors), {} failed",
            result.indexed_count, result.vectors_count, result.failed_count
        )
    } else if result.vectors_count > 0 {
        format!(
            "Indexed {} files with {} vectors",
            result.indexed_count, result.vectors_count
        )
    } else {
        format!("Indexed {} files (semantic search disabled)", result.indexed_count)
    };

    // 에러 로그 출력 (디버깅용)
    if !result.errors.is_empty() {
        tracing::warn!("Indexing errors ({}):", result.errors.len());
        for (i, err) in result.errors.iter().take(10).enumerate() {
            tracing::warn!("  {}: {}", i + 1, err);
        }
        if result.errors.len() > 10 {
            tracing::warn!("  ... and {} more errors", result.errors.len() - 10);
        }
    }

    Ok(AddFolderResult {
        success: true,
        indexed_count: result.indexed_count,
        failed_count: result.failed_count,
        vectors_count: result.vectors_count,
        message,
        errors: result.errors,
    })
}

/// 인덱싱 취소
#[tauri::command]
pub async fn cancel_indexing(state: State<'_, Mutex<AppState>>) -> ApiResult<()> {
    tracing::info!("Cancelling indexing...");
    let state = state.lock()?;
    state.cancel_indexing();
    Ok(())
}

/// 감시 폴더 제거
#[tauri::command]
pub async fn remove_folder(
    path: String,
    state: State<'_, Mutex<AppState>>,
) -> ApiResult<()> {
    tracing::info!("Removing folder from watch: {}", path);

    let folder_path = Path::new(&path);

    // 1. 파일 감시 중지
    {
        let state = state.lock()?;
        if let Ok(wm) = state.get_watch_manager() {
            if let Ok(mut wm) = wm.write() {
                let _ = wm.unwatch(folder_path);
            }
        }
    }

    let (db_path, vector_index) = {
        let state = state.lock()?;
        (state.db_path.clone(), state.get_vector_index().ok())
    };

    let conn = db::get_connection(&db_path)
        .map_err(|e| ApiError::DatabaseConnection(e.to_string()))?;

    // 2. 벡터 인덱스에서 해당 폴더의 청크 삭제
    if let Some(vi) = vector_index.as_ref() {
        let file_chunk_ids = db::get_file_and_chunk_ids_in_folder(&conn, &path)?;

        let mut removed_vectors = 0;
        for (_file_id, chunk_ids) in file_chunk_ids {
            for chunk_id in chunk_ids {
                if vi.remove(chunk_id).is_ok() {
                    removed_vectors += 1;
                }
            }
        }

        tracing::info!("Removed {} vectors for folder: {}", removed_vectors, path);

        // 벡터 인덱스 저장 (실패 시 에러 반환 - DB 일관성 유지)
        vi.save()
            .map_err(|e| ApiError::SearchFailed(format!("벡터 인덱스 저장 실패: {}", e)))?;
    }

    // 3. DB에서 파일들 삭제 (FTS + chunks + files)
    let deleted_count = db::delete_files_in_folder(&conn, &path)?;

    tracing::info!("Deleted {} files from folder: {}", deleted_count, path);

    // 4. 감시 폴더 삭제
    db::remove_watched_folder(&conn, &path)?;

    Ok(())
}

/// 인덱스 상태 조회
#[tauri::command]
pub async fn get_index_status(state: State<'_, Mutex<AppState>>) -> ApiResult<IndexStatus> {
    let (db_path, semantic_available, vectors_count) = {
        let state = state.lock()?;
        let vectors_count = state
            .get_vector_index()
            .map(|vi| vi.size())
            .unwrap_or(0);
        (
            state.db_path.clone(),
            state.is_semantic_available(),
            vectors_count,
        )
    };

    let conn = db::get_connection(&db_path)
        .map_err(|e| ApiError::DatabaseConnection(e.to_string()))?;

    let total_files = db::get_file_count(&conn)?;
    let watched_folders = db::get_watched_folders(&conn)?;

    Ok(IndexStatus {
        total_files,
        indexed_files: total_files,
        watched_folders,
        vectors_count,
        semantic_available,
    })
}

/// 폴더별 인덱싱 통계 조회
#[tauri::command]
pub async fn get_folder_stats(
    path: String,
    state: State<'_, Mutex<AppState>>,
) -> ApiResult<FolderStats> {
    let db_path = {
        let state = state.lock()?;
        state.db_path.clone()
    };

    let conn = db::get_connection(&db_path)
        .map_err(|e| ApiError::DatabaseConnection(e.to_string()))?;
    let stats = db::get_folder_stats(&conn, &path)?;

    Ok(FolderStats {
        file_count: stats.file_count,
        last_indexed: stats.last_indexed,
    })
}

/// 감시 폴더 목록 조회 (상세 정보 포함)
#[tauri::command]
pub async fn get_folders_with_info(
    state: State<'_, Mutex<AppState>>,
) -> ApiResult<Vec<WatchedFolderInfo>> {
    let db_path = {
        let state = state.lock()?;
        state.db_path.clone()
    };

    let conn = db::get_connection(&db_path)
        .map_err(|e| ApiError::DatabaseConnection(e.to_string()))?;
    let folders = db::get_watched_folders_with_info(&conn)?;

    Ok(folders
        .into_iter()
        .map(|f| WatchedFolderInfo {
            path: f.path,
            is_favorite: f.is_favorite,
            added_at: f.added_at,
        })
        .collect())
}

/// 즐겨찾기 토글
#[tauri::command]
pub async fn toggle_favorite(
    path: String,
    state: State<'_, Mutex<AppState>>,
) -> ApiResult<bool> {
    let db_path = {
        let state = state.lock()?;
        state.db_path.clone()
    };

    let conn = db::get_connection(&db_path)
        .map_err(|e| ApiError::DatabaseConnection(e.to_string()))?;
    let is_favorite = db::toggle_favorite(&conn, &path)?;

    tracing::info!("Toggled favorite for {}: {}", path, is_favorite);
    Ok(is_favorite)
}
