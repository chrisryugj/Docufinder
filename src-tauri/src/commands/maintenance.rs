//! DB 유지보수·런타임 진단 커맨드 — prune_missing_files, probe_kordoc_runtime 등.

use crate::application::container::AppContainer;
use crate::utils::folder_scope::path_in_scope;
use crate::{db, ApiError, ApiResult};
use serde::Serialize;
use std::path::Path;
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
/// "없다"는 확인된 것만 지운다: 파일이 NotFound 이고, 그 파일이 속한 감시 폴더는 지금 보일 때.
/// 감시 폴더가 안 보이면(오프라인 공유·빠진 USB·끊긴 VPN) 그 아래 파일은 없는 게 아니라 못 보는
/// 것이라 남긴다. 종전엔 `exists()` 만 봐서 시작할 때마다 오프라인 폴더의 색인과 북마크를 지웠다.
/// 권한 거부 같은 다른 오류도 판단할 수 없어 남긴다.
///
/// 벡터는 행보다 먼저 지운다 (sync 삭제 경로와 같은 순서). 안 지우면 해제된 chunks.id 가
/// 재사용될 때 다른 문서의 임베딩으로 오귀속된다.
///
/// 10만 파일 기준 수초 소요 (stat만). chunks_fts / chunks 모두 cascade 삭제.
pub fn prune_missing_files_impl(
    db_path: &Path,
    vector_index: Option<&crate::search::vector::VectorIndex>,
) -> ApiResult<PruneResult> {
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
    let mut skipped_offline = 0usize;

    // 감시 폴더별로 지금 보이는지 한 번씩만 확인
    let roots: Vec<(String, bool)> = db::get_watched_folders(&conn)
        .map_err(|e| ApiError::IndexingFailed(e.to_string()))?
        .into_iter()
        .map(|root| {
            let reachable = Path::new(&root).is_dir();
            (root, reachable)
        })
        .collect();

    for path in &paths {
        match std::fs::metadata(path) {
            Ok(_) => continue,
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => continue,
            Err(_) => {}
        }
        let root_offline = roots
            .iter()
            .any(|(root, reachable)| !reachable && path_in_scope(path, root));
        if root_offline {
            skipped_offline += 1;
            continue;
        }
        if let Some(vi) = vector_index {
            if let Ok(chunk_ids) = db::get_chunk_ids_for_path(&conn, path) {
                for chunk_id in chunk_ids {
                    let _ = vi.remove(chunk_id);
                }
            }
        }
        match db::delete_file(&conn, path) {
            Ok(_) => pruned += 1,
            Err(e) => tracing::warn!("[Prune] Failed to delete stale {}: {}", path, e),
        }
    }

    if pruned > 0 {
        if let Some(vi) = vector_index {
            if let Err(e) = vi.save() {
                tracing::warn!("[Prune] 벡터 인덱스 저장 실패: {}", e);
            }
        }
    }
    if skipped_offline > 0 {
        tracing::info!(
            "[Prune] 감시 폴더가 안 보여 {}건은 건너뜀 (오프라인·연결 끊김)",
            skipped_offline
        );
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
    let (db_path, vector_index) = {
        let c = state
            .read()
            .map_err(|_| ApiError::IndexingFailed("Lock error".into()))?;
        (c.db_path.clone(), c.get_vector_index().ok())
    };

    tokio::task::spawn_blocking(move || prune_missing_files_impl(&db_path, vector_index.as_deref()))
        .await
        .map_err(|e| ApiError::IndexingFailed(e.to_string()))?
}

/// 시작 단계 경고(DB 무결성 등) — 프론트가 마운트 때 1회 가져간다 (설정 단계 emit 유실 대비).
#[tauri::command]
pub async fn get_startup_warnings() -> Vec<String> {
    crate::startup::startup_warnings()
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 없는 게 확인된 파일만 지우고, 안 보이는 감시 폴더(오프라인 공유 등) 아래 기록은 남긴다.
    #[test]
    fn prune_keeps_offline_roots_and_removes_confirmed_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("prune.db");
        db::init_database(&db_path).unwrap();

        let online = tmp.path().join("online");
        std::fs::create_dir_all(&online).unwrap();
        let kept = online.join("kept.txt");
        std::fs::write(&kept, "x").unwrap();
        let gone = online.join("gone.txt"); // 기록만 있고 파일은 지워짐
        let offline = tmp.path().join("offline-share"); // 감시 폴더지만 지금 안 보임
        let offline_file = offline.join("doc.hwpx");

        let conn = db::get_connection(&db_path).unwrap();
        db::add_watched_folder(&conn, &online.to_string_lossy()).unwrap();
        db::add_watched_folder(&conn, &offline.to_string_lossy()).unwrap();
        for p in [&kept, &gone, &offline_file] {
            db::insert_file_metadata_only(&conn, &p.to_string_lossy(), "n", "txt", 1, 1).unwrap();
        }
        drop(conn);

        let r = prune_missing_files_impl(&db_path, None).unwrap();
        assert_eq!(r.total_checked, 3);
        assert_eq!(r.pruned, 1, "지워진 파일 1건만 정리돼야 한다");

        let conn = db::get_connection(&db_path).unwrap();
        let left: Vec<String> = conn
            .prepare("SELECT path FROM files")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(left.contains(&kept.to_string_lossy().to_string()));
        assert!(
            left.contains(&offline_file.to_string_lossy().to_string()),
            "오프라인 감시 폴더의 기록이 지워졌다: {left:?}"
        );
        assert!(!left.contains(&gone.to_string_lossy().to_string()));
    }
}
