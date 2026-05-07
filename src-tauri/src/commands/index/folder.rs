//! Folder Commands - add, remove, reindex, resume, stats, favorites

use super::*;
use serde::Serialize;

/// 폴더 추가 전 사전 분류 결과 — 프론트가 다이얼로그 분기에 사용.
#[derive(Debug, Serialize)]
pub struct FolderClassification {
    pub kind: crate::utils::cloud_detect::LocationKind,
    /// `Settings.skip_cloud_body_indexing` 토글 현재 값. 프론트가 안내 문구 분기에 사용.
    pub skip_body_enabled: bool,
    /// 시스템 보호 폴더(C:\Windows, /System, /usr/bin 등) 여부.
    /// true면 프론트가 강한 경고 다이얼로그를 띄움.
    pub is_system: bool,
    /// `Settings.allow_system_folders` 토글 현재 값.
    /// false면 프론트는 안내 후 add_folder 호출 자체를 막음.
    pub allow_system_enabled: bool,
}

/// 폴더가 클라우드/네트워크 위치인지 사전 분류 (add_folder 호출 전 다이얼로그 안내용).
///
/// 경로 정규화 + 분류만 수행, 인덱싱 부작용 없음. 존재하지 않는 경로는 Local 로 응답.
#[tauri::command]
pub async fn classify_folder(path: String) -> ApiResult<FolderClassification> {
    let folder_path = Path::new(&path);
    let canonical = dunce::canonicalize(folder_path).unwrap_or_else(|_| folder_path.to_path_buf());
    Ok(FolderClassification {
        kind: crate::utils::cloud_detect::classify(&canonical),
        skip_body_enabled: crate::utils::cloud_detect::is_skip_enabled(),
        is_system: crate::constants::is_blocked_path(&canonical),
        allow_system_enabled: crate::constants::is_allow_system_folders(),
    })
}

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

    // 경로 정규화 — std::canonicalize 는 UNC 를 \\?\UNC\... 로 만들어 외부 도구 호환을 깨고
    // 네트워크 폴더면 서버 응답을 기다리며 수십 초 block 될 수 있다.
    // dunce::canonicalize 는 가능한 곳까지 정규화 후 \\srv\share\... 형태로 보존한다.
    let canonical_path = dunce::canonicalize(folder_path)
        .map_err(|e| ApiError::InvalidPath(format!("'{}': {}", path, e)))?;

    // 시스템 폴더 / 드라이브 루트 차단
    crate::constants::validate_watch_path(&canonical_path)
        .map_err(|msg| ApiError::AccessDenied(msg.to_string()))?;

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

    // 드라이브 루트(C:\, D:\ 등) 감지 → 벡터 자동 시작 스킵 + 경고 emit
    // 드라이브 전체 인덱싱은 Everything 스타일 검색의 정상 기능이지만,
    // 수백만 파일 대상 벡터 임베딩은 메모리/시간 소모가 커 사용자 수동 선택으로 미룬다.
    let is_drive_root = crate::constants::is_drive_root(&canonical_path);
    // 시스템 폴더(C:\Windows, /System 등): allow 토글이 켜져 여기까지 왔다는 뜻.
    // 바이너리/시스템 파일이 대부분이라 벡터는 의미가 적고 비용만 크다 → 드라이브 루트와 동일 처리.
    let is_system_folder = crate::constants::is_blocked_path(&canonical_path);
    if is_drive_root {
        tracing::warn!(
            "Drive root detected: auto vector indexing disabled for {}",
            path
        );
        let _ = app_handle.emit(
            "indexing-warning",
            &serde_json::json!({
                "type": "drive_root",
                "folder_path": path,
                "message": "드라이브 전체 인덱싱이 완료되었습니다. 시맨틱(벡터) 검색은 대용량 드라이브에서 자동 시작되지 않습니다. 필요 시 설정에서 수동으로 시작하세요."
            }),
        );
    } else if is_system_folder {
        tracing::warn!(
            "System folder detected: auto vector indexing disabled for {}",
            path
        );
        let _ = app_handle.emit(
            "indexing-warning",
            &serde_json::json!({
                "type": "system_folder",
                "folder_path": path,
                "message": "시스템 보호 폴더 인덱싱이 완료되었습니다. 시맨틱(벡터) 검색은 시스템 폴더에서 자동 시작되지 않습니다. 필요 시 설정에서 수동으로 시작하세요."
            }),
        );
    }

    let auto_vector_started = maybe_start_auto_vector(
        &ctx,
        app_handle,
        was_cancelled,
        result.indexed_count,
        is_drive_root || is_system_folder,
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

    // UNC/네트워크 경로 보존 정규화. std::canonicalize 는 `\\server\share` 를
    // `\\?\UNC\server\share` 로 바꿔 DB 에 기록된 감시 경로와 불일치해 재인덱싱 대상이
    // 0건으로 보이는 현상을 일으킨다. dunce 는 `\\server\share\...` 형태를 유지한다.
    let canonical_path = dunce::canonicalize(folder_path)
        .map_err(|e| ApiError::InvalidPath(format!("'{}': {}", path, e)))?;

    // 시스템 폴더 / 드라이브 루트 차단
    crate::constants::validate_watch_path(&canonical_path)
        .map_err(|msg| ApiError::AccessDenied(msg.to_string()))?;

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

    // UNC/네트워크 경로 보존 정규화 — add_folder 와 동일 (dunce 로 통일)
    let canonical_path = dunce::canonicalize(folder_path)
        .map_err(|e| ApiError::InvalidPath(format!("'{}': {}", path, e)))?;

    // 시스템 폴더 / 드라이브 루트 차단 (DB에 남은 오래된 경로 방어)
    crate::constants::validate_watch_path(&canonical_path)
        .map_err(|msg| ApiError::AccessDenied(msg.to_string()))?;

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
