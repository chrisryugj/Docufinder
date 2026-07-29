//! App Initialization Commands - initialize_app + spawn_startup_sync_async

use super::*;

/// 앱 초기화: 벡터 인덱싱 재개 + Startup Sync 시작
/// (면책 동의 후 프론트엔드에서 호출)
#[tauri::command]
pub async fn initialize_app(
    app_handle: AppHandle,
    _state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<()> {
    tracing::info!("Initializing app after disclaimer acceptance");

    // 벡터 인덱싱 자동 재개는 제거됨 (AI RAG 전용 — 사용자가 start_vector_indexing 을
    // 명시 호출할 때만 실행). 여기서는 startup sync 만 시작한다.
    spawn_startup_sync_async(app_handle);

    Ok(())
}

/// 앱 시작 시 완료된 폴더 자동 동기화 (오프라인 변경 감지)
pub(super) fn spawn_startup_sync_async(app_handle: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        let (folders_to_sync, service, include_subfolders, max_file_size_mb, exclude_dirs) = {
            let container_state = match app_handle.try_state::<RwLock<AppContainer>>() {
                Some(c) => c,
                None => return,
            };
            let container = match container_state.read() {
                Ok(c) => c,
                Err(_) => return,
            };
            let conn = match crate::db::get_connection(&container.db_path) {
                Ok(c) => c,
                Err(_) => return,
            };
            let folder_infos = crate::db::get_watched_folders_with_info(&conn).unwrap_or_default();
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            const SYNC_SKIP_SECS: i64 = 300;
            let completed: Vec<String> = folder_infos
                .into_iter()
                .filter(|f| {
                    if f.indexing_status != "completed" {
                        return false;
                    }
                    match f.last_synced_at {
                        Some(ts) if (now - ts) < SYNC_SKIP_SECS => {
                            tracing::debug!(
                                "[Startup Sync] Skipping {} (synced {}s ago)",
                                f.path,
                                now - ts
                            );
                            false
                        }
                        _ => true,
                    }
                })
                .map(|f| f.path)
                .collect();

            if completed.is_empty() {
                return;
            }

            (
                completed,
                container.index_service(),
                container.get_settings().include_subfolders,
                container.get_settings().max_file_size_mb,
                {
                    let mut dirs: Vec<String> = crate::constants::DEFAULT_EXCLUDED_DIRS
                        .iter()
                        .map(|s| s.to_string())
                        .collect();
                    dirs.extend(container.get_settings().exclude_dirs.clone());
                    dirs
                },
            )
        };

        let db_path = {
            let cs = match app_handle.try_state::<RwLock<AppContainer>>() {
                Some(c) => c,
                None => return,
            };
            cs.read().map(|c| c.db_path.clone()).unwrap_or_default()
        };

        // 전체 루프를 하나의 pause/resume로 감싸기 (매 폴더 pause/resume 오버헤드 제거)
        if let Some(cs) = app_handle.try_state::<RwLock<AppContainer>>() {
            if let Ok(c) = cs.read() {
                if let Ok(wm) = c.get_watch_manager() {
                    if let Ok(mut wm) = wm.write() {
                        wm.pause();
                    }
                }
            }
        }

        let mut total_added = 0usize;
        let mut total_deleted = 0usize;
        let mut synced_folders: Vec<String> = Vec::new();

        for folder in &folders_to_sync {
            let path = std::path::Path::new(folder);
            if !path.exists() {
                continue;
            }

            // sync_folder: diff 기반 (추가/삭제만 처리, 전체 재인덱싱 아님)
            match service
                .sync_folder(
                    path,
                    include_subfolders,
                    None,
                    max_file_size_mb,
                    exclude_dirs.clone(),
                )
                .await
            {
                Ok(result) => {
                    total_added += result.added;
                    total_deleted += result.deleted;
                    synced_folders.push(folder.to_string());
                    if result.added > 0 || result.deleted > 0 {
                        tracing::info!(
                            "[Startup Sync] {}: +{} added, -{} deleted, {} unchanged",
                            folder,
                            result.added,
                            result.deleted,
                            result.unchanged
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!("[Startup Sync] Sync failed for {}: {}", folder, e);
                }
            }
        }

        // 동기화 완료된 폴더들의 last_synced_at 일괄 업데이트 (커넥션 1회)
        if !synced_folders.is_empty() {
            if let Ok(conn) = crate::db::get_connection(&db_path) {
                for folder in &synced_folders {
                    let _ = crate::db::update_last_synced_at(&conn, folder);
                }
            }
        }

        // stale prune — sync 대상이 아니었던 폴더 (인덱싱 중/skip)에도 잔재 레코드 남을 수 있음.
        // 디스크에 실재하지 않는 모든 files 레코드를 일괄 삭제. 10만 파일 기준 수초.
        let db_path_for_prune = db_path.clone();
        let _ = tokio::task::spawn_blocking(move || {
            match crate::commands::maintenance::prune_missing_files_impl(&db_path_for_prune) {
                Ok(r) if r.pruned > 0 => tracing::info!(
                    "[Startup Prune] {} stale records cleaned ({}ms)",
                    r.pruned,
                    r.elapsed_ms
                ),
                Ok(_) => {}
                Err(e) => tracing::warn!("[Startup Prune] failed: {:?}", e),
            }
        })
        .await;

        // 루프 완료 후 watcher 복구 — resume 은 폴더 목록 조회 성공 여부와 무관하게
        // 수행한다 (DB 연결 실패 시에도 pause_count 짝은 맞춰야 감시가 죽지 않음)
        if let Some(cs) = app_handle.try_state::<RwLock<AppContainer>>() {
            if let Ok(c) = cs.read() {
                let remaining: Vec<String> = crate::db::get_connection(&c.db_path)
                    .ok()
                    .and_then(|conn| crate::db::get_watched_folders(&conn).ok())
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|f| std::path::Path::new(f).exists())
                    .collect();
                if let Ok(wm) = c.get_watch_manager() {
                    if let Ok(mut wm) = wm.write() {
                        wm.resume_with_folders(&remaining);
                    }
                }
            }
        }

        if total_added > 0 || total_deleted > 0 {
            if let Some(cs) = app_handle.try_state::<RwLock<AppContainer>>() {
                if let Ok(c) = cs.read() {
                    let _ = c.load_filename_cache();
                }
            }
            tracing::info!(
                "[Startup Sync] Complete: {} added, {} deleted",
                total_added,
                total_deleted
            );
        } else {
            tracing::info!("[Startup Sync] No offline changes detected");
        }
    });
}
