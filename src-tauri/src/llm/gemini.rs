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

    /// ureq 에러 → 사용자용 메시지 (API 키/URL 누출 방지)
    fn map_http_error(e: &ureq::Error) -> String {
        if let ureq::Error::StatusCode(status) = e {
            match *status {
                401 | 403 => "API 키가 유효하지 않습니다. 설정에서 확인해주세요.".to_string(),
                429 => "API 요청 한도를 초과했습니다. 잠시 후 다시 시도해주세요.".to_string(),
                404 => "모델을 찾을 수 없습니다. 모델 ID를 확인해주세요.".to_string(),
                500..=599 => "Gemini API 서버 오류입니다. 잠시 후 다시 시도해주세요.".to_string(),
                _ => format!("API 요청 실패 (HTTP {})", status),
            }
        } else {
            "API 연결 실패 (네트워크 상태를 확인해주세요)".to_string()
        }
    }
}

impl LlmProvider for GeminiClient {
    fn generate(&self, prompt: &str, config: &GenerateConfig) -> Result<LlmResponse, String> {
        let url = format!("{}:generateContent", self.base_url());
        let agent = self.build_agent();
        let body = Self::build_request_body(prompt, config);

        let mut response = agent
            .post(&url)
            .header("x-goog-api-key", &self.api_key)
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
        let url = format!("{}:streamGenerateContent?alt=sse", self.base_url());
        let agent = self.build_agent();
        let body = Self::build_request_body(prompt, config);

        let response = agent
            .post(&url)
            .header("x-goog-api-key", &self.api_key)
            .send_json(&body)
            .map_err(|e| Self::map_http_error(&e))?;

        let reader = std::io::BufReader::new(response.into_body().into_reader());
        let mut full_text = String::new();
        let mut prompt_tokens = None;
        let mut completion_tokens = None;
        // 스트리밍 중 감지된 에러/차단 사유 — 우선순위는 후속 토큰보다 높다.
        // Gemini 는 text 를 한 번도 안 내보내고 error/finishReason/blockReason 만 던지는
        // 케이스가 있어서, 이 플래그를 안 보면 "아무 말 없이 성공" 으로 끝난다.
        let mut stream_error: Option<String> = None;
        // "종결" 사유 감지 후에는 추가 파싱을 중단해 정확한 에러 메시지를 유지한다.
        let mut finished = false;

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
                let Ok(json) = serde_json::from_str::<serde_json::Value>(data) else {
                    continue;
                };
                if finished {
                    continue;
                }

                // 1) 서버 에러 — 즉시 중단 사유
                if let Some(err) = json["error"]["message"].as_str() {
                    stream_error = Some(err.to_string());
                    break;
                }

                // 2) 프롬프트 차단 — 응답 자체가 생성되지 않음
                if let Some(block) = json["promptFeedback"]["blockReason"].as_str() {
                    stream_error = Some(format!("프롬프트가 차단되었습니다 ({})", block));
                    break;
                }

                // 3) 정상 텍스트 토큰
                if let Some(text) =
                    json["candidates"][0]["content"]["parts"][0]["text"].as_str()
                {
                    if !text.is_empty() {
                        on_token(text);
                        full_text.push_str(text);
                    }
                }

                // 4) finishReason — STOP/MAX_TOKENS 는 정상 종료, 그 외는 에러.
                //    (일부 SDK 는 STOP 이 아니어도 text 가 일부 생성되었을 수 있으므로
                //     이미 받은 full_text 는 유지하되 "성공" 으로 내보내진 않는다.)
                if let Some(reason) = json["candidates"][0]["finishReason"].as_str() {
                    match reason {
                        "STOP" | "MAX_TOKENS" => {
                            finished = true;
                        }
                        other => {
                            stream_error = Some(match other {
                                "SAFETY" => {
                                    "안전 필터에 의해 응답이 차단되었습니다".to_string()
                                }
                                "RECITATION" => "저작권 정책에 의해 응답이 차단되었습니다"
                                    .to_string(),
                                "BLOCKLIST" | "PROHIBITED_CONTENT" | "SPII" => format!(
                                    "정책에 의해 응답이 차단되었습니다 ({})",
                                    other
                                ),
                                _ => format!("응답이 비정상 종료되었습니다 ({})", other),
                            });
                            finished = true;
                        }
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

        // 취소가 아닌 에러 감지가 있으면 에러로 승격.
        if let Some(e) = stream_error {
            return Err(e);
        }

        // 취소도 에러도 아닌데 토큰이 하나도 없는 경우 → 무증상 실패로 보이지 않도록 명시적 에러.
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
