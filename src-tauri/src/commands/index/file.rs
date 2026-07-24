//! File Commands - 단일 파일 재인덱싱 ("OCR로 다시 읽기")

use super::*;

/// "OCR로 다시 읽기" 가 엔진 준비를 기다리는 상한. 정상 환경의 워밍업은 수백 ms 지만
/// (이슈 #35 사용자 로그 185ms), 첫 세션·저사양에서는 몇 초까지 걸린다.
const OCR_READY_WAIT: std::time::Duration = std::time::Duration::from_secs(20);

/// OCR 엔진 준비를 **락 밖에서** 기다린다 — 매 폴링마다 read 락을 짧게만 잡는다.
async fn wait_ocr_ready(
    state: &State<'_, RwLock<AppContainer>>,
    timeout: std::time::Duration,
) -> Option<Arc<crate::ocr::OcrEngine>> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        // guard 를 await 너머로 들고 가지 않도록 블록 안에서 끝낸다.
        let ready = { state.read().ok().and_then(|c| c.ocr_engine_if_ready()) };
        if ready.is_some() {
            return ready;
        }
        if std::time::Instant::now() >= deadline {
            tracing::warn!("OCR 엔진 준비 대기 시간 초과 ({}초)", timeout.as_secs());
            return None;
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
}

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
    let (db_path, ocr_ready, vector_index) = {
        let container = state.read()?;
        (
            container.db_path.clone(),
            container.ocr_engine_if_ready(),
            container.get_vector_index().ok(),
        )
    };

    // 엔진이 아직 없으면 워밍업을 걸고 **락 밖에서** 기다린다. 종전에는 위 스코프 안에서
    // `get_ocr_engine()` 으로 동기 초기화했는데, ONNX 세션 빌드가 오래 걸리는 환경(내부망
    // EDR 의 DLL 로드 검사)에서는 read 락을 쥔 채 수십 초가 흘러 다른 커맨드까지 함께
    // 막혔고, ort 가 실패를 panic 으로 알리는 환경에서는 catch_unwind 도 없어 커맨드가
    // 통째로 죽었다(이슈 #35). 사용자가 명시적으로 누른 액션이라 forced 로 재시도한다.
    let ocr_engine = match ocr_ready {
        Some(engine) => Some(engine),
        None => {
            {
                let container = state.read()?;
                container.spawn_ocr_warmup_forced();
            }
            wait_ocr_ready(&state, OCR_READY_WAIT).await
        }
    };

    // 이미지 파일은 자체 OCR 엔진이 있어야 한 글자라도 읽는다. PDF 는 kordoc `--ocr-force`
    // 가 처리하므로 엔진 없이도 진행한다.
    if ocr_engine.is_none() && crate::constants::OCR_IMAGE_EXTENSIONS.contains(&ext.as_str()) {
        return Err(ApiError::IndexingFailed(
            "OCR 엔진을 준비하지 못했습니다. 설정에서 OCR 을 껐다 켜거나, 로그의 [DLL 진단] \
             항목을 확인해 주세요."
                .to_string(),
        ));
    }

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
