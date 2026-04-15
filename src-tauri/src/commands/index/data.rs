//! Data Management Commands - clear_all_data, get_db_debug_info

use super::*;

/// 모든 데이터 초기화
#[tauri::command]
pub async fn clear_all_data(state: State<'_, RwLock<AppContainer>>) -> ApiResult<()> {
    tracing::info!("Clearing all data...");

    // 1. 파일 감시 모두 중지
    {
        let container = state.read()?;
        if let Ok(wm) = container.get_watch_manager() {
            if let Ok(mut wm) = wm.write() {
                wm.pause();
                tracing::info!("All watchers paused and stopped");
            }
        }
    }

    // 2. 인덱싱 취소 + 벡터 인덱싱 취소 + 워커 정지 대기
    {
        let container = state.read()?;
        let service = container.index_service();

        // FTS 인덱싱 취소
        service.cancel_indexing();
        tracing::info!("FTS indexing cancelled");

        // 벡터 인덱싱 취소 (clear_all에서도 하지만, 사전에 신호 보내기)
        if container.get_vector_index().is_ok() {
            let _ = service.cancel_vector_indexing();
            tracing::info!("Vector indexing cancelled");
        }
    }

    // 잠시 대기 (워커들이 정지될 시간 확보) - 최대 2초
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // 3. 모든 데이터 클리어
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

#[cfg(debug_assertions)]
#[tauri::command]
pub async fn get_db_debug_info(
    query: String,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<DbDebugInfo> {
    use crate::db;

    let db_path = {
        let container = state.read()?;
        container.db_path.clone()
    };

    let conn =
        db::get_connection(&db_path).map_err(|e| ApiError::DatabaseConnection(e.to_string()))?;

    let files_count: usize = conn
        .query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0))
        .unwrap_or(0);
    let chunks_count: usize = conn
        .query_row("SELECT COUNT(*) FROM chunks", [], |r| r.get(0))
        .unwrap_or(0);
    let chunks_fts_count: usize = conn
        .query_row("SELECT COUNT(*) FROM chunks_fts", [], |r| r.get(0))
        .unwrap_or(0);
    let files_fts_count: usize = conn
        .query_row("SELECT COUNT(*) FROM files_fts", [], |r| r.get(0))
        .unwrap_or(0);

    let safe_query = format!("\"{}\"*", query.replace('"', "\"\""));
    let fts_match_count: usize = conn
        .query_row(
            "SELECT COUNT(*) FROM chunks_fts WHERE chunks_fts MATCH ?",
            [&safe_query],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let filename_match_count: usize = conn
        .query_row(
            "SELECT COUNT(*) FROM files_fts WHERE files_fts MATCH ?",
            [&safe_query],
            |r| r.get(0),
        )
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

#[cfg(not(debug_assertions))]
#[tauri::command]
pub async fn get_db_debug_info(
    _query: String,
    _state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<DbDebugInfo> {
    Err(ApiError::IndexingFailed(
        "Debug command not available in release build".to_string(),
    ))
}
