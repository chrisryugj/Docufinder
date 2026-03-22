use crate::application::dto::search::{AiAnalysis, SearchResult};
use crate::application::services::RagService;
use crate::error::{ApiError, ApiResult};
use crate::AppContainer;
use std::sync::RwLock;
use tauri::State;

/// AI RAG 분석 요청
#[tauri::command]
pub async fn ask_ai(
    query: String,
    search_results: Vec<SearchResult>,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<AiAnalysis> {
    if query.trim().is_empty() {
        return Err(ApiError::Validation("질문이 비어있습니다".into()));
    }

    if search_results.is_empty() {
        return Err(ApiError::Validation(
            "검색 결과가 없어 AI 분석을 수행할 수 없습니다".into(),
        ));
    }

    // 설정에서 API 키/모델/온도 가져오기
    let (api_key, model, temperature, max_tokens) = {
        let container = state.read()?;
        let settings = container.get_settings();

        let api_key = settings.ai_api_key.clone().unwrap_or_default();
        let model = if settings.ai_model.is_empty() {
            None
        } else {
            Some(settings.ai_model.clone())
        };
        let temperature = settings.ai_temperature;
        let max_tokens = settings.ai_max_tokens;
        (api_key, model, temperature, max_tokens)
    };

    RagService::analyze(
        &api_key,
        &query,
        &search_results,
        model.as_deref(),
        Some(temperature),
        Some(max_tokens),
    )
    .await
    .map_err(ApiError::from)
}
