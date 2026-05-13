//! OpenAI Chat Completions 호환 클라이언트
//!
//! 사내/오프라인 망에서 운영되는 OpenAI 호환 endpoint (vLLM·Ollama·LiteLLM·Together·
//! Groq·Anthropic openai-proxy·LM Studio·Jan·llama.cpp server 등) 와 통신.
//! `base_url + /chat/completions` 만 만족하면 어떤 backend 든 사용 가능 — 사용자는
//! 설정에서 base_url + api_key + model 만 입력. (이슈 #24 — 회사 내부망 qwen3.6-35b-a3b)
//!
//! ureq 3.x 기반. 비스트리밍 + SSE 스트리밍 지원.

use super::{GenerateConfig, LlmProvider, LlmResponse};
use std::io::BufRead;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const READ_TIMEOUT: Duration = Duration::from_secs(300); // 사내 LLM 은 첫 토큰까지 시간이 더 길 수 있음

pub struct OpenAiCompatibleClient {
    base_url: String,
    api_key: String,
    model: String,
}

impl OpenAiCompatibleClient {
    pub fn new(base_url: String, api_key: String, model: String) -> Self {
        // 사용자가 `/v1` 빠뜨리거나 trailing slash 변형 입력해도 동작하도록 정규화
        let trimmed = base_url.trim().trim_end_matches('/').to_string();
        Self {
            base_url: trimmed,
            api_key,
            model,
        }
    }

    fn endpoint(&self) -> String {
        // OpenAI 표준은 `/v1/chat/completions`. 사용자가 `/v1` 까지 포함해 입력한 경우와
        // base host 만 입력한 경우 모두 처리.
        if self.base_url.ends_with("/v1") || self.base_url.contains("/v1/") {
            format!("{}/chat/completions", self.base_url)
        } else {
            format!("{}/v1/chat/completions", self.base_url)
        }
    }

    fn build_agent() -> ureq::Agent {
        let config = ureq::Agent::config_builder()
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .timeout_recv_body(Some(READ_TIMEOUT))
            .build();
        ureq::Agent::new_with_config(config)
    }

    fn build_request_body(
        &self,
        prompt: &str,
        config: &GenerateConfig,
        stream: bool,
    ) -> serde_json::Value {
        serde_json::json!({
            "model": self.model,
            "messages": [{ "role": "user", "content": prompt }],
            "temperature": config.temperature,
            "max_tokens": config.max_tokens,
            "stream": stream,
        })
    }

    fn map_http_error(e: &ureq::Error) -> String {
        if let ureq::Error::StatusCode(status) = e {
            match *status {
                401 | 403 => "API 키가 유효하지 않습니다. 설정에서 확인해주세요.".to_string(),
                404 => "Endpoint 또는 모델을 찾을 수 없습니다. base URL / 모델 ID 를 확인해주세요."
                    .to_string(),
                429 => "API 요청 한도를 초과했습니다. 잠시 후 다시 시도해주세요.".to_string(),
                500..=599 => "LLM 서버 오류입니다. 잠시 후 다시 시도해주세요.".to_string(),
                _ => format!("API 요청 실패 (HTTP {})", status),
            }
        } else {
            "API 연결 실패 (base URL / 네트워크 상태를 확인해주세요)".to_string()
        }
    }
}

impl LlmProvider for OpenAiCompatibleClient {
    fn generate(&self, prompt: &str, config: &GenerateConfig) -> Result<LlmResponse, String> {
        let url = self.endpoint();
        let agent = Self::build_agent();
        let body = self.build_request_body(prompt, config, false);

        let mut response = agent
            .post(&url)
            .header("Authorization", &format!("Bearer {}", self.api_key))
            .send_json(&body)
            .map_err(|e| Self::map_http_error(&e))?;

        let json: serde_json::Value = response
            .body_mut()
            .read_json()
            .map_err(|e| format!("응답 파싱 실패: {}", e))?;

        if let Some(err) = json["error"]["message"].as_str() {
            return Err(err.to_string());
        }

        let text = json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        if text.is_empty() {
            return Err("응답이 비어 있습니다".to_string());
        }

        let prompt_tokens = json["usage"]["prompt_tokens"].as_u64().map(|v| v as u32);
        let completion_tokens = json["usage"]["completion_tokens"]
            .as_u64()
            .map(|v| v as u32);

        Ok(LlmResponse {
            text,
            prompt_tokens,
            completion_tokens,
        })
    }

    fn generate_stream(
        &self,
        prompt: &str,
        config: &GenerateConfig,
        on_token: &dyn Fn(&str),
        cancel: &AtomicBool,
    ) -> Result<LlmResponse, String> {
        let url = self.endpoint();
        let agent = Self::build_agent();
        let body = self.build_request_body(prompt, config, true);

        let response = agent
            .post(&url)
            .header("Authorization", &format!("Bearer {}", self.api_key))
            .send_json(&body)
            .map_err(|e| Self::map_http_error(&e))?;

        let reader = std::io::BufReader::new(response.into_body().into_reader());
        let mut full_text = String::new();
        let mut prompt_tokens = None;
        let mut completion_tokens = None;
        let mut stream_error: Option<String> = None;

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

            let Some(data) = trimmed.strip_prefix("data: ") else {
                continue;
            };
            if data == "[DONE]" {
                break;
            }
            let Ok(json) = serde_json::from_str::<serde_json::Value>(data) else {
                continue;
            };

            if let Some(err) = json["error"]["message"].as_str() {
                stream_error = Some(err.to_string());
                break;
            }

            // OpenAI streaming chunk: choices[0].delta.content
            if let Some(text) = json["choices"][0]["delta"]["content"].as_str() {
                if !text.is_empty() {
                    on_token(text);
                    full_text.push_str(text);
                }
            }

            // finish_reason: stop / length / content_filter ...
            if let Some(reason) = json["choices"][0]["finish_reason"].as_str() {
                match reason {
                    "stop" | "length" => {}
                    "content_filter" => {
                        stream_error = Some("안전 필터에 의해 응답이 차단되었습니다".to_string());
                        break;
                    }
                    other => {
                        // tool_calls 등 우리가 처리 안 하는 정상 종료 사유는 그냥 종료
                        tracing::debug!("OpenAI finish_reason: {}", other);
                    }
                }
            }

            // 일부 backend (vLLM 등) 는 마지막 chunk 에 usage 동봉
            if let Some(pt) = json["usage"]["prompt_tokens"].as_u64() {
                prompt_tokens = Some(pt as u32);
            }
            if let Some(ct) = json["usage"]["completion_tokens"].as_u64() {
                completion_tokens = Some(ct as u32);
            }
        }

        if let Some(e) = stream_error {
            return Err(e);
        }

        if full_text.is_empty() && !cancel.load(Ordering::Relaxed) {
            return Err("응답이 생성되지 않았습니다".to_string());
        }

        Ok(LlmResponse {
            text: full_text,
            prompt_tokens,
            completion_tokens,
        })
    }
}
