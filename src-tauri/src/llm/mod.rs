//! LLM 클라이언트 모듈 — RAG + AI 요약용
//!
//! Gemini API 기반. trait 추상화로 추후 다른 프로바이더 확장 가능.

pub mod gemini;

use std::sync::atomic::AtomicBool;

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

    /// 스트리밍 생성 (RAG QA) — cancel 플래그로 조기 종료 가능
    fn generate_stream(
        &self,
        prompt: &str,
        config: &GenerateConfig,
        on_token: &dyn Fn(&str),
        cancel: &AtomicBool,
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

/// 단일 파일 집중 QA 프롬프트
pub const FILE_QA_SYSTEM_PROMPT: &str = "\
당신은 아래 제공된 단일 문서를 기반으로 질문에 답하는 AI 어시스턴트입니다.

규칙:
1. 아래 제공된 문서 내용만을 근거로 답변하세요.
2. 문서에 없는 내용은 \"이 문서에서 관련 내용을 찾을 수 없습니다\"라고 답하세요.
3. 답변은 마크다운으로 작성하세요.
4. 핵심을 간결하게 전달하세요.";

/// 핵심 3줄 요약
pub const SUMMARY_BRIEF_PROMPT: &str = "\
아래 문서의 핵심 내용을 3줄로 간결하게 요약하세요.
각 줄은 독립적인 핵심 포인트여야 합니다.
마크다운 불릿 포인트로 작성하세요.";

/// 항목별 정리
pub const SUMMARY_STRUCTURED_PROMPT: &str = "\
아래 문서의 내용을 주요 항목별로 체계적으로 정리하세요.
섹션 헤딩과 불릿 포인트를 활용하여 마크다운으로 작성하세요.
중요도 순으로 배열하세요.";

/// 핵심 키워드
pub const SUMMARY_KEYWORDS_PROMPT: &str = "\
아래 문서에서 핵심 키워드와 주요 개념을 10개 이내로 추출하세요.
각 키워드 옆에 한 줄 설명을 추가하세요.
마크다운 테이블 또는 불릿 포인트로 작성하세요.";

/// RAG 컨텍스트 최대 길이 (문자 수)
pub const MAX_CONTEXT_CHARS: usize = 15_000;

/// 요약 유형 → 프롬프트 매핑
pub fn summary_prompt_for_type(summary_type: &str) -> &'static str {
    match summary_type {
        "structured" => SUMMARY_STRUCTURED_PROMPT,
        "keywords" => SUMMARY_KEYWORDS_PROMPT,
        _ => SUMMARY_BRIEF_PROMPT, // "brief" 또는 기본값
    }
}
