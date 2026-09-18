//! DB 유지보수·런타임 진단 커맨드 — prune_missing_files, probe_kordoc_runtime 등.

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
/// 10만 파일 기준 수초 소요 (stat만). chunks_fts / chunks 모두 cascade 삭제.
pub fn prune_missing_files_impl(db_path: &Path) -> ApiResult<PruneResult> {
    let start = std::time::Instant::now();
    let conn = db::get_connection(db_path).map_err(|e| ApiError::IndexingFailed(e.to_string()))?;

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
pub async fn prune_missing_files(state: State<'_, RwLock<AppContainer>>) -> ApiResult<PruneResult> {
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

/// 문서 변환기(kordoc 사이드카) 실행 가능성 실측 — 앱 시작 시 프론트가 1회 호출한다.
///
/// 번들 파일 존재만 보는 `is_available` 로는 실행통제가 `node.exe` 를 막는 내부망 PC 를 못 잡아
/// HWP·DOCX·PDF 가 파일마다 조용히 실패했다("인덱싱이 안 된다" 보고). `node cli.js --version`
/// 을 실제로 띄워 `Ok(버전)` / `Err(사용자에게 그대로 띄울 원인·조치 문구)` 를 돌려준다.
/// 실패는 로그에도 남겨 진단 탭 로그 폴더에서 확인 가능하게 한다.
#[tauri::command]
pub async fn probe_kordoc_runtime() -> Result<String, String> {
    let result = tokio::task::spawn_blocking(crate::parsers::kordoc::probe_runtime)
        .await
        .map_err(|e| format!("문서 변환기 점검 스레드 실패: {e}"))?;
    match &result {
        Ok(v) => tracing::info!("kordoc 런타임 점검 통과: v{v}"),
        Err(e) => tracing::error!("kordoc 런타임 점검 실패: {e}"),
    }
    result
}
