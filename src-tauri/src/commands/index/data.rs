//! Data Management Commands - clear_all_data

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
    let clear_result = {
        let container = state.read()?;
        container.index_service().clear_database()
    };

    // 4. 감시 재개 — 성공/실패 무관 수행해 1번의 pause 와 짝을 맞춘다.
    //    (미수행 시 pause_count 가 1로 굳어 이후 add_folder 의 pause/resume 쌍이
    //    0에 도달하지 못해 재시작 전까지 실시간 감시가 전면 죽는다.)
    //    성공 시 watched_folders 는 비어 있어 재등록은 자연히 no-op,
    //    실패 시엔 기존 폴더 감시가 복구된다.
    {
        let container = state.read()?;
        if let Ok(wm) = container.get_watch_manager() {
            if let Ok(mut wm) = wm.write() {
                let folders: Vec<String> = crate::db::get_connection(&container.db_path)
                    .ok()
                    .and_then(|conn| crate::db::get_watched_folders(&conn).ok())
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|f| std::path::Path::new(f).exists())
                    .collect();
                wm.resume_with_folders(&folders);
            }
        }
    }
    clear_result.map_err(ApiError::from)?;

    // 5. 파일명 캐시 클리어
    {
        let container = state.read()?;
        container.get_filename_cache().clear();
    }
    tracing::info!("All data cleared");

    emit_step("completed");
    Ok(())
}
