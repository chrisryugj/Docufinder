//! Search Commands - Thin Layer (Clean Architecture)
//!
//! Tauri commands that delegate to SearchService.
//! Handles only: input validation, settings retrieval, service invocation.

use crate::application::dto::search::SearchResponse;
use crate::application::services::search_service::helpers::collapse_by_lineage;
use crate::error::{ApiError, ApiResult};
use crate::search::KeywordMode;
use crate::AppContainer;
use std::sync::RwLock;
use tauri::State;

const MAX_QUERY_LEN: usize = 1000;

/// `group_versions` 설정이 켜져 있을 때 검색 결과에서 같은 lineage의 중복 버전을 접는다.
fn apply_lineage_collapse(mut response: SearchResponse, group_versions: bool) -> SearchResponse {
    if group_versions {
        let before = response.results.len();
        response.results = collapse_by_lineage(response.results);
        response.total_count = response.results.len();
        let after = response.total_count;
        if before != after {
            tracing::debug!("lineage collapse: {} → {} ({} 개 버전 접힘)", before, after, before - after);
        }
    }
    response
}

fn parse_keyword_mode(mode: Option<&str>) -> KeywordMode {
    match mode {
        Some("or") => KeywordMode::Or,
        Some("exact") => KeywordMode::Exact,
        _ => KeywordMode::And,
    }
}

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
    keyword_mode: Option<String>,
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

    let mode = parse_keyword_mode(keyword_mode.as_deref());

    let (service, max_results, group_versions) = {
        let container = state.read()?;
        let s = container.get_settings();
        (container.search_service(), s.max_results, s.group_versions)
    };

    let response = service
        .search_keyword_with_mode(&query, max_results, folder_scope.as_deref(), mode)
        .await
        .map_err(ApiError::from)?;
    Ok(apply_lineage_collapse(response, group_versions))
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

    let (service, max_results, group_versions) = {
        let container = state.read()?;
        let s = container.get_settings();
        (container.search_service(), s.max_results, s.group_versions)
    };

    let response = service
        .search_filename(&query, max_results, folder_scope.as_deref())
        .await
        .map_err(ApiError::from)?;
    Ok(apply_lineage_collapse(response, group_versions))
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

    let (service, max_results, semantic_enabled, group_versions) = {
        let container = state.read()?;
        let settings = container.get_settings();
        (
            container.search_service(),
            settings.max_results,
            settings.semantic_search_enabled,
            settings.group_versions,
        )
    };

    if !semantic_enabled {
        return Err(ApiError::SemanticSearchDisabled);
    }

    let response = service
        .search_semantic(&query, max_results, folder_scope.as_deref())
        .await
        .map_err(ApiError::from)?;
    Ok(apply_lineage_collapse(response, group_versions))
}

/// 하이브리드 검색 (FTS + 벡터 + RRF + Reranking)
/// 시맨틱 비활성화 시 키워드 검색으로 폴백
#[tauri::command]
pub async fn search_hybrid(
    query: String,
    folder_scope: Option<String>,
    keyword_mode: Option<String>,
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

    let mode = parse_keyword_mode(keyword_mode.as_deref());

    let (service, max_results, semantic_enabled, group_versions) = {
        let container = state.read()?;
        let settings = container.get_settings();
        (
            container.search_service(),
            settings.max_results,
            settings.semantic_search_enabled,
            settings.group_versions,
        )
    };

    // 시맨틱 비활성화 시 키워드 검색으로 폴백
    if !semantic_enabled {
        let response = service
            .search_keyword_with_mode(&query, max_results, folder_scope.as_deref(), mode)
            .await
            .map_err(ApiError::from)?;
        return Ok(apply_lineage_collapse(response, group_versions));
    }

    let response = service
        .search_hybrid_with_mode(&query, max_results, folder_scope.as_deref(), mode)
        .await
        .map_err(ApiError::from)?;
    Ok(apply_lineage_collapse(response, group_versions))
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

    let (service, max_results, group_versions) = {
        let container = state.read()?;
        let s = container.get_settings();
        (container.search_service(), s.max_results, s.group_versions)
    };

    let mut response = service
        .search_smart(&query, max_results, folder_scope.as_deref())
        .await
        .map_err(ApiError::from)?;
    if group_versions {
        let before = response.results.len();
        response.results = collapse_by_lineage(response.results);
        response.total_count = response.results.len();
        if before != response.total_count {
            tracing::debug!(
                "smart lineage collapse: {} → {}",
                before,
                response.total_count
            );
        }
    }
    Ok(response)
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

// ==================== 검색어 히스토리 저장 ====================

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

/// 검색 히스토리 통계
#[derive(Debug, serde::Serialize)]
pub struct SearchHistoryStats {
    /// 총 검색 횟수 (sum of frequency)
    pub total_searches: i64,
    /// 고유 검색어 수
    pub unique_queries: i64,
    /// 자주 검색한 키워드 TOP 20
    pub top_queries: Vec<QueryStat>,
    /// 최근 검색어 TOP 20
    pub recent_queries: Vec<QueryStat>,
}

#[derive(Debug, serde::Serialize)]
pub struct QueryStat {
    pub query: String,
    pub frequency: i64,
    pub last_searched_at: i64,
}

/// 검색 히스토리 통계 조회
#[tauri::command]
pub async fn get_search_history_stats(
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<SearchHistoryStats> {
    let db_path = {
        let container = state.read()?;
        container.db_path.clone()
    };

    tokio::task::spawn_blocking(move || -> ApiResult<SearchHistoryStats> {
        let conn = crate::db::get_connection(&db_path)?;

        // 총 검색 횟수
        let total_searches: i64 = conn
            .query_row("SELECT COALESCE(SUM(frequency), 0) FROM search_queries", [], |row| row.get(0))
            .unwrap_or(0);

        // 고유 검색어 수
        let unique_queries: i64 = conn
            .query_row("SELECT COUNT(*) FROM search_queries", [], |row| row.get(0))
            .unwrap_or(0);

        // 자주 검색한 TOP 20
        let mut stmt = conn
            .prepare("SELECT query, frequency, last_searched_at FROM search_queries ORDER BY frequency DESC LIMIT 20")
            .map_err(|e| ApiError::DatabaseQuery(e.to_string()))?;
        let top_queries: Vec<QueryStat> = stmt
            .query_map([], |row| {
                Ok(QueryStat {
                    query: row.get(0)?,
                    frequency: row.get(1)?,
                    last_searched_at: row.get(2)?,
                })
            })
            .map_err(|e| ApiError::DatabaseQuery(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        // 최근 검색어 TOP 20
        let mut stmt2 = conn
            .prepare("SELECT query, frequency, last_searched_at FROM search_queries ORDER BY last_searched_at DESC LIMIT 20")
            .map_err(|e| ApiError::DatabaseQuery(e.to_string()))?;
        let recent_queries: Vec<QueryStat> = stmt2
            .query_map([], |row| {
                Ok(QueryStat {
                    query: row.get(0)?,
                    frequency: row.get(1)?,
                    last_searched_at: row.get(2)?,
                })
            })
            .map_err(|e| ApiError::DatabaseQuery(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(SearchHistoryStats {
            total_searches,
            unique_queries,
            top_queries,
            recent_queries,
        })
    })
    .await?
}
