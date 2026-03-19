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
    if query.len() > MAX_QUERY_LEN {
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
        (container.search_service(), container.db_path.to_string_lossy().to_string())
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

        Ok(chunks.first().map(|c| c.content.clone()).unwrap_or_default())
    })
    .await??;

    if text.is_empty() {
        return Ok("기타".to_string());
    }

    service.classify_document(&text).map_err(ApiError::from)
}
