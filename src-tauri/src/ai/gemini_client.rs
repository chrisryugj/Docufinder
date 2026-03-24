use crate::ai::types::*;
use reqwest::Client;
use std::sync::OnceLock;

const BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta/models";
const DEFAULT_MODEL: &str = "gemini-3.1-flash-lite-preview";

static HTTP_CLIENT: OnceLock<Client> = OnceLock::new();

fn client() -> &'static Client {
    HTTP_CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .expect("Failed to create HTTP client")
    })
}

/// 공통 요청 구성
fn build_request(
    system_prompt: Option<&str>,
    user_message: &str,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
) -> GeminiRequest {
    let system_instruction = system_prompt.map(|prompt| Content {
        role: Some("user".into()),
        parts: vec![Part {
            text: prompt.into(),
        }],
    });

    GeminiRequest {
        contents: vec![Content {
            role: Some("user".into()),
            parts: vec![Part {
                text: user_message.into(),
            }],
        }],
        generation_config: Some(GenerationConfig {
            temperature,
            max_output_tokens: max_tokens,
            top_p: None,
        }),
        system_instruction,
    }
}

/// Gemini API에 generateContent 요청
pub async fn generate(
    api_key: &str,
    model: Option<&str>,
    system_prompt: Option<&str>,
    user_message: &str,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
) -> Result<GeminiResponse, GeminiClientError> {
    let model_id = model.unwrap_or(DEFAULT_MODEL);
    let url = format!("{}/{}:generateContent", BASE_URL, model_id);

    let request = build_request(system_prompt, user_message, temperature, max_tokens);

    let response = client()
        .post(&url)
        .header("x-goog-api-key", api_key)
        .json(&request)
        .send()
        .await
        .map_err(|e| GeminiClientError::Network(e.to_string()))?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(GeminiClientError::Api {
            status: status.as_u16(),
            body,
        });
    }

    let gemini_response: GeminiResponse = response
        .json()
        .await
        .map_err(|e| GeminiClientError::Parse(e.to_string()))?;

    // API-level error check
    if let Some(err) = &gemini_response.error {
        return Err(GeminiClientError::Api {
            status: err.code.unwrap_or(500),
            body: err.message.clone().unwrap_or_default(),
        });
    }

    Ok(gemini_response)
}

/// 스트리밍 generateContent (SSE)
#[allow(dead_code)]
pub async fn generate_stream(
    api_key: &str,
    model: Option<&str>,
    system_prompt: Option<&str>,
    user_message: &str,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
) -> Result<reqwest::Response, GeminiClientError> {
    let model_id = model.unwrap_or(DEFAULT_MODEL);
    let url = format!(
        "{}/{}:streamGenerateContent?alt=sse",
        BASE_URL, model_id
    );

    let request = build_request(system_prompt, user_message, temperature, max_tokens);

    let response = client()
        .post(&url)
        .header("x-goog-api-key", api_key)
        .json(&request)
        .send()
        .await
        .map_err(|e| GeminiClientError::Network(e.to_string()))?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(GeminiClientError::Api {
            status: status.as_u16(),
            body,
        });
    }

    Ok(response)
}

/// 응답에서 텍스트 추출
pub fn extract_text(response: &GeminiResponse) -> Option<String> {
    response
        .candidates
        .as_ref()?
        .first()?
        .content
        .parts
        .first()
        .map(|p| p.text.clone())
}

/// 토큰 사용량 추출
pub fn extract_usage(response: &GeminiResponse) -> Option<(u32, u32, u32)> {
    let usage = response.usage_metadata.as_ref()?;
    Some((
        usage.prompt_token_count.unwrap_or(0),
        usage.candidates_token_count.unwrap_or(0),
        usage.total_token_count.unwrap_or(0),
    ))
}

#[derive(Debug, thiserror::Error)]
pub enum GeminiClientError {
    #[error("Network error: {0}")]
    Network(String),
    #[error("API error (status {status}): {body}")]
    Api { status: u16, body: String },
    #[error("Parse error: {0}")]
    Parse(String),
}
