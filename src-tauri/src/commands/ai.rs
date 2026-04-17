//! AI Commands — RAG 질문 + AI 요약
//!
//! - ask_ai: 하이브리드 검색 → 컨텍스트 빌드 → LLM 스트리밍 응답
//! - ask_ai_file: 단일 파일 청크 → LLM 스트리밍 응답 (파일 전용 QA)
//! - summarize_ai: 파일 청크 → LLM 요약 (유형 선택 가능)

use crate::application::dto::search::{AiAnalysis, MatchType, SearchResult, TokenUsage};
use crate::application::services::search_service::helpers::{
    smart_apply_exclude_filter, smart_apply_file_type_filter,
};
use crate::db;
use crate::error::{ApiError, ApiResult};
use crate::llm::gemini::GeminiClient;
use crate::llm::{
    summary_prompt_for_type, GenerateConfig, LlmProvider, FILE_QA_SYSTEM_PROMPT, MAX_CONTEXT_CHARS,
    QA_SYSTEM_PROMPT,
};
use crate::search::nl_query::NlQueryParser;
use crate::AppContainer;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Instant;
use tauri::{AppHandle, Emitter, State};

const MAX_QUERY_LEN: usize = 1000;
const RAG_RETRIEVE_LIMIT: usize = 25;
/// RAG 이웃 청크 확장 반경 — 표/리스트 연속성 보장용
const RAG_NEIGHBOR_RADIUS: i64 = 1;
/// 이 값보다 적게 걸리면 접미사 폴백 쿼리 시도
/// (예: "예산액"이 4건만 걸리면 "예산"으로 재검색)
const RAG_LOW_RESULTS_THRESHOLD: usize = 10;

/// 한국어 복합명사 접미사 — 리트리벌 빈약 시 접미사 제거 후 재검색용.
///
/// 금액/비율/수량/요금 관련 접미사만 포함 (ambiguity 낮은 것):
/// 예산액→예산, 집행률→집행, 사용량→사용, 수수료→수수, 사건건→사건,
/// 임대비→임대, 소득세→소득, 지원금→지원.
const KR_NOUN_SUFFIXES: &[&str] = &["액", "률", "량", "료", "건", "비", "세", "금"];

/// 검색어 토큰 중 한국어 복합명사 접미사로 끝나는 것을 **접미사 제거 버전**으로 바꿔 반환.
///
/// 최소 3글자 이상이고 stem이 2글자 이상인 경우만 처리. 한 토큰이라도 변경되면 Some.
/// 예: "2026 아차산 메아리 예산액" → Some("2026 아차산 메아리 예산")
fn strip_suffix_variants(query: &str) -> Option<String> {
    let mut changed = false;
    let words: Vec<String> = query
        .split_whitespace()
        .map(|w| {
            let char_count = w.chars().count();
            if char_count >= 3 {
                for suf in KR_NOUN_SUFFIXES {
                    if w.ends_with(suf) {
                        let stem: String = w.chars().take(char_count - 1).collect();
                        if stem.chars().count() >= 2 {
                            changed = true;
                            return stem;
                        }
                    }
                }
            }
            w.to_string()
        })
        .collect();
    if changed {
        Some(words.join(" "))
    } else {
        None
    }
}
/// 동시 AI 요청 제한 (API 비용 폭발 방지)
const MAX_CONCURRENT_AI_REQUESTS: usize = 3;

/// AI 동시 요청 세마포어
static AI_SEMAPHORE: std::sync::LazyLock<tokio::sync::Semaphore> =
    std::sync::LazyLock::new(|| tokio::sync::Semaphore::new(MAX_CONCURRENT_AI_REQUESTS));

/// 현재 진행 중인 AI 스트리밍의 취소 토큰
/// 새 요청이 오면 이전 토큰을 cancel하고 새 토큰으로 교체
static AI_CANCEL: std::sync::LazyLock<std::sync::Mutex<Arc<AtomicBool>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(Arc::new(AtomicBool::new(false))));

static AI_FILE_CANCEL: std::sync::LazyLock<std::sync::Mutex<Arc<AtomicBool>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(Arc::new(AtomicBool::new(false))));

/// 이전 요청 취소 + 새 취소 토큰 발급
fn rotate_cancel_token(slot: &std::sync::Mutex<Arc<AtomicBool>>) -> Arc<AtomicBool> {
    let mut guard = slot.lock().unwrap_or_else(|p| p.into_inner());
    // 이전 요청 취소
    guard.store(true, Ordering::Release);
    // 새 토큰 발급
    let new_token = Arc::new(AtomicBool::new(false));
    *guard = Arc::clone(&new_token);
    new_token
}

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
    let api_key = settings
        .ai_api_key
        .filter(|k| !k.is_empty())
        .ok_or_else(|| {
            ApiError::AiError(
                "API 키가 설정되지 않았습니다. 설정 > AI에서 입력해주세요.".to_string(),
            )
        })?;
    Ok(GeminiClient::new(api_key, settings.ai_model))
}

/// retrieved 청크의 ±radius 이웃을 DB에서 추가 로드하여 병합.
///
/// 검색 랭킹에서 표/리스트 중간 청크만 top-k에 올라왔을 때, 인접 청크를 함께 제공해
/// LLM이 문맥을 복원할 수 있도록 한다. 이웃 청크는 SearchResult로 합성되며
/// `full_content`만 채워지고 나머지 필드는 기본값이다 (build_rag_context는
/// file_path / file_name / chunk_index / full_content / page_number / location_hint 만 사용).
async fn expand_with_neighbors(
    results: Vec<SearchResult>,
    db_path: &std::path::Path,
) -> Vec<SearchResult> {
    use std::collections::{HashMap, HashSet};

    if results.is_empty() {
        return results;
    }

    // 파일별 retrieved chunk_index 집합
    let mut per_file: HashMap<String, HashSet<i64>> = HashMap::new();
    for r in &results {
        per_file
            .entry(r.file_path.clone())
            .or_default()
            .insert(r.chunk_index);
    }

    // 이웃 인덱스 계산 (원본 제외)
    let mut to_fetch: Vec<(String, Vec<i64>)> = Vec::new();
    for (path, indices) in &per_file {
        let mut neighbors: HashSet<i64> = HashSet::new();
        for &idx in indices {
            for offset in -RAG_NEIGHBOR_RADIUS..=RAG_NEIGHBOR_RADIUS {
                let n = idx + offset;
                if n >= 0 && !indices.contains(&n) {
                    neighbors.insert(n);
                }
            }
        }
        if !neighbors.is_empty() {
            let mut v: Vec<i64> = neighbors.into_iter().collect();
            v.sort_unstable();
            to_fetch.push((path.clone(), v));
        }
    }

    if to_fetch.is_empty() {
        return results;
    }

    // DB 조회 (blocking I/O → spawn_blocking)
    let db_path_owned = db_path.to_path_buf();
    let fetched = tokio::task::spawn_blocking(move || -> Vec<crate::db::ChunkInfo> {
        let conn = match crate::db::get_connection(&db_path_owned) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("RAG 이웃 확장 DB 연결 실패: {}", e);
                return vec![];
            }
        };
        let mut all = Vec::new();
        for (path, indices) in &to_fetch {
            match crate::db::get_chunks_for_file_indices(&conn, path, indices) {
                Ok(chunks) => all.extend(chunks),
                Err(e) => tracing::warn!("RAG 이웃 청크 조회 실패 ({}): {}", path, e),
            }
        }
        all
    })
    .await
    .unwrap_or_default();

    tracing::debug!("RAG 이웃 청크 {}개 추가 로드", fetched.len());

    // ChunkInfo → SearchResult 합성 후 병합
    let mut merged = results;
    for c in fetched {
        merged.push(SearchResult {
            file_path: c.file_path,
            file_name: c.file_name,
            chunk_index: c.chunk_index,
            content_preview: String::new(),
            full_content: c.content,
            score: 0.0,
            confidence: 0,
            match_type: MatchType::Keyword,
            highlight_ranges: vec![],
            page_number: c.page_number,
            start_offset: c.start_offset,
            location_hint: c.location_hint,
            snippet: None,
            modified_at: c.modified_at,
            has_hwp_pair: false,
            total_chunks: 0,
        });
    }

    merged
}

/// 문서 내용에 들어있는 프롬프트 구분자를 치환하여 Prompt Injection 방어
///
/// 공격자가 인덱싱된 문서 안에 `--- 질문 ---` 같은 구분자를 삽입해 LLM
/// 프롬프트 경계를 뚫으려는 시도를 차단한다. 외장 드라이브/공유 폴더처럼
/// 제3자 파일이 들어올 수 있는 환경에서 실제 위협이 된다.
fn sanitize_doc_content(s: &str) -> String {
    // 하이픈 3개(---) 시작 라인의 구분자 패턴을 em-dash로 치환
    // 의미는 유지하면서 프롬프트 경계와 충돌하지 않도록 함
    s.replace("--- 질문 ---", "— 질문 —")
        .replace("--- 문서 ---", "— 문서 —")
        .replace("--- 문서 내용 ---", "— 문서 내용 —")
        .replace("--- 답변 ---", "— 답변 —")
        .replace("--- 시스템 ---", "— 시스템 —")
        .replace("--- User ---", "— User —")
        .replace("--- Assistant ---", "— Assistant —")
        .replace("--- System ---", "— System —")
}

/// 검색 결과 → RAG 컨텍스트 문자열
///
/// 동일 파일의 청크를 그룹화하여 문맥 연속성 확보.
/// location_hint(페이지/시트) 포함으로 출처 정확도 향상.
/// 문서 내용은 sanitize_doc_content로 Prompt Injection 방어.
fn build_rag_context(
    results: &[crate::application::dto::search::SearchResult],
) -> (String, Vec<String>) {
    // 파일별 청크 그룹화 (검색 순서 유지)
    let mut file_order: Vec<String> = Vec::new();
    let mut file_groups: std::collections::HashMap<
        String,
        Vec<&crate::application::dto::search::SearchResult>,
    > = std::collections::HashMap::new();
    for r in results {
        if !file_groups.contains_key(&r.file_path) {
            file_order.push(r.file_path.clone());
        }
        file_groups.entry(r.file_path.clone()).or_default().push(r);
    }

    let mut context = String::new();
    let mut source_files: Vec<String> = Vec::new();

    for file_path in &file_order {
        let chunks = match file_groups.get(file_path) {
            Some(c) => c,
            None => continue,
        };
        if context.len() >= MAX_CONTEXT_CHARS {
            break;
        }

        source_files.push(file_path.clone());
        let doc_num = source_files.len();
        let file_name = &chunks[0].file_name;

        // 문서 헤더 (파일당 1회)
        context.push_str(&format!("[문서{}: {}]\n", doc_num, file_name));

        // 파일 내 청크들을 chunk_index 순으로 정렬하여 문맥 연속성 확보
        let mut sorted_chunks: Vec<_> = chunks.iter().collect();
        sorted_chunks.sort_by_key(|r| r.chunk_index);

        for (i, r) in sorted_chunks.iter().enumerate() {
            if context.len() >= MAX_CONTEXT_CHARS {
                break;
            }

            // 위치 정보 (페이지, 시트 등)
            if let Some(hint) = &r.location_hint {
                context.push_str(&format!("({})\n", hint));
            } else if let Some(page) = r.page_number {
                context.push_str(&format!("(페이지 {})\n", page));
            }

            let raw = if r.full_content.is_empty() {
                &r.content_preview
            } else {
                &r.full_content
            };
            let content = sanitize_doc_content(raw);

            // UTF-8 char boundary 안전한 자르기
            let remaining = MAX_CONTEXT_CHARS.saturating_sub(context.len());
            if content.len() > remaining {
                let mut end = remaining;
                while end > 0 && !content.is_char_boundary(end) {
                    end -= 1;
                }
                context.push_str(&content[..end]);
                break; // 컨텍스트 한계 도달
            } else {
                context.push_str(&content);
            }

            // 청크 간 구분 (같은 파일 내)
            if i + 1 < sorted_chunks.len() {
                context.push_str("\n...\n");
            }
        }

        context.push_str("\n\n");
    }

    (context, source_files)
}

/// 파일 청크 텍스트 로드 (공통 헬퍼)
/// 각 청크는 sanitize_doc_content로 Prompt Injection 방어된 뒤 연결됨.
fn load_file_chunks_text(conn: &rusqlite::Connection, file_path: &str) -> Result<String, String> {
    let chunk_ids = db::get_chunk_ids_for_path(conn, file_path)
        .map_err(|e| format!("청크 조회 실패: {}", e))?;

    if chunk_ids.is_empty() {
        return Err("이 파일의 인덱스가 없습니다. 폴더를 인덱싱해주세요.".to_string());
    }

    let chunks =
        db::get_chunks_by_ids(conn, &chunk_ids).map_err(|e| format!("청크 로드 실패: {}", e))?;

    let mut sorted = chunks;
    sorted.sort_by_key(|c| c.chunk_index);

    let mut text = String::new();
    for chunk in &sorted {
        if text.len() >= MAX_CONTEXT_CHARS {
            break;
        }
        let sanitized = sanitize_doc_content(&chunk.content);
        let remaining = MAX_CONTEXT_CHARS.saturating_sub(text.len());
        if sanitized.len() > remaining {
            let mut end = remaining;
            while end > 0 && !sanitized.is_char_boundary(end) {
                end -= 1;
            }
            text.push_str(&sanitized[..end]);
            break;
        } else {
            text.push_str(&sanitized);
            text.push('\n');
        }
    }

    Ok(text)
}

/// 파일 청크 텍스트 로드 (제한된 예산)
fn load_file_chunks_text_limited(
    conn: &rusqlite::Connection,
    file_path: &str,
    max_chars: usize,
) -> Result<String, String> {
    let chunk_ids = db::get_chunk_ids_for_path(conn, file_path)
        .map_err(|e| format!("청크 조회 실패: {}", e))?;

    if chunk_ids.is_empty() {
        return Ok(String::new());
    }

    let chunks =
        db::get_chunks_by_ids(conn, &chunk_ids).map_err(|e| format!("청크 로드 실패: {}", e))?;

    let mut sorted = chunks;
    sorted.sort_by_key(|c| c.chunk_index);

    let mut text = String::new();
    for chunk in &sorted {
        if text.len() >= max_chars {
            break;
        }
        let sanitized = sanitize_doc_content(&chunk.content);
        let remaining = max_chars.saturating_sub(text.len());
        if sanitized.len() > remaining {
            let mut end = remaining;
            while end > 0 && !sanitized.is_char_boundary(end) {
                end -= 1;
            }
            text.push_str(&sanitized[..end]);
            break;
        } else {
            text.push_str(&sanitized);
            text.push('\n');
        }
    }

    Ok(text)
}

/// 스트리밍 결과를 AiAnalysis DTO로 변환 헬퍼
fn to_analysis(
    response: crate::llm::LlmResponse,
    source_files: Vec<String>,
    elapsed: u64,
) -> AiAnalysis {
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
        return Err(ApiError::Validation(format!(
            "질문이 너무 깁니다 (최대 {}자)",
            MAX_QUERY_LEN
        )));
    }

    // 동시 AI 요청 제한
    let permit = AI_SEMAPHORE.try_acquire().map_err(|_| {
        ApiError::AiError("AI 요청이 너무 많습니다. 잠시 후 다시 시도해주세요.".to_string())
    })?;

    let (client, service, config, tokenizer, db_path) = {
        let container = state.read()?;
        let client = build_llm_client(&container)?;
        let service = container.search_service();
        let settings = container.get_settings();
        let config = GenerateConfig {
            temperature: settings.ai_temperature,
            max_tokens: settings.ai_max_tokens,
        };
        let tokenizer = container.get_tokenizer().ok();
        let db_path = container.db_path.clone();
        (client, service, config, tokenizer, db_path)
    };

    // 이전 요청 취소 + 새 취소 토큰 발급
    let cancel_token = rotate_cancel_token(&AI_CANCEL);

    let query_clone = query.clone();
    let app_clone = app.clone();
    let rid = request_id.clone();

    tauri::async_runtime::spawn(async move {
        // permit을 태스크 완료까지 보유하여 동시 요청 제한 보장
        let _permit = permit;
        let start = Instant::now();

        // 자연어 질문에서 키워드 추출
        // 토크나이저가 있으면 형태소 분석으로 명사만 추출 (의문사/조사 자동 제거)
        let parsed = match tokenizer.as_ref() {
            Some(tok) => NlQueryParser::parse_with_tokenizer(&query_clone, tok.as_ref()),
            None => NlQueryParser::parse(&query_clone),
        };
        // RAG에서는 날짜를 키워드에 포함 (문서 내용 연도 검색용)
        use chrono::Datelike;
        let mut search_query = if parsed.keywords.is_empty() {
            query_clone.clone()
        } else {
            parsed.keywords.clone()
        };
        if let Some(ref df) = parsed.date_filter {
            let year_str = match df {
                crate::search::nl_query::DateFilter::Year(y) => Some(y.to_string()),
                crate::search::nl_query::DateFilter::LastYear => {
                    Some((chrono::Utc::now().naive_utc().date().year() - 1).to_string())
                }
                crate::search::nl_query::DateFilter::ThisYear => {
                    Some(chrono::Utc::now().naive_utc().date().year().to_string())
                }
                _ => None,
            };
            if let Some(y) = year_str {
                if !search_query.contains(&y) {
                    search_query = format!("{} {}", y, search_query);
                }
            }
        }

        tracing::debug!("RAG 검색쿼리: '{}' (원본: '{}')", search_query, query_clone);

        // 검색 전 취소 확인
        if cancel_token.load(Ordering::Relaxed) {
            return;
        }

        let search_result = service
            .search_hybrid(&search_query, RAG_RETRIEVE_LIMIT, folder_scope.as_deref())
            .await;

        let results = match search_result {
            Ok(resp) => resp.results,
            Err(e) => {
                tracing::error!("RAG 검색 실패: {}", e);
                let _ = app_clone.emit(
                    "ai-error",
                    AiErrorEvent {
                        request_id: rid,
                        error: format!("검색 실패: {}", e),
                    },
                );
                return;
            }
        };

        // NL 파서가 추출한 필터 적용 (exclude, file_type만)
        // ⚠ date_filter는 RAG에 적용하지 않음:
        //   "2026년 노인일자리"에서 "2026년"은 문서 내용의 연도이지 파일 수정일이 아님.
        //   파일 수정일 필터를 걸면 관련 문서를 놓칠 수 있음.
        //   연도는 키워드로 FTS 검색에 반영됨.
        let mut results: Vec<_> = results
            .into_iter()
            .filter(|r| smart_apply_exclude_filter(r, &parsed.exclude_keywords))
            .filter(|r| smart_apply_file_type_filter(r, &parsed.file_type))
            .collect();

        // 저결과 폴백 — 복합명사 접미사 때문에 매칭이 빈약할 수 있음.
        // 예: "예산액"으로 4건만 걸릴 때 "예산"으로 재검색해서 커버리지 확보.
        // 한국어에서 "예산액/집행률/사용량" 같은 복합명사가 FTS5 토큰 경계로
        // 쪼개지거나 원본에 다른 표기로 존재하는 경우를 구제한다.
        if results.len() < RAG_LOW_RESULTS_THRESHOLD {
            if let Some(fallback_query) = strip_suffix_variants(&search_query) {
                tracing::debug!(
                    "RAG 저결과 폴백: '{}' ({} hits) → '{}'",
                    search_query,
                    results.len(),
                    fallback_query
                );
                if let Ok(fallback_resp) = service
                    .search_hybrid(&fallback_query, RAG_RETRIEVE_LIMIT, folder_scope.as_deref())
                    .await
                {
                    let existing: std::collections::HashSet<(String, i64)> = results
                        .iter()
                        .map(|r| (r.file_path.clone(), r.chunk_index))
                        .collect();

                    let additional = fallback_resp
                        .results
                        .into_iter()
                        .filter(|r| smart_apply_exclude_filter(r, &parsed.exclude_keywords))
                        .filter(|r| smart_apply_file_type_filter(r, &parsed.file_type))
                        .filter(|r| !existing.contains(&(r.file_path.clone(), r.chunk_index)));

                    for r in additional {
                        results.push(r);
                    }
                    tracing::debug!("RAG 폴백 병합 후 {} hits", results.len());
                }
            }
        }

        // 최장 단어 폴백 — NL 파서가 "관련"/"요약" 같은 자연어 보조어까지 키워드로 잡아서
        // FTS AND 쿼리가 과도하게 좁아진 경우 구제. "금연도시 관련 요약" → "금연도시"만
        // 단독 검색. 2글자 이상, 원본과 달라야 하고 공백 2개 이상 포함된 쿼리에만 적용.
        if results.is_empty() && search_query.split_whitespace().count() >= 2 {
            if let Some(longest) = search_query
                .split_whitespace()
                .filter(|w| w.chars().count() >= 2)
                .max_by_key(|w| w.chars().count())
            {
                tracing::debug!("RAG 0건 폴백: 최장 단어 '{}' 재시도", longest);
                if let Ok(resp) = service
                    .search_hybrid(longest, RAG_RETRIEVE_LIMIT, folder_scope.as_deref())
                    .await
                {
                    results = resp
                        .results
                        .into_iter()
                        .filter(|r| smart_apply_exclude_filter(r, &parsed.exclude_keywords))
                        .filter(|r| smart_apply_file_type_filter(r, &parsed.file_type))
                        .collect();
                    tracing::debug!("RAG 최장단어 폴백 결과 {} hits", results.len());
                }
            }
        }

        // 이웃 청크 확장 — 표/리스트가 청크 경계에서 잘리는 문제 해결.
        // 검색으로 걸린 청크의 ±RAG_NEIGHBOR_RADIUS 범위 청크를 DB에서 추가 로드하여
        // 같은 파일 내 문맥 연속성을 확보한다. (예: 예산표 중간 청크만 걸렸을 때
        // 앞뒤 청크를 함께 제공하면 LLM이 "표가 잘렸다"고 답하지 않음.)
        let results = expand_with_neighbors(results, &db_path).await;

        if results.is_empty() {
            let _ = app_clone.emit(
                "ai-error",
                AiErrorEvent {
                    request_id: rid,
                    error: format!(
                        "'{}' 관련 문서를 찾지 못했습니다. 더 일반적인 키워드로 시도해보세요 \
                         (예: 조사·서술어 제거, 핵심 명사만 사용).",
                        query_clone
                    ),
                },
            );
            return;
        }

        // LLM 호출 전 취소 확인
        if cancel_token.load(Ordering::Relaxed) {
            return;
        }

        let (context, source_files) = build_rag_context(&results);
        let prompt = format!(
            "{}\n\n--- 문서 내용 ---\n{}\n--- 질문 ---\n{}",
            QA_SYSTEM_PROMPT, context, query_clone
        );

        let app_for_token = app_clone.clone();
        let rid_for_token = rid.clone();
        let cancel_for_stream = Arc::clone(&cancel_token);
        let stream_result = tokio::task::spawn_blocking(move || {
            client.generate_stream(
                &prompt,
                &config,
                &|token| {
                    let _ = app_for_token.emit(
                        "ai-token",
                        AiTokenEvent {
                            request_id: rid_for_token.clone(),
                            token: token.to_string(),
                        },
                    );
                },
                &cancel_for_stream,
            )
        })
        .await;

        let elapsed = start.elapsed().as_millis() as u64;

        // 취소된 요청은 이벤트 발행하지 않음
        if cancel_token.load(Ordering::Relaxed) {
            return;
        }

        match stream_result {
            Ok(Ok(response)) => {
                let _ = app_clone.emit(
                    "ai-complete",
                    AiCompleteEvent {
                        request_id: rid,
                        analysis: to_analysis(response, source_files, elapsed),
                    },
                );
            }
            Ok(Err(e)) => {
                tracing::error!("LLM 생성 실패: {}", e);
                let _ = app_clone.emit(
                    "ai-error",
                    AiErrorEvent {
                        request_id: rid,
                        error: e,
                    },
                );
            }
            Err(e) => {
                tracing::error!("LLM 태스크 실패: {}", e);
                let _ = app_clone.emit(
                    "ai-error",
                    AiErrorEvent {
                        request_id: rid,
                        error: format!("처리 중 오류: {}", e),
                    },
                );
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
        return Err(ApiError::Validation(format!(
            "질문이 너무 깁니다 (최대 {}자)",
            MAX_QUERY_LEN
        )));
    }
    if file_path.trim().is_empty() {
        return Err(ApiError::Validation(
            "파일 경로가 비어있습니다.".to_string(),
        ));
    }

    // 동시 AI 요청 제한
    let permit = AI_SEMAPHORE.try_acquire().map_err(|_| {
        ApiError::AiError("AI 요청이 너무 많습니다. 잠시 후 다시 시도해주세요.".to_string())
    })?;

    let (client, db_path, config, service, tokenizer) = {
        let container = state.read()?;
        let client = build_llm_client(&container)?;
        let db_path = container.db_path.clone();
        let settings = container.get_settings();
        let config = GenerateConfig {
            temperature: settings.ai_temperature,
            max_tokens: settings.ai_max_tokens,
        };
        let service = container.search_service();
        let tokenizer = container.get_tokenizer().ok();
        (client, db_path, config, service, tokenizer)
    };

    // 이전 파일 QA 요청 취소 + 새 취소 토큰 발급
    let cancel_token = rotate_cancel_token(&AI_FILE_CANCEL);

    let app_clone = app.clone();
    let file_path_clone = file_path.clone();
    let rid = request_id.clone();
    let query_clone = query.clone();

    tauri::async_runtime::spawn(async move {
        // permit을 태스크 완료까지 보유하여 동시 요청 제한 보장
        let _permit = permit;
        let start = Instant::now();

        // 1단계: 쿼리 기반으로 파일 내 관련 청크 검색 (타겟 검색)
        let parsed = match tokenizer.as_ref() {
            Some(tok) => NlQueryParser::parse_with_tokenizer(&query_clone, tok.as_ref()),
            None => NlQueryParser::parse(&query_clone),
        };
        let search_query = if parsed.keywords.is_empty() {
            query_clone.clone()
        } else {
            parsed.keywords.clone()
        };

        tracing::debug!(
            "파일 QA 검색쿼리: '{}' (원본: '{}', 파일: '{}')",
            search_query,
            query_clone,
            file_path_clone
        );

        // 하이브리드 검색 후 대상 파일만 필터
        let targeted_results = service
            .search_hybrid(&search_query, RAG_RETRIEVE_LIMIT, None)
            .await
            .ok()
            .map(|resp| {
                resp.results
                    .into_iter()
                    .filter(|r| r.file_path.eq_ignore_ascii_case(&file_path_clone))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        if cancel_token.load(Ordering::Relaxed) {
            return;
        }

        // 2단계: 컨텍스트 빌드
        // - 검색 결과가 있으면 관련 청크 우선 사용 + 나머지 예산으로 순차 청크 보충
        // - 검색 결과 없으면 순차 로딩 폴백
        let db_path_for_load = db_path.clone();
        let fp_for_load = file_path_clone.clone();
        let targeted_len = targeted_results.len();

        let content = if !targeted_results.is_empty() {
            let (targeted_ctx, _) = build_rag_context(&targeted_results);
            let targeted_chars = targeted_ctx.len();

            // 남은 예산으로 순차 청크 보충 (문서 앞부분 맥락 제공)
            if targeted_chars < MAX_CONTEXT_CHARS / 2 {
                let remaining_budget = MAX_CONTEXT_CHARS.saturating_sub(targeted_chars + 200);
                let sequential = tokio::task::spawn_blocking(move || {
                    let conn = db::get_connection(&db_path_for_load).ok()?;
                    load_file_chunks_text_limited(&conn, &fp_for_load, remaining_budget).ok()
                })
                .await
                .ok()
                .flatten()
                .unwrap_or_default();

                if sequential.is_empty() {
                    targeted_ctx
                } else {
                    format!(
                        "[문서 앞부분]\n{}\n\n[질문 관련 부분]\n{}",
                        sequential, targeted_ctx
                    )
                }
            } else {
                targeted_ctx
            }
        } else {
            // 검색 결과 없음 → 순차 로딩 폴백
            let result = tokio::task::spawn_blocking(move || {
                let conn = db::get_connection(&db_path_for_load)
                    .map_err(|e| format!("DB 연결 실패: {}", e))?;
                load_file_chunks_text(&conn, &fp_for_load)
            })
            .await;

            match result {
                Ok(Ok(text)) => text,
                Ok(Err(e)) => {
                    let _ = app_clone.emit(
                        "ai-file-error",
                        AiErrorEvent {
                            request_id: rid,
                            error: e,
                        },
                    );
                    return;
                }
                Err(e) => {
                    let _ = app_clone.emit(
                        "ai-file-error",
                        AiErrorEvent {
                            request_id: rid,
                            error: format!("태스크 실패: {}", e),
                        },
                    );
                    return;
                }
            }
        };

        tracing::debug!(
            "파일 QA 컨텍스트: {}자 (타겟 청크 {}개)",
            content.len(),
            targeted_len
        );

        let prompt = format!(
            "{}\n\n--- 문서 내용 ---\n{}\n--- 질문 ---\n{}",
            FILE_QA_SYSTEM_PROMPT, content, query
        );

        let app_for_token = app_clone.clone();
        let rid_for_token = rid.clone();
        let cancel_for_stream = Arc::clone(&cancel_token);
        let stream_result = tokio::task::spawn_blocking(move || {
            client.generate_stream(
                &prompt,
                &config,
                &|token| {
                    let _ = app_for_token.emit(
                        "ai-file-token",
                        AiTokenEvent {
                            request_id: rid_for_token.clone(),
                            token: token.to_string(),
                        },
                    );
                },
                &cancel_for_stream,
            )
        })
        .await;

        let elapsed = start.elapsed().as_millis() as u64;

        if cancel_token.load(Ordering::Relaxed) {
            return;
        }

        let source_files = vec![file_path.clone()];

        match stream_result {
            Ok(Ok(response)) => {
                let _ = app_clone.emit(
                    "ai-file-complete",
                    AiCompleteEvent {
                        request_id: rid,
                        analysis: to_analysis(response, source_files, elapsed),
                    },
                );
            }
            Ok(Err(e)) => {
                tracing::error!("파일 QA LLM 실패: {}", e);
                let _ = app_clone.emit(
                    "ai-file-error",
                    AiErrorEvent {
                        request_id: rid,
                        error: e,
                    },
                );
            }
            Err(e) => {
                tracing::error!("파일 QA 태스크 실패: {}", e);
                let _ = app_clone.emit(
                    "ai-file-error",
                    AiErrorEvent {
                        request_id: rid,
                        error: format!("처리 중 오류: {}", e),
                    },
                );
            }
        }
    });

    Ok(())
}

/// AI 요약 (비스트리밍, 유형 선택 가능)
/// 취소 토큰 미적용: non-streaming + spawn_blocking이라 중간 취소 불가.
/// Semaphore로 동시 요청만 제한.
#[tauri::command]
pub async fn summarize_ai(
    file_path: String,
    summary_type: Option<String>,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<AiAnalysis> {
    if file_path.trim().is_empty() {
        return Err(ApiError::Validation(
            "파일 경로가 비어있습니다.".to_string(),
        ));
    }

    // 동시 AI 요청 제한
    let _permit = AI_SEMAPHORE.try_acquire().map_err(|_| {
        ApiError::AiError("AI 요청이 너무 많습니다. 잠시 후 다시 시도해주세요.".to_string())
    })?;

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
        let conn = db::get_connection(&db_path).map_err(|e| format!("DB 연결 실패: {}", e))?;
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
