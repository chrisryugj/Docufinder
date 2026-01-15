use crate::db;
use crate::indexer::pipeline;
use crate::AppState;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;
use tauri::State;

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
}

/// 감시 폴더 추가 및 인덱싱
#[tauri::command]
pub async fn add_folder(
    path: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<AddFolderResult, String> {
    tracing::info!("Adding folder to watch: {}", path);

    let folder_path = Path::new(&path);
    if !folder_path.exists() {
        return Err(format!("Folder does not exist: {}", path));
    }

    let (db_path, embedder, vector_index) = {
        let state = state.lock().map_err(|e| e.to_string())?;
        (
            state.db_path.clone(),
            state.get_embedder().ok(),
            state.get_vector_index().ok(),
        )
    };

    let conn = db::get_connection(&db_path).map_err(|e| e.to_string())?;

    // 1. 감시 폴더 등록
    db::add_watched_folder(&conn, &path).map_err(|e| e.to_string())?;

    // 2. 폴더 인덱싱
    let result = pipeline::index_folder(
        &conn,
        folder_path,
        embedder.as_ref(),
        vector_index.as_ref(),
    )
    .map_err(|e| e.to_string())?;

    // 3. 파일 감시 시작
    {
        let state = state.lock().map_err(|e| e.to_string())?;
        if let Ok(wm) = state.get_watch_manager() {
            if let Ok(mut wm) = wm.write() {
                if let Err(e) = wm.watch(folder_path) {
                    tracing::warn!("Failed to start watching {}: {}", path, e);
                }
            }
        }
    }

    let message = if result.failed_count > 0 {
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

    Ok(AddFolderResult {
        success: true,
        indexed_count: result.indexed_count,
        failed_count: result.failed_count,
        vectors_count: result.vectors_count,
        message,
    })
}

/// 감시 폴더 제거
#[tauri::command]
pub async fn remove_folder(
    path: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    tracing::info!("Removing folder from watch: {}", path);

    let folder_path = Path::new(&path);

    // 1. 파일 감시 중지
    {
        let state = state.lock().map_err(|e| e.to_string())?;
        if let Ok(wm) = state.get_watch_manager() {
            if let Ok(mut wm) = wm.write() {
                let _ = wm.unwatch(folder_path);
            }
        }
    }

    let db_path = {
        let state = state.lock().map_err(|e| e.to_string())?;
        state.db_path.clone()
    };

    let conn = db::get_connection(&db_path).map_err(|e| e.to_string())?;

    // 2. 감시 폴더 삭제
    db::remove_watched_folder(&conn, &path).map_err(|e| e.to_string())?;

    // TODO: 해당 폴더의 파일들도 인덱스에서 삭제 (벡터 포함)

    Ok(())
}

/// 인덱스 상태 조회
#[tauri::command]
pub async fn get_index_status(state: State<'_, Mutex<AppState>>) -> Result<IndexStatus, String> {
    let (db_path, semantic_available, vectors_count) = {
        let state = state.lock().map_err(|e| e.to_string())?;
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

    let conn = db::get_connection(&db_path).map_err(|e| e.to_string())?;

    let total_files = db::get_file_count(&conn).map_err(|e| e.to_string())?;
    let watched_folders = db::get_watched_folders(&conn).map_err(|e| e.to_string())?;

    Ok(IndexStatus {
        total_files,
        indexed_files: total_files,
        watched_folders,
        vectors_count,
        semantic_available,
    })
}
