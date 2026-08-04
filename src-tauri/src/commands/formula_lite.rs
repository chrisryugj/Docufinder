//! lite(내부망) 빌드용 수식 OCR 모델 커맨드 스텁.
//!
//! 원본 커맨드는 kordoc 사이드카에 `check-formula-models` 를 시켜 HuggingFace 에서
//! Pix2Text 모델 ~155MB 를 내려받게 한다. "실행 중인 앱이 자식 프로세스를 띄워 외부에서
//! 바이너리를 받는" 형태는 EDR 이 dropper 로 분류하는 전형이라 lite 에서는 경로째 뺀다.

use serde::{Deserialize, Serialize};

use crate::error::{ApiError, ApiResult};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModelStatus {
    pub name: String,
    pub filename: String,
    #[serde(rename = "sizeMb")]
    pub size_mb: u64,
    pub exists: bool,
    pub verified: bool,
    pub path: String,
    #[serde(rename = "invalidReason", default)]
    pub invalid_reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FormulaModelsStatus {
    #[serde(rename = "modelsDir")]
    pub models_dir: String,
    #[serde(rename = "allReady")]
    pub all_ready: bool,
    pub models: Vec<ModelStatus>,
}

/// 항상 "준비된 모델 없음" — 프론트가 수식 OCR UI 를 숨기는 근거로 쓴다.
#[tauri::command]
pub async fn get_formula_models_status() -> ApiResult<FormulaModelsStatus> {
    Ok(FormulaModelsStatus {
        models_dir: String::new(),
        all_ready: false,
        models: Vec::new(),
    })
}

#[tauri::command]
pub async fn download_formula_models() -> ApiResult<()> {
    Err(ApiError::CommandFailed(
        "이 설치본(내부망 전용)은 모델을 내려받지 않습니다.".to_string(),
    ))
}
