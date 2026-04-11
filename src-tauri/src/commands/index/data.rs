//! Data Management Commands - clear_all_data, convert_hwp_to_hwpx, get_db_debug_info

use super::*;

/// 모든 데이터 초기화
#[tauri::command]
pub async fn clear_all_data(state: State<'_, RwLock<AppContainer>>) -> ApiResult<()> {
    tracing::info!("Clearing all data...");

    // 1. 파일 감시 모두 중지
    {
        let container = state.read()?;
        if let Ok(wm) = container.get_watch_manager() {
            if let Ok(mut wm) = wm.write() {
                wm.pause();
                tracing::info!("All watchers paused and stopped");
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

        // HwpxConverter.exe 실행 (120초 타임아웃 — 모달 다이얼로그 hang 방지)
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(120),
            tokio::process::Command::new(&converter_exe)
                .arg(canonical.as_os_str())
                .output(),
        )
        .await;

        // 타임아웃 처리
        let result = match result {
            Ok(inner) => inner,
            Err(_) => {
                errors.push(format!("{}: 변환 시간 초과 (120초)", hwp_path));
                failed_count += 1;
                continue;
            }
        };

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
