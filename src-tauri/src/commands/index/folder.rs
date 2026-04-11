//! Folder Commands - add, remove, reindex, resume, stats, favorites

use super::*;

/// 감시 폴더 추가 및 인덱싱 (2단계: FTS → 벡터 백그라운드)
#[tauri::command]
pub async fn add_folder(
    path: String,
    app_handle: AppHandle,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<AddFolderResult> {
    tracing::info!("Adding folder to watch: {}", path);

    // 경로 존재 확인
    let folder_path = Path::new(&path);
    if !folder_path.exists() {
        return Err(ApiError::PathNotFound(path));
    }

    // 경로 정규화
    let canonical_path = folder_path
        .canonicalize()
        .map_err(|e| ApiError::InvalidPath(format!("'{}': {}", path, e)))?;
    let path = canonical_path.to_string_lossy().to_string();

    let ctx = extract_indexing_context(&state)?;

    // 이미 등록된 폴더면 인덱싱 스킵
    if let Ok(conn) = crate::db::get_connection(&ctx.db_path) {
        if crate::db::is_folder_watched(&conn, &path).unwrap_or(false) {
            tracing::info!("Folder already watched, skipping: {}", path);
            return Ok(AddFolderResult {
                success: true,
                indexed_count: 0,
                failed_count: 0,
                vectors_count: 0,
                message: "이미 등록된 폴더입니다. 재인덱싱하려면 '다시 인덱싱' 버튼을 사용하세요."
                    .to_string(),
                errors: vec![],
                hwp_files: vec![],
                ocr_image_count: 0,
            });
        }
    }

    // 1. 감시 폴더 등록
    ctx.service
        .add_watched_folder(&path)
        .map_err(ApiError::from)?;

    // 인덱싱 상태를 'indexing'으로 설정
    if let Ok(conn) = crate::db::get_connection(&ctx.db_path) {
        let _ = crate::db::set_folder_indexing_status(&conn, &path, "indexing");
    }

    // UI에 준비 중 상태 알림 (메타데이터 스캔 전)
    let _ = app_handle.emit(
        "indexing-progress",
        &IndexingProgress {
            phase: "preparing".to_string(),
            total_files: 0,
            processed_files: 0,
            current_file: None,
            folder_path: path.clone(),
            error: None,
        },
    );

    // 2. 기존 감시 일시 중지 (FTS 배치 트랜잭션 중 DB 동시 접근 방지)
    pause_watching(&state);

    // 3. 메타데이터 스캔 (파일명 검색 즉시 가능)
    let metadata_result = ctx
        .service
        .scan_metadata_only(
            &canonical_path,
            ctx.include_subfolders,
            None,
            ctx.max_file_size_mb,
            ctx.exclude_dirs.clone(),
        )
        .await;

    // 3. FilenameCache 즉시 갱신
    if let Ok(ref meta) = metadata_result {
        refresh_filename_cache(&state);
        tracing::info!(
            "FilenameCache ready: {} files (metadata scan)",
            meta.files_found
        );
    }

    // 4. FTS 인덱싱
    let progress_callback = create_fts_progress_callback(app_handle.clone());
    let result = match ctx
        .service
        .index_folder_fts(
            &canonical_path,
            ctx.include_subfolders,
            Some(progress_callback),
            ctx.max_file_size_mb,
            ctx.exclude_dirs.clone(),
        )
        .await
    {
        Ok(r) => r,
        Err(e) => {
            if let Ok(conn) = crate::db::get_connection(&ctx.db_path) {
                let _ = crate::db::set_folder_indexing_status(&conn, &path, "failed");
            }
            resume_watching(&state, &ctx.db_path); // pause 해제 후 에러 반환
            return Err(ApiError::from(e));
        }
    };

    // 5. FilenameCache 최종 갱신 (FTS 인덱싱 후)
    refresh_filename_cache(&state);

    let was_cancelled = result.was_cancelled;

    // 인덱싱 상태 업데이트
    if let Ok(conn) = crate::db::get_connection(&ctx.db_path) {
        let status = if was_cancelled {
            "cancelled"
        } else {
            "completed"
        };
        let _ = crate::db::set_folder_indexing_status(&conn, &path, status);
        if !was_cancelled {
            let _ = crate::db::update_last_synced_at(&conn, &path);
        }
    }

    // 드라이브 루트 감지 (C:\, D:\ 등) → 벡터 인덱싱 자동 시작 안 함 + 경고 알림
    let is_drive_root = {
        let p = canonical_path.to_string_lossy();
        let normalized = p.replace("\\\\?\\", "");
        normalized.len() <= 3 && normalized.chars().nth(1) == Some(':')
    };
    if is_drive_root {
        tracing::warn!(
            "Drive root detected: auto vector indexing disabled for {}. \
             Large drives may take significant time and memory.",
            path
        );
        // 프론트엔드에 경고 알림 (대용량 인덱싱 완료 후)
        let _ = app_handle.emit(
            "indexing-warning",
            &serde_json::json!({
                "type": "drive_root",
                "folder_path": path,
                "message": "드라이브 전체 인덱싱이 완료되었습니다. 시맨틱(벡터) 검색은 대용량 드라이브에서 자동 시작되지 않습니다. 필요 시 설정에서 수동으로 시작하세요."
            }),
        );
    }

    let auto_vector_started = maybe_start_auto_vector(
        &ctx,
        app_handle,
        was_cancelled,
        result.indexed_count,
        is_drive_root,
        Some(&state),
    );
    if !auto_vector_started {
        // 수동 벡터 모드이거나 자동 시작 대상이 아니면 여기서 watcher 재개
        resume_watching(&state, &ctx.db_path);
    }

    let message = build_result_message(
        &result,
        was_cancelled,
        ctx.semantic_available && ctx.semantic_enabled,
        false,
    );
    log_indexing_errors(&result.errors);

    Ok(AddFolderResult {
        success: true,
        indexed_count: result.indexed_count,
        failed_count: result.failed_count,
        vectors_count: 0,
        message,
        errors: result.errors,
        hwp_files: result.hwp_files,
        ocr_image_count: result.ocr_image_count,
    })
}

/// 감시 폴더 제거 (비동기 — 즉시 응답 후 백그라운드 삭제)
#[tauri::command]
pub async fn remove_folder(
    path: String,
    app_handle: AppHandle,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<()> {
    tracing::info!("Removing folder from watch: {}", path);

    // 파일 감시 즉시 중지 (동기, 빠름)
    stop_file_watching(&state, Path::new(&path))?;

    // DB/벡터 삭제에 필요한 것들을 미리 추출 (State는 spawn 안으로 못 넘김)
    let (service, db_path, filename_cache) = {
        let container = state.read()?;
        (
            container.folder_service(),
            container.db_path.clone(),
            container.get_filename_cache(),
        )
    };

    // 즉시 응답 — 무거운 삭제는 백그라운드
    let path_clone = path.clone();
    tauri::async_runtime::spawn(async move {
        match service.remove_folder(&path_clone).await {
            Ok(()) => {
                // FilenameCache 갱신
                if let Ok(conn) = crate::db::get_connection(&db_path) {
                    let _ = filename_cache.load_from_db(&conn);
                }
                let _ = app_handle.emit(
                    "folder-removed",
                    serde_json::json!({
                        "path": path_clone,
                        "success": true,
                    }),
                );
                tracing::info!("Folder removed successfully: {}", path_clone);
            }
            Err(e) => {
                tracing::error!("Folder removal failed: {}: {}", path_clone, e);
                let _ = app_handle.emit(
                    "folder-removed",
                    serde_json::json!({
                        "path": path_clone,
                        "success": false,
                        "error": e.to_string(),
                    }),
                );
            }
        }
    });

    Ok(())
}

/// 폴더 재인덱싱
#[tauri::command]
pub async fn reindex_folder(
    path: String,
    app_handle: AppHandle,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<AddFolderResult> {
    tracing::info!("Reindexing folder: {}", path);

    let folder_path = Path::new(&path);
    if !folder_path.exists() {
        return Err(ApiError::PathNotFound(path));
    }

    let canonical_path = folder_path
        .canonicalize()
        .map_err(|e| ApiError::InvalidPath(format!("'{}': {}", path, e)))?;

    let ctx = extract_indexing_context(&state)?;

    // 인덱싱 상태를 'indexing'으로 설정
    let path_str = canonical_path.to_string_lossy().to_string();
    if let Ok(conn) = crate::db::get_connection(&ctx.db_path) {
        let _ = crate::db::set_folder_indexing_status(&conn, &path_str, "indexing");
    }

    // 재인덱싱 전 감시 일시 중지 (FTS 배치 트랜잭션 중 DB 동시 접근 방지)
    pause_watching(&state);

    // IndexService로 재인덱싱 위임
    let progress_callback = create_fts_progress_callback(app_handle.clone());
    let result = match ctx
        .service
        .reindex_folder(
            &canonical_path,
            ctx.include_subfolders,
            Some(progress_callback),
            ctx.max_file_size_mb,
            ctx.exclude_dirs.clone(),
        )
        .await
    {
        Ok(r) => r,
        Err(e) => {
            if let Ok(conn) = crate::db::get_connection(&ctx.db_path) {
                let _ = crate::db::set_folder_indexing_status(&conn, &path_str, "failed");
            }
            resume_watching(&state, &ctx.db_path); // pause 해제 후 에러 반환
            return Err(ApiError::from(e));
        }
    };

    refresh_filename_cache(&state);

    let was_cancelled = result.was_cancelled;

    if let Ok(conn) = crate::db::get_connection(&ctx.db_path) {
        let status = if was_cancelled {
            "cancelled"
        } else {
            "completed"
        };
        let _ = crate::db::set_folder_indexing_status(&conn, &path_str, status);
        if !was_cancelled {
            let _ = crate::db::update_last_synced_at(&conn, &path_str);
        }
    }

    let auto_vector_started = maybe_start_auto_vector(
        &ctx,
        app_handle,
        was_cancelled,
        result.indexed_count,
        false,
        Some(&state),
    );
    if !auto_vector_started {
        resume_watching(&state, &ctx.db_path);
    }

    let message = build_result_message(
        &result,
        was_cancelled,
        ctx.semantic_available && ctx.semantic_enabled,
        true,
    );

    Ok(AddFolderResult {
        success: true,
        indexed_count: result.indexed_count,
        failed_count: result.failed_count,
        vectors_count: 0,
        message,
        errors: result.errors,
        hwp_files: result.hwp_files,
        ocr_image_count: result.ocr_image_count,
    })
}

/// 미완료 인덱싱 재개 (resume_folder_fts 기반: fts_indexed_at이 있는 파일 스킵)
#[tauri::command]
pub async fn resume_indexing(
    path: String,
    app_handle: AppHandle,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<AddFolderResult> {
    tracing::info!("Resuming incomplete indexing: {}", path);

    let folder_path = Path::new(&path);
    if !folder_path.exists() {
        return Err(ApiError::PathNotFound(path));
    }

    let canonical_path = folder_path
        .canonicalize()
        .map_err(|e| ApiError::InvalidPath(format!("'{}': {}", path, e)))?;

    let ctx = extract_indexing_context(&state)?;

    // UI에 준비 중 상태 알림
    let path_str = canonical_path.to_string_lossy().to_string();
    let _ = app_handle.emit(
        "indexing-progress",
        &IndexingProgress {
            phase: "preparing".to_string(),
            total_files: 0,
            processed_files: 0,
            current_file: None,
            folder_path: path_str.clone(),
            error: None,
        },
    );

    // 인덱싱 상태를 'indexing'으로 유지
    if let Ok(conn) = crate::db::get_connection(&ctx.db_path) {
        let _ = crate::db::set_folder_indexing_status(&conn, &path_str, "indexing");
    }

    // FTS resume도 배치 DB 쓰기가 길어 watcher와 충돌하므로 동일하게 pause
    pause_watching(&state);

    // resume_folder_fts 기반 인덱싱 (fts_indexed_at이 있는 파일은 스킵)
    // sync_folder는 강종 후 메타데이터 트랜잭션 롤백 시 모든 파일을 "new"로 인식하여
    // 0부터 재인덱싱하는 문제가 있었음
    let progress_callback = create_fts_progress_callback(app_handle.clone());
    let result = match ctx
        .service
        .resume_folder_fts(
            &canonical_path,
            ctx.include_subfolders,
            Some(progress_callback),
            ctx.max_file_size_mb,
            ctx.exclude_dirs.clone(),
        )
        .await
    {
        Ok(r) => r,
        Err(e) => {
            if let Ok(conn) = crate::db::get_connection(&ctx.db_path) {
                let _ = crate::db::set_folder_indexing_status(&conn, &path_str, "failed");
            }
            resume_watching(&state, &ctx.db_path);
            return Err(ApiError::from(e));
        }
    };

    refresh_filename_cache(&state);

    let was_cancelled = result.was_cancelled;

    if let Ok(conn) = crate::db::get_connection(&ctx.db_path) {
        let status = if was_cancelled {
            "cancelled"
        } else {
            "completed"
        };
        let _ = crate::db::set_folder_indexing_status(&conn, &path_str, status);
        if !was_cancelled {
            let _ = crate::db::update_last_synced_at(&conn, &path_str);
        }
    }

    let auto_vector_started = maybe_start_auto_vector(
        &ctx,
        app_handle,
        was_cancelled,
        result.indexed_count,
        false,
        Some(&state),
    );
    if !auto_vector_started {
        resume_watching(&state, &ctx.db_path);
    }

    let message = build_result_message(
        &result,
        was_cancelled,
        ctx.semantic_available && ctx.semantic_enabled,
        false,
    );
    log_indexing_errors(&result.errors);

    Ok(AddFolderResult {
        success: true,
        indexed_count: result.indexed_count,
        failed_count: result.failed_count,
        vectors_count: 0,
        message,
        errors: result.errors,
        hwp_files: result.hwp_files,
        ocr_image_count: result.ocr_image_count,
    })
}

// ============================================
// Folder Info Commands (FolderService 위임)
// ============================================

/// 폴더별 인덱싱 통계 조회
#[tauri::command]
pub async fn get_folder_stats(
    path: String,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<FolderStats> {
    let service = {
        let container = state.read()?;
        container.folder_service()
    };
    service
        .get_folder_stats(&path)
        .await
        .map_err(ApiError::from)
}

/// 전체 폴더 통계 배치 조회 (N+1 IPC 방지)
#[tauri::command]
pub async fn get_all_folder_stats(
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<std::collections::HashMap<String, FolderStats>> {
    let service = {
        let container = state.read()?;
        container.folder_service()
    };
    let stats = service
        .get_all_folder_stats()
        .await
        .map_err(ApiError::from)?;
    Ok(stats.into_iter().collect())
}

/// 감시 폴더 목록 조회
#[tauri::command]
pub async fn get_folders_with_info(
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<Vec<WatchedFolderInfo>> {
    let service = {
        let container = state.read()?;
        container.folder_service()
    };
    service
        .get_folders_with_info()
        .await
        .map_err(ApiError::from)
}

/// 즐겨찾기 토글
#[tauri::command]
pub async fn toggle_favorite(
    path: String,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<bool> {
    let service = {
        let container = state.read()?;
        container.folder_service()
    };
    service.toggle_favorite(&path).await.map_err(ApiError::from)
}
