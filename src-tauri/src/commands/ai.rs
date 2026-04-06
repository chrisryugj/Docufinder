//! AI Commands — RAG 질문 + AI 요약
//!
//! - ask_ai: 하이브리드 검색 → 컨텍스트 빌드 → LLM 스트리밍 응답
//! - summarize_ai: 파일 청크 → LLM 요약

use crate::application::dto::search::{AiAnalysis, TokenUsage};
use crate::db;
use crate::error::{ApiError, ApiResult};
use crate::llm::gemini::GeminiClient;
use crate::llm::{GenerateConfig, LlmProvider, MAX_CONTEXT_CHARS, QA_SYSTEM_PROMPT, SUMMARY_SYSTEM_PROMPT};
use crate::AppContainer;
use std::sync::RwLock;
use std::time::Instant;
use tauri::{AppHandle, Emitter, State};

const MAX_QUERY_LEN: usize = 1000;
const RAG_RETRIEVE_LIMIT: usize = 10;

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

/// 검색 결과 → RAG 컨텍스트 문자열
fn build_rag_context(
    results: &[crate::application::dto::search::SearchResult],
) -> (String, Vec<String>) {
    let mut context = String::new();
    let mut source_files: Vec<String> = Vec::new();
    let mut seen_files = std::collections::HashSet::new();

    for r in results {
        // 중복 파일 추적
        if seen_files.insert(r.file_path.clone()) {
            source_files.push(r.file_path.clone());
        }

        // 컨텍스트 크기 제한
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

        // full_content 우선, 없으면 content_preview
        let content = if r.full_content.is_empty() {
            &r.content_preview
        } else {
            &r.full_content
        };

        let remaining = MAX_CONTEXT_CHARS.saturating_sub(context.len());
        if content.len() > remaining {
            context.push_str(&content[..remaining]);
        } else {
            context.push_str(content);
        }
        context.push_str("\n\n");
    }

    (context, source_files)
}

/// RAG 질문 (스트리밍) — Tauri events로 토큰 전달
#[tauri::command]
pub async fn ask_ai(
    query: String,
    folder_scope: Option<String>,
    app: AppHandle,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<()> {
    // 입력 검증
    if query.trim().is_empty() {
        return Err(ApiError::Validation("질문을 입력해주세요.".to_string()));
    }
    if query.chars().count() > MAX_QUERY_LEN {
        return Err(ApiError::Validation(format!(
            "질문이 너무 깁니다 (최대 {}자)",
            MAX_QUERY_LEN
        )));
    }

    // LLM 클라이언트 + 검색 서비스 준비
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

    // 비동기 검색 → 스트리밍 응답 (별도 태스크)
    let query_clone = query.clone();
    let app_clone = app.clone();

    tauri::async_runtime::spawn(async move {
        let start = Instant::now();

        // 1. 하이브리드 검색으로 관련 청크 수집
        let search_result = service
            .search_hybrid(&query_clone, RAG_RETRIEVE_LIMIT, folder_scope.as_deref())
            .await;

        let results = match search_result {
            Ok(resp) => resp.results,
            Err(e) => {
                tracing::error!("RAG 검색 실패: {}", e);
                let _ = app_clone.emit("ai-error", format!("검색 실패: {}", e));
                return;
            }
        };

        if results.is_empty() {
            let _ = app_clone.emit(
                "ai-error",
                "관련 문서를 찾을 수 없습니다. 먼저 폴더를 인덱싱해주세요.".to_string(),
            );
            return;
        }

        // 2. 컨텍스트 빌드
        let (context, source_files) = build_rag_context(&results);

        // 3. 프롬프트 조립
        let prompt = format!(
            "{}\n\n--- 문서 내용 ---\n{}\n--- 질문 ---\n{}",
            QA_SYSTEM_PROMPT, context, query_clone
        );

        // 4. LLM 스트리밍 (blocking — ureq는 sync)
        let app_for_token = app_clone.clone();
        let stream_result = tokio::task::spawn_blocking(move || {
            client.generate_stream(&prompt, &config, &|token| {
                let _ = app_for_token.emit("ai-token", token);
            })
        })
        .await;

        let elapsed = start.elapsed().as_millis() as u64;

        match stream_result {
            Ok(Ok(response)) => {
                let analysis = AiAnalysis {
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
                };
                let _ = app_clone.emit("ai-complete", &analysis);
            }
            Ok(Err(e)) => {
                tracing::error!("LLM 생성 실패: {}", e);
                let _ = app_clone.emit("ai-error", e);
            }
            Err(e) => {
                tracing::error!("LLM 태스크 실패: {}", e);
                let _ = app_clone.emit("ai-error", format!("처리 중 오류: {}", e));
            }
        }
    });

    Ok(())
}

/// AI 요약 (비스트리밍)
#[tauri::command]
pub async fn summarize_ai(
    file_path: String,
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
            temperature: settings.ai_temperature.min(0.3), // 요약은 낮은 온도
            max_tokens: settings.ai_max_tokens,
        };
        (client, db_path, config)
    };

    let start = Instant::now();
    let file_path_for_result = file_path.clone();

    // DB에서 청크 로드
    let content = tokio::task::spawn_blocking(move || -> Result<String, String> {
        let conn = db::get_connection(&db_path)
            .map_err(|e| format!("DB 연결 실패: {}", e))?;
        let chunk_ids = db::get_chunk_ids_for_path(&conn, &file_path)
            .map_err(|e| format!("청크 조회 실패: {}", e))?;

        if chunk_ids.is_empty() {
            return Err("이 파일의 인덱스가 없습니다. 폴더를 인덱싱해주세요.".to_string());
        }

        let chunks = db::get_chunks_by_ids(&conn, &chunk_ids)
            .map_err(|e| format!("청크 로드 실패: {}", e))?;

        // 청크 인덱스 순으로 정렬 후 연결
        let mut sorted = chunks;
        sorted.sort_by_key(|c| c.chunk_index);

        let mut text = String::new();
        for chunk in &sorted {
            if text.len() >= MAX_CONTEXT_CHARS {
                break;
            }
            text.push_str(&chunk.content);
            text.push('\n');
        }

        Ok(text)
    })
    .await
    .map_err(|e| ApiError::AiError(format!("태스크 실패: {}", e)))?
    .map_err(|e| ApiError::AiError(e))?;

    // 프롬프트 조립
    let prompt = format!("{}\n\n--- 문서 ---\n{}", SUMMARY_SYSTEM_PROMPT, content);

    // LLM 호출 (blocking)
    let response = tokio::task::spawn_blocking(move || client.generate(&prompt, &config))
        .await
        .map_err(|e| ApiError::AiError(format!("태스크 실패: {}", e)))?
        .map_err(|e| ApiError::AiError(e))?;

    let elapsed = start.elapsed().as_millis() as u64;

    Ok(AiAnalysis {
        answer: response.text,
        source_files: vec![file_path_for_result],
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
    })
}
