//! LLM 클라이언트 모듈 — RAG + AI 요약용
//!
//! Gemini API 기반. trait 추상화로 추후 다른 프로바이더 확장 가능.

pub mod gemini;

/// LLM 생성 설정
pub struct GenerateConfig {
    pub temperature: f32,
    pub max_tokens: u32,
}

/// LLM 응답
pub struct LlmResponse {
    pub text: String,
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
}

/// LLM 프로바이더 trait
pub trait LlmProvider: Send + Sync {
    /// 비스트리밍 생성 (요약 등)
    fn generate(&self, prompt: &str, config: &GenerateConfig) -> Result<LlmResponse, String>;

    /// 스트리밍 생성 (RAG QA)
    fn generate_stream(
        &self,
        prompt: &str,
        config: &GenerateConfig,
        on_token: &dyn Fn(&str),
    ) -> Result<LlmResponse, String>;
}

// ── 프롬프트 템플릿 ──────────────────────────────

pub const QA_SYSTEM_PROMPT: &str = "\
당신은 사용자의 로컬 문서를 기반으로 질문에 답하는 AI 어시스턴트입니다.

규칙:
1. 아래 제공된 문서 내용만을 근거로 답변하세요.
2. 문서에 없는 내용은 \"제공된 문서에서 관련 내용을 찾을 수 없습니다\"라고 답하세요.
3. 답변은 마크다운으로 작성하세요.
4. 출처를 [파일명, 페이지 N] 형식으로 인용하세요.
5. 핵심을 간결하게 전달하세요.";

pub const SUMMARY_SYSTEM_PROMPT: &str = "\
아래 문서의 핵심 내용을 3~5문장으로 요약하세요.
마크다운으로 작성하세요.";

/// RAG 컨텍스트 최대 길이 (문자 수)
pub const MAX_CONTEXT_CHARS: usize = 15_000;
