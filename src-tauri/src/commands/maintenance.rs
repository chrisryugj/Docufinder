//! DB 유지보수 커맨드 — prune_missing_files 등.

use crate::application::container::AppContainer;
use crate::{db, ApiError, ApiResult};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use tauri::State;

#[derive(Debug, Serialize)]
pub struct PruneResult {
    pub total_checked: usize,
    pub pruned: usize,
    pub elapsed_ms: u64,
}

/// DB의 files 테이블을 스캔하여 디스크에 없는 파일 레코드를 삭제한다.
///
/// - Startup sync에서 자동 호출 (init.rs의 spawn_startup_sync_async 말미)
/// - 설정 > "없는 파일 정리" 버튼에서 수동 호출
///
/// 10만 파일 기준 수초 소요 (stat만). chunks_fts / files_fts / chunks 모두 cascade 삭제.
pub fn prune_missing_files_impl(db_path: &Path) -> ApiResult<PruneResult> {
    let start = std::time::Instant::now();
    let conn = db::get_connection(db_path)
        .map_err(|e| ApiError::IndexingFailed(e.to_string()))?;

    let paths: Vec<String> = {
        let mut stmt = conn
            .prepare("SELECT path FROM files")
            .map_err(|e| ApiError::IndexingFailed(e.to_string()))?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| ApiError::IndexingFailed(e.to_string()))?;
        rows.filter_map(|r| r.ok()).collect()
    };

    let total_checked = paths.len();
    let mut pruned = 0usize;

    for path in &paths {
        if !PathBuf::from(path).exists() {
            match db::delete_file(&conn, path) {
                Ok(_) => pruned += 1,
                Err(e) => tracing::warn!("[Prune] Failed to delete stale {}: {}", path, e),
            }
        }
    }

    if pruned > 0 {
        tracing::info!(
            "[Prune] {}/{} stale records removed in {}ms",
            pruned,
            total_checked,
            start.elapsed().as_millis()
        );
    }

    Ok(PruneResult {
        total_checked,
        pruned,
        elapsed_ms: start.elapsed().as_millis() as u64,
    })
}

/// 수동 실행용 Tauri 커맨드 (설정 > "없는 파일 정리" 버튼).
#[tauri::command]
pub async fn prune_missing_files(
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<PruneResult> {
    let db_path = {
        let c = state
            .read()
            .map_err(|_| ApiError::IndexingFailed("Lock error".into()))?;
        c.db_path.clone()
    };

    tokio::task::spawn_blocking(move || prune_missing_files_impl(&db_path))
        .await
        .map_err(|e| ApiError::IndexingFailed(e.to_string()))?
}
