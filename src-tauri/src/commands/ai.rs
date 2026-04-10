//! AI Commands — RAG 질문 + AI 요약
//!
//! - ask_ai: 하이브리드 검색 → 컨텍스트 빌드 → LLM 스트리밍 응답
//! - ask_ai_file: 단일 파일 청크 → LLM 스트리밍 응답 (파일 전용 QA)
//! - summarize_ai: 파일 청크 → LLM 요약 (유형 선택 가능)

use crate::application::dto::search::{AiAnalysis, TokenUsage};
use crate::db;
use crate::error::{ApiError, ApiResult};
use crate::llm::gemini::GeminiClient;
use crate::llm::{
    summary_prompt_for_type, GenerateConfig, LlmProvider, MAX_CONTEXT_CHARS,
    QA_SYSTEM_PROMPT, FILE_QA_SYSTEM_PROMPT,
};
use crate::search::nl_query::NlQueryParser;
use crate::AppContainer;
use std::sync::RwLock;
use std::time::Instant;
use tauri::{AppHandle, Emitter, State};

const MAX_QUERY_LEN: usize = 1000;
const RAG_RETRIEVE_LIMIT: usize = 25;

/// 스트리밍 토큰 이벤트 payload (request_id로 요청 구분)
#[derive(serde::Serialize, Clone)]
struct AiTokenEvent {
    request_id: String,
    token: String,
}

/// 스트리밍 완료 이벤트 payload
#[derive(serde::Serialize, Clone)]
struct AiCompleteEvent {
    request_id: String,
    #[serde(flatten)]
    analysis: AiAnalysis,
}

/// 에러 이벤트 payload
#[derive(serde::Serialize, Clone)]
struct AiErrorEvent {
    request_id: String,
    error: String,
}

/// Settings에서 GeminiClient 생성
fn build_llm_client(container: &AppContainer) -> ApiResult<GeminiClient> {
    let settings = container.get_settings();
    if !settings.ai_enabled {
        return Err(ApiError::AiError(
            "AI 기능이 비활성화되어 있습니다. 설정에서 활성화해주세요.".to_string(),
        ));
    }
    let api_key = settings.ai_api_key.filter(|k| !k.is_empty()).ok_or_else(|| {
        ApiError::AiError("API 키가 설정되지 않았습니다. 설정 > AI에서 입력해주세요.".to_string())
    })?;
    Ok(GeminiClient::new(api_key, settings.ai_model))
}

/// 검색 결과 → RAG 컨텍스트 문자열 (UTF-8 char boundary 안전)
fn build_rag_context(
    results: &[crate::application::dto::search::SearchResult],
) -> (String, Vec<String>) {
    let mut context = String::new();
    let mut source_files: Vec<String> = Vec::new();
    let mut seen_files = std::collections::HashSet::new();

    for r in results {
        if seen_files.insert(r.file_path.clone()) {
            source_files.push(r.file_path.clone());
        }

        if context.len() >= MAX_CONTEXT_CHARS {
            break;
        }

        let header = if let Some(page) = r.page_number {
            format!("[문서: {}, 페이지 {}]", r.file_name, page)
        } else if let Some(ref hint) = r.location_hint {
            format!("[문서: {}, {}]", r.file_name, hint)
        } else {
            format!("[문서: {}]", r.file_name)
        };

        context.push_str(&header);
        context.push('\n');

        let content = if r.full_content.is_empty() {
            &r.content_preview
        } else {
            &r.full_content
        };

        let remaining = MAX_CONTEXT_CHARS.saturating_sub(context.len());
        if content.len() > remaining {
            let mut end = remaining;
            while end > 0 && !content.is_char_boundary(end) {
                end -= 1;
            }
            context.push_str(&content[..end]);
        } else {
            context.push_str(content);
        }
        context.push_str("\n\n");
    }

    (context, source_files)
}

/// 파일 청크 텍스트 로드 (공통 헬퍼)
fn load_file_chunks_text(conn: &rusqlite::Connection, file_path: &str) -> Result<String, String> {
    let chunk_ids = db::get_chunk_ids_for_path(conn, file_path)
        .map_err(|e| format!("청크 조회 실패: {}", e))?;

    if chunk_ids.is_empty() {
        return Err("이 파일의 인덱스가 없습니다. 폴더를 인덱싱해주세요.".to_string());
    }

    let chunks = db::get_chunks_by_ids(conn, &chunk_ids)
        .map_err(|e| format!("청크 로드 실패: {}", e))?;

    let mut sorted = chunks;
    sorted.sort_by_key(|c| c.chunk_index);

    let mut text = String::new();
    for chunk in &sorted {
        if text.len() >= MAX_CONTEXT_CHARS {
            break;
        }
        let remaining = MAX_CONTEXT_CHARS.saturating_sub(text.len());
        if chunk.content.len() > remaining {
            let mut end = remaining;
            while end > 0 && !chunk.content.is_char_boundary(end) {
                end -= 1;
            }
            text.push_str(&chunk.content[..end]);
            break;
        } else {
            text.push_str(&chunk.content);
            text.push('\n');
        }
    }

    Ok(text)
}

/// 스트리밍 결과를 AiAnalysis DTO로 변환 헬퍼
fn to_analysis(response: crate::llm::LlmResponse, source_files: Vec<String>, elapsed: u64) -> AiAnalysis {
    AiAnalysis {
        answer: response.text,
        source_files,
        processing_time_ms: elapsed,
        model: "gemini".to_string(),
        tokens_used: match (response.prompt_tokens, response.completion_tokens) {
            (Some(pt), Some(ct)) => Some(TokenUsage {
                prompt_tokens: pt,
                completion_tokens: ct,
                total_tokens: pt + ct,
            }),
            _ => None,
        },
    }
}

// ── 커맨드 ──────────────────────────────────────────────

/// RAG 질문 (스트리밍) — 전체 인덱스 기반
#[tauri::command]
pub async fn ask_ai(
    query: String,
    folder_scope: Option<String>,
    request_id: String,
    app: AppHandle,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<()> {
    if query.trim().is_empty() {
        return Err(ApiError::Validation("질문을 입력해주세요.".to_string()));
    }
    if query.chars().count() > MAX_QUERY_LEN {
        return Err(ApiError::Validation(format!("질문이 너무 깁니다 (최대 {}자)", MAX_QUERY_LEN)));
    }

    let (client, service, config) = {
        let container = state.read()?;
        let client = build_llm_client(&container)?;
        let service = container.search_service();
        let settings = container.get_settings();
        let config = GenerateConfig {
            temperature: settings.ai_temperature,
            max_tokens: settings.ai_max_tokens,
        };
        (client, service, config)
    };

    let query_clone = query.clone();
    let app_clone = app.clone();
    let rid = request_id.clone();

    tauri::async_runtime::spawn(async move {
        let start = Instant::now();

        // 자연어 질문에서 FTS 키워드 추출 ("얼마야", "뭔가요" 등 의문 표현 제거)
        // NlQueryParser: intent 제거 + 날짜("2026년") 분리 → keywords만 FTS에 사용
        let parsed = NlQueryParser::parse(&query_clone);
        let search_query = if parsed.keywords.is_empty() {
            query_clone.clone()
        } else {
            parsed.keywords.clone()
        };

        tracing::debug!("RAG 검색쿼리: '{}' (원본: '{}')", search_query, query_clone);

        let search_result = service
            .search_hybrid(&search_query, RAG_RETRIEVE_LIMIT, folder_scope.as_deref())
            .await;

        let results = match search_result {
            Ok(resp) => resp.results,
            Err(e) => {
                tracing::error!("RAG 검색 실패: {}", e);
                let _ = app_clone.emit("ai-error", AiErrorEvent { request_id: rid, error: format!("검색 실패: {}", e) });
                return;
            }
        };

        if results.is_empty() {
            let _ = app_clone.emit("ai-error", AiErrorEvent { request_id: rid, error: "관련 문서를 찾을 수 없습니다. 먼저 폴더를 인덱싱해주세요.".to_string() });
            return;
        }

        let (context, source_files) = build_rag_context(&results);
        let prompt = format!(
            "{}\n\n--- 문서 내용 ---\n{}\n--- 질문 ---\n{}",
            QA_SYSTEM_PROMPT, context, query_clone
        );

        let app_for_token = app_clone.clone();
        let rid_for_token = rid.clone();
        let stream_result = tokio::task::spawn_blocking(move || {
            client.generate_stream(&prompt, &config, &|token| {
                let _ = app_for_token.emit("ai-token", AiTokenEvent { request_id: rid_for_token.clone(), token: token.to_string() });
            })
        })
        .await;

        let elapsed = start.elapsed().as_millis() as u64;

        match stream_result {
            Ok(Ok(response)) => {
                let _ = app_clone.emit("ai-complete", AiCompleteEvent { request_id: rid, analysis: to_analysis(response, source_files, elapsed) });
            }
            Ok(Err(e)) => {
                tracing::error!("LLM 생성 실패: {}", e);
                let _ = app_clone.emit("ai-error", AiErrorEvent { request_id: rid, error: e });
            }
            Err(e) => {
                tracing::error!("LLM 태스크 실패: {}", e);
                let _ = app_clone.emit("ai-error", AiErrorEvent { request_id: rid, error: format!("처리 중 오류: {}", e) });
            }
        }
    });

    Ok(())
}

/// 단일 파일 기반 QA (스트리밍) — ai-file-* 이벤트 사용
#[tauri::command]
pub async fn ask_ai_file(
    file_path: String,
    query: String,
    request_id: String,
    app: AppHandle,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<()> {
    if query.trim().is_empty() {
        return Err(ApiError::Validation("질문을 입력해주세요.".to_string()));
    }
    if query.chars().count() > MAX_QUERY_LEN {
        return Err(ApiError::Validation(format!("질문이 너무 깁니다 (최대 {}자)", MAX_QUERY_LEN)));
    }
    if file_path.trim().is_empty() {
        return Err(ApiError::Validation("파일 경로가 비어있습니다.".to_string()));
    }

    let (client, db_path, config) = {
        let container = state.read()?;
        let client = build_llm_client(&container)?;
        let db_path = container.db_path.clone();
        let settings = container.get_settings();
        let config = GenerateConfig {
            temperature: settings.ai_temperature,
            max_tokens: settings.ai_max_tokens,
        };
        (client, db_path, config)
    };

    let app_clone = app.clone();
    let file_path_clone = file_path.clone();
    let rid = request_id.clone();

    tauri::async_runtime::spawn(async move {
        let start = Instant::now();

        // 청크 로드
        let content_result = tokio::task::spawn_blocking(move || {
            let conn = db::get_connection(&db_path)
                .map_err(|e| format!("DB 연결 실패: {}", e))?;
            load_file_chunks_text(&conn, &file_path_clone)
        })
        .await;

        let content = match content_result {
            Ok(Ok(text)) => text,
            Ok(Err(e)) => {
                let _ = app_clone.emit("ai-file-error", AiErrorEvent { request_id: rid, error: e });
                return;
            }
            Err(e) => {
                let _ = app_clone.emit("ai-file-error", AiErrorEvent { request_id: rid, error: format!("태스크 실패: {}", e) });
                return;
            }
        };

        let prompt = format!(
            "{}\n\n--- 문서 내용 ---\n{}\n--- 질문 ---\n{}",
            FILE_QA_SYSTEM_PROMPT, content, query
        );

        let app_for_token = app_clone.clone();
        let rid_for_token = rid.clone();
        let stream_result = tokio::task::spawn_blocking(move || {
            client.generate_stream(&prompt, &config, &|token| {
                let _ = app_for_token.emit("ai-file-token", AiTokenEvent { request_id: rid_for_token.clone(), token: token.to_string() });
            })
        })
        .await;

        let elapsed = start.elapsed().as_millis() as u64;
        let source_files = vec![file_path.clone()];

        match stream_result {
            Ok(Ok(response)) => {
                let _ = app_clone.emit("ai-file-complete", AiCompleteEvent { request_id: rid, analysis: to_analysis(response, source_files, elapsed) });
            }
            Ok(Err(e)) => {
                tracing::error!("파일 QA LLM 실패: {}", e);
                let _ = app_clone.emit("ai-file-error", AiErrorEvent { request_id: rid, error: e });
            }
            Err(e) => {
                tracing::error!("파일 QA 태스크 실패: {}", e);
                let _ = app_clone.emit("ai-file-error", AiErrorEvent { request_id: rid, error: format!("처리 중 오류: {}", e) });
            }
        }
    });

    Ok(())
}

/// AI 요약 (비스트리밍, 유형 선택 가능)
#[tauri::command]
pub async fn summarize_ai(
    file_path: String,
    summary_type: Option<String>,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<AiAnalysis> {
    if file_path.trim().is_empty() {
        return Err(ApiError::Validation("파일 경로가 비어있습니다.".to_string()));
    }

    let (client, db_path, config) = {
        let container = state.read()?;
        let client = build_llm_client(&container)?;
        let db_path = container.db_path.clone();
        let settings = container.get_settings();
        let config = GenerateConfig {
            temperature: settings.ai_temperature.min(0.3),
            max_tokens: settings.ai_max_tokens,
        };
        (client, db_path, config)
    };

    let start = Instant::now();
    let file_path_for_result = file_path.clone();
    let stype = summary_type.unwrap_or_else(|| "brief".to_string());

    let content = tokio::task::spawn_blocking(move || {
        let conn = db::get_connection(&db_path)
            .map_err(|e| format!("DB 연결 실패: {}", e))?;
        load_file_chunks_text(&conn, &file_path)
    })
    .await
    .map_err(|e| ApiError::AiError(format!("태스크 실패: {}", e)))?
    .map_err(ApiError::AiError)?;

    let system_prompt = summary_prompt_for_type(&stype);
    let prompt = format!("{}\n\n--- 문서 ---\n{}", system_prompt, content);

    let response = tokio::task::spawn_blocking(move || client.generate(&prompt, &config))
        .await
        .map_err(|e| ApiError::AiError(format!("태스크 실패: {}", e)))?
        .map_err(ApiError::AiError)?;

    let elapsed = start.elapsed().as_millis() as u64;

    Ok(to_analysis(response, vec![file_path_for_result], elapsed))
}
