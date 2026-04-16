//! Data Management Commands - clear_all_data, get_db_debug_info

use super::*;

/// 모든 데이터 초기화 (단계별 진행상황 이벤트 emit)
#[tauri::command]
pub async fn clear_all_data(
    app: AppHandle,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<()> {
    use tauri::Emitter;

    let emit_step = |step: &str| {
        let _ = app.emit("clear-data-progress", step);
    };

    tracing::info!("Clearing all data...");

    // 1. 파일 감시 모두 중지
    emit_step("stopping-watchers");
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
    emit_step("cancelling-indexing");
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

    // 3. 벡터 워커 종료 대기 + 인덱스 클리어
    emit_step("clearing-vectors");
    {
        let container = state.read()?;
        let service = container.index_service();
        service.stop_vector_worker();
        service.clear_vector_index();
    }

    // 4. DB DROP + 재생성 (DELETE 대비 수백 배 빠름, VACUUM 불필요)
    emit_step("clearing-database");
    {
        let container = state.read()?;
        container.index_service().clear_database().map_err(ApiError::from)?;
    }

    // 5. 파일명 캐시 클리어
    {
        let container = state.read()?;
        container.get_filename_cache().clear();
    }
    tracing::info!("All data cleared");

    emit_step("completed");
    Ok(())
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
