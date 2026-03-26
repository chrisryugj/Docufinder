//! Index Commands - Thin Layer (Clean Architecture)
//!
//! Tauri commands that delegate to IndexService and FolderService.

use super::settings::VectorIndexingMode;
use crate::application::dto::indexing::{
    AddFolderResult, ConvertHwpResult, FolderStats, IndexStatus, WatchedFolderInfo,
};
use crate::error::{ApiError, ApiResult};
use crate::indexer::pipeline::FtsIndexingProgress;
use crate::indexer::vector_worker::{VectorIndexingProgress, VectorIndexingStatus};
use crate::AppContainer;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tauri::{AppHandle, Emitter, Manager, State};

/// 인덱싱 명령에서 공통으로 필요한 설정/서비스 번들
struct IndexingContext {
    service: crate::application::services::IndexService,
    include_subfolders: bool,
    semantic_available: bool,
    vector_mode: VectorIndexingMode,
    semantic_enabled: bool,
    intensity: super::settings::IndexingIntensity,
    max_file_size_mb: u64,
    db_path: PathBuf,
    exclude_dirs: Vec<String>,
}

/// 단일 lock 스코프에서 인덱싱에 필요한 모든 설정/서비스를 추출
fn extract_indexing_context(state: &State<'_, RwLock<AppContainer>>) -> ApiResult<IndexingContext> {
    let container = state.read()?;
    let settings = container.get_settings();
    let mut dirs: Vec<String> = crate::constants::DEFAULT_EXCLUDED_DIRS
        .iter()
        .map(|s| s.to_string())
        .collect();
    dirs.extend(settings.exclude_dirs.clone());
    Ok(IndexingContext {
        service: container.index_service(),
        include_subfolders: settings.include_subfolders,
        semantic_available: container.is_semantic_available(),
        vector_mode: settings.vector_indexing_mode.clone(),
        semantic_enabled: settings.semantic_search_enabled,
        intensity: settings.indexing_intensity.clone(),
        max_file_size_mb: settings.max_file_size_mb,
        db_path: container.db_path.clone(),
        exclude_dirs: dirs,
    })
}

/// 인덱싱 완료 후 벡터 자동 시작 여부 판단 + 실행
fn maybe_start_auto_vector(
    ctx: &IndexingContext,
    app_handle: AppHandle,
    was_cancelled: bool,
    indexed_count: usize,
    skip_drive_root: bool,
    state: Option<&State<'_, RwLock<AppContainer>>>,
) {
    let auto_vector = ctx.semantic_enabled
        && ctx.semantic_available
        && ctx.vector_mode == VectorIndexingMode::Auto
        && !was_cancelled
        && indexed_count > 0
        && !skip_drive_root;
    if auto_vector {
        // 벡터 인덱싱 중 파일 감시 일시 중지 (DB 동시 접근 방지)
        if let Some(s) = state {
            pause_watching(s);
        }

        let vector_callback = create_vector_progress_callback(app_handle);
        let _ = ctx
            .service
            .start_vector_indexing(Some(vector_callback), Some(ctx.intensity.clone()));
    }
}

/// 파일 감시 일시 중지 (벡터 인덱싱 중 동시 접근 방지)
fn pause_watching(state: &State<'_, RwLock<AppContainer>>) {
    if let Ok(container) = state.read() {
        if let Ok(wm) = container.get_watch_manager() {
            if let Ok(mut wm) = wm.write() {
                let folders = wm.watched_folders();
                wm.unwatch_all();
                tracing::info!("File watching paused ({} folders)", folders.len());
            }
        }
    }
}

/// 파일 감시 재개 (향후 벡터 인덱싱 완료 콜백에서 호출)
#[allow(dead_code)]
fn resume_watching(
    state: &State<'_, RwLock<AppContainer>>,
    db_path: &std::path::PathBuf,
) {
    if let Ok(container) = state.read() {
        if let Ok(wm) = container.get_watch_manager() {
            if let Ok(mut wm) = wm.write() {
                if let Ok(conn) = crate::db::get_connection(db_path) {
                    if let Ok(folders) = crate::db::get_watched_folders(&conn) {
                        let folder_count = folders.len();
                        for folder in folders {
                            let _ = wm.watch(std::path::Path::new(&folder));
                        }
                        tracing::info!("File watching resumed ({} folders)", folder_count);
                    }
                }
            }
        }
    }
}

/// 프론트엔드 이벤트용 인덱싱 진행률
#[derive(Debug, Clone, Serialize)]
struct IndexingProgress {
    phase: String,
    total_files: usize,
    processed_files: usize,
    current_file: Option<String>,
    folder_path: String,
    error: Option<String>,
}

// ============================================
// FTS Progress Callback Helper
// ============================================

fn create_fts_progress_callback(
    app_handle: AppHandle,
) -> Box<dyn Fn(FtsIndexingProgress) + Send + Sync> {
    Box::new(move |progress: FtsIndexingProgress| {
        let legacy_progress = IndexingProgress {
            phase: progress.phase,
            total_files: progress.total_files,
            processed_files: progress.processed_files,
            current_file: progress.current_file,
            folder_path: progress.folder_path,
            error: None,
        };
        if let Err(e) = app_handle.emit("indexing-progress", &legacy_progress) {
            tracing::warn!("Failed to emit progress: {}", e);
        }
    })
}

fn create_vector_progress_callback(
    app_handle: AppHandle,
) -> Arc<dyn Fn(VectorIndexingProgress) + Send + Sync> {
    Arc::new(move |progress: VectorIndexingProgress| {
        if let Err(e) = app_handle.emit("vector-indexing-progress", &progress) {
            tracing::warn!("Failed to emit vector progress: {}", e);
        }
    })
}

// ============================================
// Folder Commands
// ============================================

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

    // 2. 메타데이터 스캔 (파일명 검색 즉시 가능)
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

    // 3. FilenameCache 즉시 갱신 + 메타 스캔에서 수집한 파일 목록 재사용
    let pre_collected = if let Ok(ref meta) = metadata_result {
        refresh_filename_cache(&state);
        tracing::info!(
            "FilenameCache ready: {} files (metadata scan)",
            meta.files_found
        );
        Some(meta.file_paths.clone())
    } else {
        None
    };

    // 4. FTS 인덱싱 (메타 스캔에서 수집한 파일 목록 재사용 → 이중 FS 순회 방지)
    let progress_callback = create_fts_progress_callback(app_handle.clone());
    let result = match ctx
        .service
        .index_folder_fts(
            &canonical_path,
            ctx.include_subfolders,
            Some(progress_callback),
            ctx.max_file_size_mb,
            pre_collected,
            ctx.exclude_dirs.clone(),
        )
        .await
    {
        Ok(r) => r,
        Err(e) => {
            if let Ok(conn) = crate::db::get_connection(&ctx.db_path) {
                let _ = crate::db::set_folder_indexing_status(&conn, &path, "failed");
            }
            return Err(ApiError::from(e));
        }
    };

    // 5. 파일 감시 시작
    start_file_watching(&state, &canonical_path)?;

    // 6. FilenameCache 최종 갱신 (FTS 인덱싱 후)
    refresh_filename_cache(&state);

    let was_cancelled = result.errors.iter().any(|e| e.contains("Cancelled"));

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

    // 드라이브 루트 감지 (C:\, D:\ 등) → 벡터 인덱싱 자동 시작 안 함
    let is_drive_root = {
        let p = canonical_path.to_string_lossy();
        let normalized = p.replace("\\\\?\\", "");
        normalized.len() <= 3 && normalized.chars().nth(1) == Some(':')
    };
    if is_drive_root {
        tracing::info!(
            "Drive root detected: skipping auto vector indexing for {}",
            path
        );
    }

    maybe_start_auto_vector(
        &ctx,
        app_handle,
        was_cancelled,
        result.indexed_count,
        is_drive_root,
        Some(&state),
    );

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

/// 감시 폴더 제거
#[tauri::command]
pub async fn remove_folder(path: String, state: State<'_, RwLock<AppContainer>>) -> ApiResult<()> {
    tracing::info!("Removing folder from watch: {}", path);

    // 파일 감시 중지
    stop_file_watching(&state, Path::new(&path))?;

    // FolderService로 DB/벡터 삭제 위임
    let service = {
        let container = state.read()?;
        container.folder_service()
    };

    let result = service.remove_folder(&path).await.map_err(ApiError::from);

    // FilenameCache 갱신
    refresh_filename_cache(&state);

    result
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
            return Err(ApiError::from(e));
        }
    };

    refresh_filename_cache(&state);

    let was_cancelled = result.errors.iter().any(|e| e.contains("Cancelled"));

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

    maybe_start_auto_vector(&ctx, app_handle, was_cancelled, result.indexed_count, false, Some(&state));

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

/// 미완료 인덱싱 재개 (sync 기반: 추가/수정/삭제 감지)
#[tauri::command]
pub async fn resume_indexing(
    path: String,
    app_handle: AppHandle,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<AddFolderResult> {
    tracing::info!("Syncing folder (resume): {}", path);

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

    // sync 기반 인덱싱 (추가/수정/삭제 감지)
    let progress_callback = create_fts_progress_callback(app_handle.clone());
    let sync_result = match ctx
        .service
        .sync_folder(
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
            return Err(ApiError::from(e));
        }
    };

    refresh_filename_cache(&state);
    let _ = start_file_watching(&state, &canonical_path);

    let was_cancelled = sync_result.errors.iter().any(|e| e.contains("Cancelled"));

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
    let indexed_count = sync_result.added + sync_result.modified;
    maybe_start_auto_vector(&ctx, app_handle, was_cancelled, indexed_count, false, Some(&state));

    let message = format!(
        "동기화 완료: +{}개, -{}개, 변경없음 {}개{}",
        sync_result.added,
        sync_result.deleted,
        sync_result.unchanged,
        if sync_result.failed > 0 {
            format!(", {}개 실패", sync_result.failed)
        } else {
            String::new()
        }
    );

    Ok(AddFolderResult {
        success: true,
        indexed_count,
        failed_count: sync_result.failed,
        vectors_count: 0,
        message,
        errors: sync_result.errors,
        hwp_files: vec![],
        ocr_image_count: 0, // sync에서는 별도 추적 안 함
    })
}

// ============================================
// Index Status Commands
// ============================================

/// 인덱스 상태 조회
#[tauri::command]
pub async fn get_index_status(state: State<'_, RwLock<AppContainer>>) -> ApiResult<IndexStatus> {
    let (service, model_available) = {
        let container = state.read()?;
        (container.index_service(), container.is_semantic_available())
    };
    let mut status = service.get_status().await.map_err(ApiError::from)?;
    // OnceCell 초기화 여부가 아닌 모델 파일 존재 여부로 판단
    status.semantic_available = model_available;
    Ok(status)
}

/// 벡터 인덱싱 상태 조회
#[tauri::command]
pub async fn get_vector_indexing_status(
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<VectorIndexingStatus> {
    let service = {
        let container = state.read()?;
        container.index_service()
    };
    service.get_vector_status().map_err(ApiError::from)
}

// ============================================
// Manual Vector Indexing
// ============================================

/// 수동 벡터 인덱싱 시작
#[tauri::command]
pub async fn start_vector_indexing(
    app_handle: AppHandle,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<()> {
    tracing::info!("Manual vector indexing requested");

    let (service, semantic_enabled, intensity) = {
        let container = state.read()?;
        let settings = container.get_settings();
        (
            container.index_service(),
            settings.semantic_search_enabled,
            settings.indexing_intensity.clone(),
        )
    };

    if !semantic_enabled {
        return Err(ApiError::SemanticSearchDisabled);
    }

    let vector_callback = create_vector_progress_callback(app_handle);
    service
        .start_vector_indexing(Some(vector_callback), Some(intensity))
        .map_err(ApiError::from)
}

// ============================================
// Cancel Commands
// ============================================

/// 인덱싱 취소
#[tauri::command]
pub async fn cancel_indexing(state: State<'_, RwLock<AppContainer>>) -> ApiResult<()> {
    tracing::info!("Cancelling indexing...");
    let service = {
        let container = state.read()?;
        container.index_service()
    };
    service.cancel_indexing();
    Ok(())
}

/// 벡터 인덱싱 취소
#[tauri::command]
pub async fn cancel_vector_indexing(state: State<'_, RwLock<AppContainer>>) -> ApiResult<()> {
    tracing::info!("Cancelling vector indexing...");
    let service = {
        let container = state.read()?;
        container.index_service()
    };
    service.cancel_vector_indexing().map_err(ApiError::from)
}

// ============================================
// Data Management Commands
// ============================================

/// 모든 데이터 초기화
#[tauri::command]
pub async fn clear_all_data(state: State<'_, RwLock<AppContainer>>) -> ApiResult<()> {
    tracing::info!("Clearing all data...");

    // 1. 파일 감시 모두 중지
    {
        let container = state.read()?;
        if let Ok(wm) = container.get_watch_manager() {
            if let Ok(mut wm) = wm.write() {
                wm.unwatch_all();
                tracing::info!("All watchers stopped");
            }
        }
    }

    // 2. 인덱싱 취소 + 벡터 인덱싱 취소 + 워커 정지 대기
    {
        let container = state.read()?;
        let service = container.index_service();

        // FTS 인덱싱 취소
        service.cancel_indexing();
        tracing::info!("FTS indexing cancelled");

        // 벡터 인덱싱 취소 (clear_all에서도 하지만, 사전에 신호 보내기)
        if container.get_vector_index().is_ok() {
            let _ = service.cancel_vector_indexing();
            tracing::info!("Vector indexing cancelled");
        }
    }

    // 잠시 대기 (워커들이 정지될 시간 확보) - 최대 2초
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // 3. 모든 데이터 클리어
    let (service, filename_cache) = {
        let container = state.read()?;
        (container.index_service(), container.get_filename_cache())
    };
    let result = service.clear_all().map_err(ApiError::from);

    filename_cache.clear();
    tracing::info!("FilenameCache cleared");

    result
}

// ============================================
// HWP Conversion Commands
// ============================================

/// HwpxConverter.exe 경로 탐색 (설치된 변환기)
fn find_hwpx_converter() -> Option<std::path::PathBuf> {
    let candidates = [
        r"C:\Program Files (x86)\HNC\HwpxConverter\HwpxConverter.exe",
        r"C:\Program Files\HNC\HwpxConverter\HwpxConverter.exe",
    ];
    for path in &candidates {
        let p = std::path::PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

/// HWP → HWPX 변환 (HwpxConverter.exe 사용)
#[tauri::command]
pub async fn convert_hwp_to_hwpx(
    paths: Vec<String>,
    app: AppHandle,
) -> ApiResult<ConvertHwpResult> {
    tracing::info!("Converting {} HWP files to HWPX...", paths.len());

    // HwpxConverter.exe 찾기
    let converter_exe = match find_hwpx_converter() {
        Some(exe) => exe,
        None => {
            // 미설치 → 번들된 설치 파일 경로 반환
            let resource_dir = app.path().resource_dir().map_err(|e| {
                ApiError::IndexingFailed(format!("Failed to get resource dir: {}", e))
            })?;
            let installer_path = resource_dir.join("HwpxConverterSetup.exe");
            let installer_str = if installer_path.exists() {
                Some(installer_path.to_string_lossy().to_string())
            } else {
                None
            };
            return Ok(ConvertHwpResult {
                success_count: 0,
                failed_count: 0,
                converted_paths: vec![],
                errors: vec!["HWPX 변환기가 설치되지 않았습니다.".to_string()],
                installer_path: installer_str,
            });
        }
    };

    let total = paths.len();
    let mut success_count = 0;
    let mut failed_count = 0;
    let mut converted_paths = Vec::new();
    let mut errors = Vec::new();

    for (i, hwp_path) in paths.iter().enumerate() {
        let hwp = Path::new(hwp_path);

        // 경로 정규화 + 존재 확인 (path traversal 방지)
        let canonical = match hwp.canonicalize() {
            Ok(p) => p,
            Err(_) => {
                errors.push(format!("Invalid path: {}", hwp_path));
                failed_count += 1;
                continue;
            }
        };
        if !canonical.is_file() {
            errors.push(format!("File not found: {}", hwp_path));
            failed_count += 1;
            continue;
        }
        // 확장자 검증
        let ext = canonical
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if ext != "hwp" {
            errors.push(format!("Not a HWP file: {}", hwp_path));
            failed_count += 1;
            continue;
        }

        let hwpx_path = canonical.with_extension("hwpx");

        // 이미 변환된 파일 건너뛰기
        if hwpx_path.exists() {
            success_count += 1;
            converted_paths.push(hwpx_path.to_string_lossy().to_string());
            continue;
        }

        // 진행률 이벤트
        let _ = app.emit(
            "hwp-convert-progress",
            serde_json::json!({
                "total": total,
                "current": i + 1,
                "current_file": hwp_path,
            }),
        );

        // HwpxConverter.exe 실행 (파일 경로를 인수로 전달)
        let result = tokio::process::Command::new(&converter_exe)
            .arg(canonical.as_os_str())
            .output()
            .await;

        match result {
            Ok(output) if hwpx_path.exists() => {
                success_count += 1;
                converted_paths.push(hwpx_path.to_string_lossy().to_string());
                tracing::info!("Converted: {} → .hwpx", hwp_path);
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let err_msg = format!(
                    "{}: 변환 실패 (exit: {:?}, {})",
                    hwp_path,
                    output.status.code(),
                    stderr.trim()
                );
                tracing::warn!("HWP conversion failed: {}", err_msg);
                errors.push(err_msg);
                failed_count += 1;
            }
            Err(e) => {
                let err_msg = format!("{}: {}", hwp_path, e);
                tracing::error!("HwpxConverter execution failed: {}", err_msg);
                errors.push(err_msg);
                failed_count += 1;
            }
        }
    }

    // 완료 이벤트
    let _ = app.emit(
        "hwp-convert-progress",
        serde_json::json!({
            "total": total,
            "current": total,
            "done": true,
        }),
    );

    tracing::info!(
        "HWP conversion complete: {} success, {} failed",
        success_count,
        failed_count
    );

    Ok(ConvertHwpResult {
        success_count,
        failed_count,
        converted_paths,
        errors,
        installer_path: None,
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

// ============================================
// Private Helpers
// ============================================

fn start_file_watching(state: &State<'_, RwLock<AppContainer>>, path: &Path) -> ApiResult<()> {
    let container = state.read()?;
    if let Ok(wm) = container.get_watch_manager() {
        if let Ok(mut wm) = wm.write() {
            if let Err(e) = wm.watch(path) {
                tracing::warn!("Failed to start watching {}: {}", path.display(), e);
            }
        }
    }
    Ok(())
}

fn stop_file_watching(state: &State<'_, RwLock<AppContainer>>, path: &Path) -> ApiResult<()> {
    let container = state.read()?;
    if let Ok(wm) = container.get_watch_manager() {
        if let Ok(mut wm) = wm.write() {
            let _ = wm.unwatch(path);
        }
    }
    Ok(())
}

/// FilenameCache 갱신 (인덱싱 완료 후 호출)
fn refresh_filename_cache(state: &State<'_, RwLock<AppContainer>>) {
    if let Ok(container) = state.read() {
        match container.load_filename_cache() {
            Ok(count) => tracing::info!("FilenameCache refreshed: {} entries", count),
            Err(e) => tracing::warn!("Failed to refresh FilenameCache: {}", e),
        }
    }
}

fn build_result_message(
    result: &crate::indexer::pipeline::FolderIndexResult,
    was_cancelled: bool,
    semantic_available: bool,
    is_reindex: bool,
) -> String {
    let action = if is_reindex {
        "재인덱싱"
    } else {
        "인덱싱"
    };

    if was_cancelled {
        format!("{}이 취소되었습니다", action)
    } else if result.failed_count > 0 {
        format!(
            "{} 파일 {} 완료, {} 실패{}",
            result.indexed_count,
            action,
            result.failed_count,
            if semantic_available {
                " (시맨틱 검색 준비 중...)"
            } else {
                ""
            }
        )
    } else if semantic_available {
        format!(
            "{} 파일 {} 완료 (시맨틱 검색 준비 중...)",
            result.indexed_count, action
        )
    } else {
        format!("{} 파일 {} 완료", result.indexed_count, action)
    }
}

fn log_indexing_errors(errors: &[String]) {
    if !errors.is_empty() {
        tracing::warn!("Indexing errors ({}):", errors.len());
        for (i, err) in errors.iter().take(10).enumerate() {
            tracing::warn!("  {}: {}", i + 1, err);
        }
        if errors.len() > 10 {
            tracing::warn!("  ... and {} more errors", errors.len() - 10);
        }
    }
}
