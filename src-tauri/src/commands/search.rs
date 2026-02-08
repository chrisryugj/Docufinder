//! Search Commands - Thin Layer (Clean Architecture)
//!
//! Tauri commands that delegate to SearchService.
//! Handles only: input validation, settings retrieval, service invocation.

use crate::application::dto::search::SearchResponse;
use crate::error::{ApiError, ApiResult};
use crate::AppContainer;
use std::sync::Mutex;
use tauri::State;

/// 키워드 검색 (FTS5)
#[tauri::command]
pub async fn search_keyword(
    query: String,
    state: State<'_, Mutex<AppContainer>>,
) -> ApiResult<SearchResponse> {
    if query.trim().is_empty() {
        return Ok(SearchResponse {
            results: vec![],
            total_count: 0,
            search_time_ms: 0,
            search_mode: "keyword".to_string(),
        });
    }

    let (service, max_results) = {
        let container = state.lock()?;
        let max_results = container.get_settings().max_results;
        (container.search_service(), max_results)
    };

    service
        .search_keyword(&query, max_results)
        .await
        .map_err(ApiError::from)
}

/// 파일명 검색 (FTS5)
#[tauri::command]
pub async fn search_filename(
    query: String,
    state: State<'_, Mutex<AppContainer>>,
) -> ApiResult<SearchResponse> {
    if query.trim().is_empty() {
        return Ok(SearchResponse {
            results: vec![],
            total_count: 0,
            search_time_ms: 0,
            search_mode: "filename".to_string(),
        });
    }

    let (service, max_results) = {
        let container = state.lock()?;
        let max_results = container.get_settings().max_results;
        (container.search_service(), max_results)
    };

    service
        .search_filename(&query, max_results)
        .await
        .map_err(ApiError::from)
}

/// 시맨틱 검색 (벡터)
#[tauri::command]
pub async fn search_semantic(
    query: String,
    state: State<'_, Mutex<AppContainer>>,
) -> ApiResult<SearchResponse> {
    if query.trim().is_empty() {
        return Ok(SearchResponse {
            results: vec![],
            total_count: 0,
            search_time_ms: 0,
            search_mode: "semantic".to_string(),
        });
    }

    let (service, max_results, semantic_enabled) = {
        let container = state.lock()?;
        let settings = container.get_settings();
        (container.search_service(), settings.max_results, settings.semantic_search_enabled)
    };

    if !semantic_enabled {
        return Err(ApiError::SemanticSearchDisabled);
    }

    service
        .search_semantic(&query, max_results)
        .await
        .map_err(ApiError::from)
}

/// 하이브리드 검색 (FTS + 벡터 + RRF + Reranking)
/// 시맨틱 비활성화 시 키워드 검색으로 폴백
#[tauri::command]
pub async fn search_hybrid(
    query: String,
    state: State<'_, Mutex<AppContainer>>,
) -> ApiResult<SearchResponse> {
    if query.trim().is_empty() {
        return Ok(SearchResponse {
            results: vec![],
            total_count: 0,
            search_time_ms: 0,
            search_mode: "hybrid".to_string(),
        });
    }

    let (service, max_results, semantic_enabled) = {
        let container = state.lock()?;
        let settings = container.get_settings();
        (container.search_service(), settings.max_results, settings.semantic_search_enabled)
    };

    // 시맨틱 비활성화 시 키워드 검색으로 폴백
    if !semantic_enabled {
        return service
            .search_keyword(&query, max_results)
            .await
            .map_err(ApiError::from);
    }

    service
        .search_hybrid(&query, max_results)
        .await
        .map_err(ApiError::from)
}
