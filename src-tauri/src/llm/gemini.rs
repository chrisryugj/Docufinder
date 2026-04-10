//! Gemini API 클라이언트
//!
//! ureq 3.x 기반. 비스트리밍 + SSE 스트리밍 지원.

use super::{GenerateConfig, LlmProvider, LlmResponse};
use std::io::BufRead;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const READ_TIMEOUT: Duration = Duration::from_secs(120);

/// Gemini API 클라이언트 (stateless — 요청마다 Settings에서 생성)
pub struct GeminiClient {
    api_key: String,
    model: String,
}

impl GeminiClient {
    pub fn new(api_key: String, model: String) -> Self {
        Self { api_key, model }
    }

    fn base_url(&self) -> String {
        format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}",
            self.model
        )
    }

    fn build_agent(&self) -> ureq::Agent {
        let config = ureq::Agent::config_builder()
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .timeout_recv_body(Some(READ_TIMEOUT))
            .build();
        ureq::Agent::new_with_config(config)
    }

    fn build_request_body(prompt: &str, config: &GenerateConfig) -> serde_json::Value {
        serde_json::json!({
            "contents": [{
                "parts": [{ "text": prompt }]
            }],
            "generationConfig": {
                "temperature": config.temperature,
                "maxOutputTokens": config.max_tokens,
            }
        })
    }

    /// JSON 응답에서 텍스트 + 토큰 사용량 추출
    fn parse_response(json: &serde_json::Value) -> Result<LlmResponse, String> {
        let text = json["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .unwrap_or("")
            .to_string();

        if text.is_empty() {
            if let Some(err) = json["error"]["message"].as_str() {
                return Err(err.to_string());
            }
            if let Some(reason) = json["candidates"][0]["finishReason"].as_str() {
                if reason == "SAFETY" {
                    return Err("안전 필터에 의해 응답이 차단되었습니다".to_string());
                }
            }
        }

        let prompt_tokens = json["usageMetadata"]["promptTokenCount"]
            .as_u64()
            .map(|v| v as u32);
        let completion_tokens = json["usageMetadata"]["candidatesTokenCount"]
            .as_u64()
            .map(|v| v as u32);

        Ok(LlmResponse {
            text,
            prompt_tokens,
            completion_tokens,
        })
    }

    fn map_http_error(e: &ureq::Error) -> String {
        if let ureq::Error::StatusCode(status) = e {
            match *status {
                401 | 403 => {
                    "API 키가 유효하지 않습니다. 설정에서 확인해주세요.".to_string()
                }
                429 => "API 요청 한도를 초과했습니다. 잠시 후 다시 시도해주세요.".to_string(),
                404 => "모델을 찾을 수 없습니다. 모델 ID를 확인해주세요.".to_string(),
                500..=599 => {
                    "Gemini API 서버 오류입니다. 잠시 후 다시 시도해주세요.".to_string()
                }
                _ => format!("API 요청 실패 (HTTP {})", status),
            }
        } else {
            format!("API 연결 실패: {}", e)
        }
    }
}

impl LlmProvider for GeminiClient {
    fn generate(&self, prompt: &str, config: &GenerateConfig) -> Result<LlmResponse, String> {
        let url = format!("{}:generateContent?key={}", self.base_url(), self.api_key);
        let agent = self.build_agent();
        let body = Self::build_request_body(prompt, config);

        let mut response = agent
            .post(&url)
            .send_json(&body)
            .map_err(|e| Self::map_http_error(&e))?;

        let json: serde_json::Value = response
            .body_mut()
            .read_json()
            .map_err(|e| format!("응답 파싱 실패: {}", e))?;

        Self::parse_response(&json)
    }

    fn generate_stream(
        &self,
        prompt: &str,
        config: &GenerateConfig,
        on_token: &dyn Fn(&str),
        cancel: &AtomicBool,
    ) -> Result<LlmResponse, String> {
        let url = format!(
            "{}:streamGenerateContent?alt=sse&key={}",
            self.base_url(),
            self.api_key
        );
        let agent = self.build_agent();
        let body = Self::build_request_body(prompt, config);

        let response = agent
            .post(&url)
            .send_json(&body)
            .map_err(|e| Self::map_http_error(&e))?;

        let reader = std::io::BufReader::new(response.into_body().into_reader());
        let mut full_text = String::new();
        let mut prompt_tokens = None;
        let mut completion_tokens = None;

        for line in reader.lines() {
            if cancel.load(Ordering::Relaxed) {
                tracing::debug!("LLM 스트리밍 취소됨");
                break;
            }

            let line = line.map_err(|e| format!("스트림 읽기 실패: {}", e))?;
            let trimmed = line.trim();

            if trimmed.is_empty() || trimmed.starts_with(':') {
                continue;
            }

            if let Some(data) = trimmed.strip_prefix("data: ") {
                if data == "[DONE]" {
                    break;
                }
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                    if let Some(text) =
                        json["candidates"][0]["content"]["parts"][0]["text"].as_str()
                    {
                        if !text.is_empty() {
                            on_token(text);
                            full_text.push_str(text);
                        }
                    }
                    if let Some(pt) = json["usageMetadata"]["promptTokenCount"].as_u64() {
                        prompt_tokens = Some(pt as u32);
                    }
                    if let Some(ct) = json["usageMetadata"]["candidatesTokenCount"].as_u64() {
                        completion_tokens = Some(ct as u32);
                    }
                }
            }
        }

        Ok(LlmResponse {
            text: full_text,
            prompt_tokens,
            completion_tokens,
        })
    }
}
