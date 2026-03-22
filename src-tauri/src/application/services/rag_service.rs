use crate::ai::gemini_client;
use crate::application::dto::search::{AiAnalysis, SearchResult, TokenUsage};
use crate::application::errors::AppError;
use std::time::Instant;

const DEFAULT_MODEL: &str = "gemini-3.1-flash-lite-preview";
const MAX_CONTEXT_RESULTS: usize = 5;
const MAX_SNIPPET_CHARS: usize = 800;

const SYSTEM_PROMPT: &str = r#"당신은 로컬 문서 검색 앱의 AI 어시스턴트입니다.
사용자의 질문에 대해 제공된 문서 검색 결과를 바탕으로 답변합니다.

규칙:
- 제공된 문서 내용만 사용하여 답변합니다. 없는 내용은 추측하지 마세요.
- 답변은 한국어로, 간결하고 명확하게 작성합니다.
- 관련 문서가 없으면 "검색 결과에서 관련 내용을 찾을 수 없습니다"라고 답합니다.
- 마크다운 형식으로 답변합니다 (제목, 목록 등).
- 답변 끝에 참조 문서를 언급하지 않아도 됩니다 (시스템이 별도 표시)."#;

pub struct RagService;

impl RagService {
    /// 검색 결과 기반 AI 분석
    pub async fn analyze(
        api_key: &str,
        query: &str,
        search_results: &[SearchResult],
        model: Option<&str>,
        temperature: Option<f32>,
        max_tokens: Option<u32>,
    ) -> Result<AiAnalysis, AppError> {
        if api_key.is_empty() {
            return Err(AppError::AiError("API 키가 설정되지 않았습니다".into()));
        }

        let start = Instant::now();
        let model_id = model.unwrap_or(DEFAULT_MODEL);

        // 상위 N개 결과로 컨텍스트 생성
        let context = build_context(search_results);
        let source_files: Vec<String> = search_results
            .iter()
            .take(MAX_CONTEXT_RESULTS)
            .map(|r| r.file_path.clone())
            .collect();

        let user_message = format!(
            "## 사용자 질문\n{}\n\n## 검색된 문서 내용\n{}",
            query, context
        );

        let response = gemini_client::generate(
            api_key,
            Some(model_id),
            Some(SYSTEM_PROMPT),
            &user_message,
            temperature.or(Some(0.3)),
            max_tokens.or(Some(2048)),
        )
        .await
        .map_err(|e| AppError::AiError(e.to_string()))?;

        let answer = gemini_client::extract_text(&response)
            .unwrap_or_else(|| "AI 응답을 생성하지 못했습니다.".into());

        let tokens_used = gemini_client::extract_usage(&response).map(|(p, c, t)| TokenUsage {
            prompt_tokens: p,
            completion_tokens: c,
            total_tokens: t,
        });

        Ok(AiAnalysis {
            answer,
            source_files,
            processing_time_ms: start.elapsed().as_millis() as u64,
            model: model_id.to_string(),
            tokens_used,
        })
    }
}

/// 검색 결과를 Gemini 프롬프트용 컨텍스트로 변환
fn build_context(results: &[SearchResult]) -> String {
    let mut context = String::new();
    for (i, result) in results.iter().take(MAX_CONTEXT_RESULTS).enumerate() {
        let content = if result.full_content.len() > MAX_SNIPPET_CHARS {
            &result.full_content[..result.full_content.char_indices()
                .nth(MAX_SNIPPET_CHARS)
                .map(|(idx, _)| idx)
                .unwrap_or(result.full_content.len())]
        } else {
            &result.full_content
        };

        context.push_str(&format!(
            "### 문서 {} — {}\n{}\n\n",
            i + 1,
            result.file_name,
            content,
        ));
    }
    context
}
