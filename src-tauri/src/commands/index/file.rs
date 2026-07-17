//! File Commands - 단일 파일 재인덱싱 ("OCR로 다시 읽기")

use super::*;

/// 단일 파일 재인덱싱 결과
#[derive(Debug, Serialize)]
pub struct ReindexFileResult {
    pub success: bool,
    pub chunks_count: usize,
    pub message: String,
}

/// 단일 파일을 강제 OCR 로 재인덱싱 — 검색결과/미리보기의 "OCR로 다시 읽기".
///
/// PDF 는 kordoc `--ocr-force`(내장 PP-OCRv5, 전 페이지 재인식 — 첫 사용 시 모델
/// ~18MB 자동 다운로드)로, 이미지 파일은 자체 OCR 엔진으로 다시 읽는다.
/// 전역 OCR 토글과 무관하게 동작한다 (사용자의 명시적 요청이므로).
#[tauri::command]
pub async fn reindex_file(
    path: String,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<ReindexFileResult> {
    let file_path = PathBuf::from(&path);
    if !file_path.is_file() {
        return Err(ApiError::PathNotFound(path));
    }

    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if !crate::constants::SUPPORTED_EXTENSIONS.contains(&ext.as_str())
        && !crate::constants::OCR_IMAGE_EXTENSIONS.contains(&ext.as_str())
    {
        return Err(ApiError::Validation(format!(
            "지원하지 않는 파일 형식: {ext}"
        )));
    }

    // lock 스코프 최소화 — 무거운 파싱 전에 필요한 것만 추출
    let (db_path, ocr_engine, vector_index) = {
        let container = state.read()?;
        (
            container.db_path.clone(),
            container.get_ocr_engine().ok(),
            container.get_vector_index().ok(),
        )
    };

    tracing::info!("Reindexing single file (force OCR): {}", path);
    let result = tauri::async_runtime::spawn_blocking(move || {
        let conn = crate::db::get_connection(&db_path)
            .map_err(|e| ApiError::DatabaseConnection(e.to_string()))?;
        crate::indexer::pipeline::index_file_fts_only_no_tx_opts(
            &conn,
            &file_path,
            ocr_engine.as_deref(),
            vector_index.as_deref(),
            true,
        )
        .map_err(|e| ApiError::IndexingFailed(e.to_string()))
    })
    .await
    .map_err(|e| ApiError::IndexingFailed(format!("재인덱싱 작업 실패: {e}")))??;

    refresh_filename_cache(&state);

    Ok(ReindexFileResult {
        success: true,
        chunks_count: result.chunks_count,
        message: format!("OCR 재인덱싱 완료 ({}개 청크)", result.chunks_count),
    })
}
