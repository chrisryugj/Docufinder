//! 앱 종료 절차: 인덱싱 취소, 벡터 리소스 정리, DB 정리(FTS 세그먼트 병합, WAL 체크포인트).

use crate::AppContainer;
use std::sync::RwLock;
use tauri::Manager;

/// 벡터 워커 정리 + 인덱스 저장 + DB 최적화 (종료/트레이 quit 공통)
pub(crate) fn cleanup_vector_resources(container: &AppContainer) {
    // FTS 파이프라인 즉시 취소 신호 (인덱싱 중 종료 시 스레드가 빠르게 탈출하도록)
    container.cancel_indexing();
    // 주기 sync task 중단 신호 (v2.5.2) — 루프가 최대 60초 내 탈출
    container.signal_sync_shutdown();

    let vector_worker = container.get_vector_worker();
    if let Ok(mut worker) = vector_worker.write() {
        if worker.is_running() {
            tracing::info!("Stopping vector worker...");
            worker.cancel();
            worker.join();
        }
    }
    if let Ok(vi) = container.get_vector_index() {
        if let Err(e) = vi.save() {
            tracing::error!("Failed to save vector index: {}", e);
        }
    }
    // DB 최적화: WAL 체크포인트 + 쿼리 플래너 통계 갱신
    cleanup_database(&container.db_path);
}

/// 앱 종료 절차 (트레이 quit + 창 닫기 공통):
/// 즉시 취소 신호 → cleanup 교착 대비 3초 watchdog → 벡터 리소스 정리 → 프로세스 종료
pub(crate) fn graceful_shutdown(app: &tauri::AppHandle) {
    // 즉시 취소 신호 (인덱싱 스레드가 최대한 빨리 탈출하도록)
    if let Some(container) = app.try_state::<RwLock<AppContainer>>() {
        if let Ok(container) = container.read() {
            container.cancel_indexing();
            container.signal_sync_shutdown();
            if let Ok(worker) = container.get_vector_worker().read() {
                worker.cancel();
            }
        }
    }
    // Watchdog: cleanup 교착 시 3초 후 강제 종료 (인덱싱 중 종료 안 되는 버그 방지)
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_secs(3));
        tracing::warn!("Cleanup timeout — forcing process exit");
        std::process::exit(0);
    });
    // 정상 cleanup 시도
    if let Some(container) = app.try_state::<RwLock<AppContainer>>() {
        if let Ok(container) = container.read() {
            cleanup_vector_resources(&container);
        }
    }
    app.exit(0);
}

/// FTS5 세그먼트 점진 병합 (종료 시 시간 예산 내 실행)
///
/// 증분 인덱싱이 누적되면 automerge 기본값만으로는 작은 b-tree 세그먼트가 늘어나
/// MATCH doclist 병합 비용이 점진적으로 증가한다 (prefix 와일드카드 쿼리 특히 민감).
/// 전체 `'optimize'` 는 단일 트랜잭션이라 대용량 DB에서 graceful_shutdown 의 3초
/// watchdog 을 초과해 통째로 롤백될 수 있으므로, FTS5 문서의 'merge=N' 점진 병합
/// 패턴을 사용한다 — 회당 자체 트랜잭션으로 커밋되어 중단돼도 진행분이 보존되고,
/// 남은 병합은 다음 종료 시 이어서 진행된다.
fn merge_fts_segments(conn: &rusqlite::Connection) {
    const MERGE_UNITS: i64 = 64;
    const TIME_BUDGET_MS: u128 = 500; // watchdog 3초 내 체크포인트 시간 확보

    // 음수 파라미터 = 모든 세그먼트를 대상으로 새 병합 사이클 시작
    if let Err(e) = conn.execute(
        "INSERT INTO chunks_fts(chunks_fts, rank) VALUES('merge', ?1)",
        [-MERGE_UNITS],
    ) {
        tracing::warn!("FTS5 segment merge start failed: {}", e);
        return;
    }
    let start = std::time::Instant::now();
    let mut rounds = 0usize;
    while start.elapsed().as_millis() < TIME_BUDGET_MS {
        let before: i64 = conn
            .query_row("SELECT total_changes()", [], |r| r.get(0))
            .unwrap_or(0);
        if conn
            .execute(
                "INSERT INTO chunks_fts(chunks_fts, rank) VALUES('merge', ?1)",
                [MERGE_UNITS],
            )
            .is_err()
        {
            return;
        }
        rounds += 1;
        let after: i64 = conn
            .query_row("SELECT total_changes()", [], |r| r.get(0))
            .unwrap_or(0);
        // FTS5 문서: 'merge' 양수 호출의 total_changes 증가가 2 미만이면 병합할 작업 없음
        if after - before < 2 {
            tracing::info!("FTS5 segment merge complete ({} rounds)", rounds);
            return;
        }
    }
    tracing::info!(
        "FTS5 segment merge: time budget reached ({} rounds, 다음 종료 시 계속)",
        rounds
    );
}

/// 앱 종료 시 DB 정리: 풀 drain → FTS 세그먼트 병합 → WAL 체크포인트 + PRAGMA optimize
fn cleanup_database(db_path: &std::path::Path) {
    // 풀의 모든 커넥션을 먼저 닫아야 WAL 체크포인트가 완전히 적용됨
    // (풀 커넥션이 WAL read lock을 보유하면 TRUNCATE 모드 체크포인트 실패)
    crate::db::pool::drain_pool();

    if let Ok(conn) = crate::db::get_connection(db_path) {
        // FTS5 세그먼트 병합 — WAL 체크포인트 전에 실행해 병합분이 본 DB 파일에 흡수되게 함
        merge_fts_segments(&conn);

        match conn.execute_batch(
            "PRAGMA wal_checkpoint(TRUNCATE);
             PRAGMA optimize;
             PRAGMA incremental_vacuum;",
        ) {
            Ok(_) => tracing::info!(
                "DB cleanup completed (WAL checkpoint + optimize + incremental vacuum)"
            ),
            Err(e) => tracing::warn!("DB cleanup partial failure: {}", e),
        }
    }
}
