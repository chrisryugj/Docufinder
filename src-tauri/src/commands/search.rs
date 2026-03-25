//! Search Commands - Thin Layer (Clean Architecture)
//!
//! Tauri commands that delegate to SearchService.
//! Handles only: input validation, settings retrieval, service invocation.

use crate::application::dto::search::SearchResponse;
use crate::error::{ApiError, ApiResult};
use crate::AppContainer;
use std::sync::RwLock;
use tauri::State;

const MAX_QUERY_LEN: usize = 1000;

fn validate_query(query: &str) -> ApiResult<()> {
    if query.chars().count() > MAX_QUERY_LEN {
        return Err(ApiError::Validation(format!(
            "검색어가 너무 깁니다 (최대 {}자)",
            MAX_QUERY_LEN
        )));
    }
    Ok(())
}

/// 키워드 검색 (FTS5)
#[tauri::command]
pub async fn search_keyword(
    query: String,
    folder_scope: Option<String>,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<SearchResponse> {
    validate_query(&query)?;
    if query.trim().is_empty() {
        return Ok(SearchResponse {
            results: vec![],
            total_count: 0,
            search_time_ms: 0,
            search_mode: "keyword".to_string(),
        });
    }

    let (service, max_results) = {
        let container = state.read()?;
        let max_results = container.get_settings().max_results;
        (container.search_service(), max_results)
    };

    service
        .search_keyword(&query, max_results, folder_scope.as_deref())
        .await
        .map_err(ApiError::from)
}

/// 파일명 검색 (FTS5)
#[tauri::command]
pub async fn search_filename(
    query: String,
    folder_scope: Option<String>,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<SearchResponse> {
    validate_query(&query)?;
    if query.trim().is_empty() {
        return Ok(SearchResponse {
            results: vec![],
            total_count: 0,
            search_time_ms: 0,
            search_mode: "filename".to_string(),
        });
    }

    let (service, max_results) = {
        let container = state.read()?;
        let max_results = container.get_settings().max_results;
        (container.search_service(), max_results)
    };

    service
        .search_filename(&query, max_results, folder_scope.as_deref())
        .await
        .map_err(ApiError::from)
}

/// 시맨틱 검색 (벡터)
#[tauri::command]
pub async fn search_semantic(
    query: String,
    folder_scope: Option<String>,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<SearchResponse> {
    validate_query(&query)?;
    if query.trim().is_empty() {
        return Ok(SearchResponse {
            results: vec![],
            total_count: 0,
            search_time_ms: 0,
            search_mode: "semantic".to_string(),
        });
    }

    let (service, max_results, semantic_enabled) = {
        let container = state.read()?;
        let settings = container.get_settings();
        (
            container.search_service(),
            settings.max_results,
            settings.semantic_search_enabled,
        )
    };

    if !semantic_enabled {
        return Err(ApiError::SemanticSearchDisabled);
    }

    service
        .search_semantic(&query, max_results, folder_scope.as_deref())
        .await
        .map_err(ApiError::from)
}

/// 하이브리드 검색 (FTS + 벡터 + RRF + Reranking)
/// 시맨틱 비활성화 시 키워드 검색으로 폴백
#[tauri::command]
pub async fn search_hybrid(
    query: String,
    folder_scope: Option<String>,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<SearchResponse> {
    validate_query(&query)?;
    if query.trim().is_empty() {
        return Ok(SearchResponse {
            results: vec![],
            total_count: 0,
            search_time_ms: 0,
            search_mode: "hybrid".to_string(),
        });
    }

    let (service, max_results, semantic_enabled) = {
        let container = state.read()?;
        let settings = container.get_settings();
        (
            container.search_service(),
            settings.max_results,
            settings.semantic_search_enabled,
        )
    };

    // 시맨틱 비활성화 시 키워드 검색으로 폴백
    if !semantic_enabled {
        return service
            .search_keyword(&query, max_results, folder_scope.as_deref())
            .await
            .map_err(ApiError::from);
    }

    service
        .search_hybrid(&query, max_results, folder_scope.as_deref())
        .await
        .map_err(ApiError::from)
}

/// 스마트(자연어) 검색
/// NL 파서가 키워드/날짜/파일타입/부정어를 자동 추출 후 하이브리드 검색 위임
#[tauri::command]
pub async fn search_smart(
    query: String,
    folder_scope: Option<String>,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<crate::application::dto::search::SmartSearchResponse> {
    validate_query(&query)?;
    if query.trim().is_empty() {
        return Ok(crate::application::dto::search::SmartSearchResponse {
            results: vec![],
            total_count: 0,
            search_time_ms: 0,
            parsed_query: crate::search::nl_query::NlQueryParser::parse(""),
        });
    }

    let (service, max_results) = {
        let container = state.read()?;
        let max_results = container.get_settings().max_results;
        (container.search_service(), max_results)
    };

    service
        .search_smart(&query, max_results, folder_scope.as_deref())
        .await
        .map_err(ApiError::from)
}

/// 유사 문서 검색 (파일 경로 기반)
#[tauri::command]
pub async fn find_similar_documents(
    file_path: String,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<SearchResponse> {
    if file_path.trim().is_empty() {
        return Err(ApiError::Validation("파일 경로가 비어있습니다".to_string()));
    }

    let (service, max_results) = {
        let container = state.read()?;
        let max_results = container.get_settings().max_results;
        (container.search_service(), max_results)
    };

    service
        .find_similar(&file_path, max_results.min(20)) // 유사문서는 최대 20개
        .await
        .map_err(ApiError::from)
}

/// 문서 카테고리 분류
#[tauri::command]
pub async fn classify_document(
    file_path: String,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<String> {
    if file_path.trim().is_empty() {
        return Err(ApiError::Validation("파일 경로가 비어있습니다".to_string()));
    }

    let (service, db_path) = {
        let container = state.read()?;
        (
            container.search_service(),
            container.db_path.to_string_lossy().to_string(),
        )
    };

    // 파일의 첫 번째 청크 텍스트 가져오기
    let text = tokio::task::spawn_blocking(move || -> ApiResult<String> {
        let conn = crate::db::get_connection(std::path::Path::new(&db_path))?;
        let chunk_ids = crate::db::get_chunk_ids_for_path(&conn, &file_path)
            .map_err(|e| ApiError::DatabaseQuery(e.to_string()))?;

        if chunk_ids.is_empty() {
            return Ok(String::new());
        }

        let chunks = crate::db::get_chunks_by_ids(&conn, &[chunk_ids[0]])
            .map_err(|e| ApiError::DatabaseQuery(e.to_string()))?;

        Ok(chunks
            .first()
            .map(|c| c.content.clone())
            .unwrap_or_default())
    })
    .await??;

    if text.is_empty() {
        return Ok("기타".to_string());
    }

    service.classify_document(&text).map_err(ApiError::from)
}

// ==================== 자동완성 (v2.3) ====================

/// 검색어 자동완성 제안
#[tauri::command]
pub async fn get_suggestions(
    query: String,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<Vec<SuggestionItem>> {
    validate_query(&query)?;
    let prefix = query.trim().to_lowercase();
    if prefix.len() < 2 {
        return Ok(vec![]);
    }

    let db_path = {
        let container = state.read()?;
        container.db_path.clone()
    };

    tokio::task::spawn_blocking(move || -> ApiResult<Vec<SuggestionItem>> {
        let conn = crate::db::get_connection(&db_path)?;
        let mut suggestions = Vec::new();

        // 1) 최근 검색어 히스토리 (우선)
        if let Ok(history) = crate::db::get_search_query_suggestions(&conn, &prefix, 5) {
            for (term, freq) in history {
                suggestions.push(SuggestionItem {
                    text: term,
                    source: "history".to_string(),
                    frequency: freq,
                });
            }
        }

        // 2) fts5vocab 용어 (히스토리에 없는 것만)
        let history_texts: std::collections::HashSet<String> =
            suggestions.iter().map(|s| s.text.clone()).collect();

        if let Ok(vocab) = crate::db::get_vocab_suggestions(&conn, &prefix, 10 + suggestions.len())
        {
            for (term, doc_count) in vocab {
                if !history_texts.contains(&term) && term.len() >= 2 {
                    suggestions.push(SuggestionItem {
                        text: term,
                        source: "vocab".to_string(),
                        frequency: doc_count,
                    });
                }
                if suggestions.len() >= 10 {
                    break;
                }
            }
        }

        Ok(suggestions)
    })
    .await?
}

/// 검색어 저장 (검색 실행 시 호출)
#[tauri::command]
pub async fn save_search_query(
    query: String,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<()> {
    validate_query(&query)?;
    let trimmed = query.trim().to_string();
    if trimmed.len() < 2 {
        return Ok(());
    }

    let db_path = {
        let container = state.read()?;
        container.db_path.clone()
    };

    tokio::task::spawn_blocking(move || -> ApiResult<()> {
        let conn = crate::db::get_connection(&db_path)?;
        crate::db::upsert_search_query(&conn, &trimmed)?;
        Ok(())
    })
    .await?
}

// ==================== 통계 대시보드 (v2.3) ====================

/// 문서 통계 조회
#[tauri::command]
pub async fn get_document_statistics(
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<DocumentStatistics> {
    let db_path = {
        let container = state.read()?;
        container.db_path.clone()
    };

    tokio::task::spawn_blocking(move || -> ApiResult<DocumentStatistics> {
        let conn = crate::db::get_connection(&db_path)?;

        let total_files = crate::db::get_file_count(&conn)? as i64;
        let indexed_files = crate::db::get_indexed_file_count(&conn)? as i64;
        let total_size = crate::db::get_total_size(&conn)?;

        let file_types = crate::db::get_file_type_distribution(&conn)?
            .into_iter()
            .map(|(t, c)| StatEntry { label: t, count: c })
            .collect();

        let years = crate::db::get_year_distribution(&conn)?
            .into_iter()
            .map(|(y, c)| StatEntry { label: y, count: c })
            .collect();

        let folders = crate::db::get_folder_distribution(&conn)?
            .into_iter()
            .map(|(f, c)| StatEntry { label: f, count: c })
            .collect();

        let recent = crate::db::get_recent_files(&conn, 10)?
            .into_iter()
            .map(|(path, name, modified_at)| FileEntry {
                path,
                name,
                value: modified_at,
            })
            .collect();

        let largest = crate::db::get_largest_files(&conn, 10)?
            .into_iter()
            .map(|(path, name, size)| FileEntry {
                path,
                name,
                value: size,
            })
            .collect();

        Ok(DocumentStatistics {
            total_files,
            indexed_files,
            total_size,
            file_types,
            years,
            folders,
            recent_files: recent,
            largest_files: largest,
        })
    })
    .await?
}

/// 자동완성 제안 항목
#[derive(Debug, serde::Serialize)]
pub struct SuggestionItem {
    pub text: String,
    pub source: String,
    pub frequency: i64,
}

/// 통계 항목
#[derive(Debug, serde::Serialize)]
pub struct StatEntry {
    pub label: String,
    pub count: i64,
}

/// 파일 항목 (최근/최대)
#[derive(Debug, serde::Serialize)]
pub struct FileEntry {
    pub path: String,
    pub name: String,
    pub value: i64,
}

/// 문서 통계 전체
#[derive(Debug, serde::Serialize)]
pub struct DocumentStatistics {
    pub total_files: i64,
    pub indexed_files: i64,
    pub total_size: i64,
    pub file_types: Vec<StatEntry>,
    pub years: Vec<StatEntry>,
    pub folders: Vec<StatEntry>,
    pub recent_files: Vec<FileEntry>,
    pub largest_files: Vec<FileEntry>,
}
