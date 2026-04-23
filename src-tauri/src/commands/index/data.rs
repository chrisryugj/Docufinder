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

    // 1. 취소 신호 즉시 브로드캐스트 + 감시 중지 (인덱싱 중이면 빨리 종료하도록)
    emit_step("stopping-watchers");
    {
        let container = state.read()?;
        // 취소 신호를 먼저 전달해야 각 스레드가 최대한 빨리 탈출
        let service = container.index_service();
        service.cancel_indexing();
        if container.get_vector_index().is_ok() {
            let _ = service.cancel_vector_indexing();
        }
        if let Ok(wm) = container.get_watch_manager() {
            if let Ok(mut wm) = wm.write() {
                wm.pause();
            }
        }
        tracing::info!("Cancel signals broadcast + watchers paused");
    }

    // 2. 벡터 워커 종료 대기 + 인덱스 클리어
    emit_step("cancelling-indexing");
    // FTS 파이프라인이 cancel_flag 를 체크 → COMMIT/ROLLBACK 후 종료할 시간.
    // 200ms 면 consumer 의 recv_timeout(100ms) 루프가 2회 돌아 대부분 탈출.
    // 이 sleep 이 없으면 DROP 이 FTS 의 WAL read lock 과 경쟁해 수 초 지연.
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    {
        let container = state.read()?;
        let service = container.index_service();
        service.stop_vector_worker();
    }

    emit_step("clearing-vectors");
    {
        let container = state.read()?;
        let service = container.index_service();
        service.clear_vector_index();
    }

    // 3. DB 초기화 전 풀 drain + WAL 체크포인트
    //    잔존 read lock/WAL snapshot 때문에 첫 DROP이 수 초 지연되던 문제 해결.
    //    재시도가 빠른 이유는 두 번째엔 이미 잔존 lock이 정리됐기 때문.
    emit_step("clearing-database");
    crate::db::pool::drain_pool();
    {
        let container = state.read()?;
        container
            .index_service()
            .clear_database()
            .map_err(ApiError::from)?;
    }

    // 4. 파일명 캐시 클리어
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
