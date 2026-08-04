//! lite(내부망) 빌드용 AI 커맨드 스텁.
//!
//! lite 빌드는 HTTP 클라이언트(`ureq`) 자체를 링크하지 않으므로 LLM 호출 경로가 없다.
//! 커맨드 이름만 남겨 `generate_handler!` 목록을 두 빌드가 공유하게 하고, 호출되면
//! 이유를 밝히는 오류를 돌려준다. 프론트는 애초에 AI UI 를 렌더링하지 않으므로
//! 정상 경로에서는 호출되지 않는다.

use crate::application::dto::search::AiAnalysis;
use crate::error::{ApiError, ApiResult};

const UNAVAILABLE: &str = "이 설치본(내부망 전용)에는 AI 기능이 포함되어 있지 않습니다.";

#[tauri::command]
pub async fn ask_ai() -> ApiResult<()> {
    Err(ApiError::AiError(UNAVAILABLE.to_string()))
}

#[tauri::command]
pub async fn ask_ai_file() -> ApiResult<()> {
    Err(ApiError::AiError(UNAVAILABLE.to_string()))
}

#[tauri::command]
pub async fn summarize_ai() -> ApiResult<AiAnalysis> {
    Err(ApiError::AiError(UNAVAILABLE.to_string()))
}
