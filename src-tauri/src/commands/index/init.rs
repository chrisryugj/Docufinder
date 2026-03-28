//! App Initialization Commands - initialize_app + spawn_startup_sync_async

use super::*;

/// 앱 초기화: 벡터 인덱싱 재개 + Startup Sync 시작
/// (면책 동의 후 프론트엔드에서 호출)
#[tauri::command]
pub async fn initialize_app(
    app_handle: AppHandle,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<()> {
    tracing::info!("Initializing app after disclaimer acceptance");

    // 미완료 벡터 인덱싱 자동 재개
    // 단, FTS 미완료 폴더가 존재하면 벡터 인덱싱 스킵
    // (FolderTree의 resume_indexing이 FTS → 벡터 순서로 처리하므로 동시 실행 시 DB Lock 발생)
    let has_pending_chunks = {
        let container = state.read()?;
        let startup_settings = container.get_settings();
        let should_auto_resume = container.is_semantic_available()
            && startup_settings.semantic_search_enabled
            && startup_settings.vector_indexing_mode == VectorIndexingMode::Auto;

        if should_auto_resume {
            let Ok(conn) = crate::db::get_connection(&container.db_path) else {
                return Ok(());
            };

            // FTS 미완료 폴더가 있으면 벡터 인덱싱 스킵
            // (resume_indexing이 FTS 완료 후 auto vector를 순차적으로 시작)
            let has_incomplete_fts = crate::db::get_watched_folders_with_info(&conn)
                .map(|folders| {
                    folders.iter().any(|f| {
                        f.indexing_status == "indexing" || f.indexing_status == "cancelled"
                    })
                })
                .unwrap_or(false);

            if has_incomplete_fts {
                tracing::info!(
                    "[initialize_app] FTS 미완료 폴더 존재 → 벡터 인덱싱 스킵 (resume_indexing에서 순차 처리)"
                );
                false
            } else {
                let Ok(stats) = crate::db::get_vector_indexing_stats(&conn) else {
                    return Ok(());
                };
                stats.pending_chunks > 0
            }
        } else {
            false
        }
    };

    if has_pending_chunks {
        tracing::info!("Found pending vector chunks. Starting background indexing.");

        // read lock을 최소 범위로 유지하고, 필요한 데이터만 추출
        let (
            embedder,
            vector_index,
            vector_worker,
            db_path,
            intensity,
            watched_folders,
            watch_manager,
        ) = {
            let container = state.read()?;
            let watched_folders = if let Ok(conn) = crate::db::get_connection(&container.db_path) {
                crate::db::get_watched_folders(&conn).unwrap_or_default()
            } else {
                vec![]
            };
            (
                container.get_embedder(),
                container.get_vector_index(),
                container.get_vector_worker(),
                container.db_path.clone(),
                container.get_settings().indexing_intensity.clone(),
                watched_folders,
                container.get_watch_manager(),
            )
        }; // read lock 해제

        // watcher 일시 중지 (lock 해제 후 별도 수행)
        if let Ok(ref wm) = watch_manager {
            if let Ok(mut wm) = wm.write() {
                wm.pause();
            }
        }

        if let (Ok(emb), Ok(vi)) = (embedder, vector_index) {
            if let Ok(mut worker) = vector_worker.write() {
                let app_handle_clone = app_handle.clone();
                let watched_folders_clone = watched_folders.clone();
                let started = worker.start(
                    db_path,
                    emb,
                    vi,
                    Some(Arc::new(move |progress| {
                        let _ = app_handle_clone.emit("vector-indexing-progress", &progress);
                        // 벡터 인덱싱 완료 시 watcher 재개 + startup sync 시작
                        if progress.is_complete {
                            if let Some(cs) = app_handle_clone.try_state::<RwLock<AppContainer>>() {
                                if let Ok(c) = cs.read() {
                                    if let Ok(wm) = c.get_watch_manager() {
                                        if let Ok(mut wm) = wm.write() {
                                            wm.resume_with_folders(&watched_folders_clone);
                                        }
                                    }
                                }
                            }
                            // 벡터 완료 후 startup sync 시작
                            spawn_startup_sync_async(app_handle_clone.clone());
                        }
                    })),
                    Some(intensity),
                );
                if started.is_err() {
                    // 시작 실패 → 즉시 재개 (pause만 된 채로 방치 방지)
                    tracing::warn!("Failed to start vector indexing worker");
                    if let Ok(ref wm) = watch_manager {
                        if let Ok(mut wm) = wm.write() {
                            wm.resume_with_folders(&watched_folders);
                        }
                    }
                    spawn_startup_sync_async(app_handle.clone());
                }
            }
        }
    } else {
        // 벡터 인덱싱이 필요 없으면 바로 startup sync 시작
        spawn_startup_sync_async(app_handle);
    }

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
                    if let Ok(conn) = crate::db::get_connection(&db_path) {
                        let _ = crate::db::update_last_synced_at(&conn, folder);
                    }
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

        // 루프 완료 후 watcher 복구
        if let Some(cs) = app_handle.try_state::<RwLock<AppContainer>>() {
            if let Ok(c) = cs.read() {
                if let Ok(conn) = crate::db::get_connection(&c.db_path) {
                    let remaining = crate::db::get_watched_folders(&conn)
                        .unwrap_or_default()
                        .into_iter()
                        .filter(|f| std::path::Path::new(f).exists())
                        .collect::<Vec<_>>();
                    if let Ok(wm) = c.get_watch_manager() {
                        if let Ok(mut wm) = wm.write() {
                            wm.resume_with_folders(&remaining);
                        }
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
