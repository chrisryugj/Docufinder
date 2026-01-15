use crate::db;
use crate::search::{fts, hybrid};
use crate::AppState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;
use tauri::State;

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResult {
    pub file_path: String,
    pub file_name: String,
    pub chunk_index: i64,
    pub content_preview: String,
    pub score: f64,
    pub highlight_ranges: Vec<(usize, usize)>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
    pub total_count: usize,
    pub search_time_ms: u64,
    pub search_mode: String,
}

/// 키워드 검색 (FTS5)
#[tauri::command]
pub async fn search_keyword(
    query: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<SearchResponse, String> {
    let start = Instant::now();

    if query.trim().is_empty() {
        return Ok(SearchResponse {
            results: vec![],
            total_count: 0,
            search_time_ms: 0,
            search_mode: "keyword".to_string(),
        });
    }

    let db_path = {
        let state = state.lock().map_err(|e| e.to_string())?;
        state.db_path.clone()
    };

    let conn = db::get_connection(&db_path).map_err(|e| e.to_string())?;

    // FTS5 검색 실행
    let fts_results = fts::search(&conn, &query, 50).map_err(|e| e.to_string())?;

    // 결과 변환
    let results: Vec<SearchResult> = fts_results
        .into_iter()
        .map(|r| {
            let highlight_ranges = fts::find_highlight_ranges(&r.content, &query);
            SearchResult {
                file_path: r.file_path,
                file_name: r.file_name,
                chunk_index: r.chunk_index,
                content_preview: truncate_preview(&r.content, 200),
                score: r.score,
                highlight_ranges,
            }
        })
        .collect();

    let total_count = results.len();
    let search_time_ms = start.elapsed().as_millis() as u64;

    tracing::info!(
        "Keyword search '{}': {} results in {}ms",
        query,
        total_count,
        search_time_ms
    );

    Ok(SearchResponse {
        results,
        total_count,
        search_time_ms,
        search_mode: "keyword".to_string(),
    })
}

/// 시맨틱 검색 (벡터)
#[tauri::command]
pub async fn search_semantic(
    query: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<SearchResponse, String> {
    let start = Instant::now();

    if query.trim().is_empty() {
        return Ok(SearchResponse {
            results: vec![],
            total_count: 0,
            search_time_ms: 0,
            search_mode: "semantic".to_string(),
        });
    }

    let (db_path, embedder, vector_index) = {
        let state = state.lock().map_err(|e| e.to_string())?;
        (
            state.db_path.clone(),
            state.get_embedder()?,
            state.get_vector_index()?,
        )
    };

    // 1. 쿼리 임베딩
    let query_embedding = embedder
        .embed(&query, true)
        .map_err(|e| e.to_string())?;

    // 2. 벡터 검색
    let vector_results = vector_index
        .search(&query_embedding, 50)
        .map_err(|e| e.to_string())?;

    // 3. chunk_id로 파일 정보 조회
    let conn = db::get_connection(&db_path).map_err(|e| e.to_string())?;
    let chunk_ids: Vec<i64> = vector_results.iter().map(|r| r.chunk_id).collect();
    let chunks = db::get_chunks_by_ids(&conn, &chunk_ids).map_err(|e| e.to_string())?;

    // chunk_id를 키로 하는 맵 생성
    let chunk_map: HashMap<i64, db::ChunkInfo> = chunks
        .into_iter()
        .map(|c| (c.chunk_id, c))
        .collect();

    // 결과 변환 (벡터 검색 순서 유지)
    let results: Vec<SearchResult> = vector_results
        .into_iter()
        .filter_map(|vr| {
            chunk_map.get(&vr.chunk_id).map(|chunk| SearchResult {
                file_path: chunk.file_path.clone(),
                file_name: chunk.file_name.clone(),
                chunk_index: chunk.chunk_index,
                content_preview: truncate_preview(&chunk.content, 200),
                score: vr.score as f64,
                highlight_ranges: vec![], // 시맨틱 검색은 하이라이트 없음
            })
        })
        .collect();

    let total_count = results.len();
    let search_time_ms = start.elapsed().as_millis() as u64;

    tracing::info!(
        "Semantic search '{}': {} results in {}ms",
        query,
        total_count,
        search_time_ms
    );

    Ok(SearchResponse {
        results,
        total_count,
        search_time_ms,
        search_mode: "semantic".to_string(),
    })
}

/// 하이브리드 검색 (FTS + 벡터 + RRF)
#[tauri::command]
pub async fn search_hybrid(
    query: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<SearchResponse, String> {
    let start = Instant::now();

    if query.trim().is_empty() {
        return Ok(SearchResponse {
            results: vec![],
            total_count: 0,
            search_time_ms: 0,
            search_mode: "hybrid".to_string(),
        });
    }

    let (db_path, embedder, vector_index) = {
        let state = state.lock().map_err(|e| e.to_string())?;
        (
            state.db_path.clone(),
            state.get_embedder().ok(),
            state.get_vector_index().ok(),
        )
    };

    let conn = db::get_connection(&db_path).map_err(|e| e.to_string())?;

    // 1. FTS5 검색
    let fts_results = fts::search(&conn, &query, 50).map_err(|e| e.to_string())?;

    // 2. 벡터 검색 (가능한 경우)
    let vector_results = match (embedder.as_ref(), vector_index.as_ref()) {
        (Some(emb), Some(vi)) => {
            match emb.embed(&query, true) {
                Ok(query_embedding) => vi.search(&query_embedding, 50).unwrap_or_default(),
                Err(e) => {
                    tracing::warn!("Failed to embed query: {}", e);
                    vec![]
                }
            }
        }
        _ => vec![],
    };

    // 3. RRF 병합
    let hybrid_results = hybrid::merge_results(fts_results.clone(), vector_results, 60.0);

    // 4. chunk_id로 파일 정보 조회
    let chunk_ids: Vec<i64> = hybrid_results.iter().map(|r| r.chunk_id).collect();
    let chunks = db::get_chunks_by_ids(&conn, &chunk_ids).map_err(|e| e.to_string())?;

    // chunk_id를 키로 하는 맵 생성
    let chunk_map: HashMap<i64, db::ChunkInfo> = chunks
        .into_iter()
        .map(|c| (c.chunk_id, c))
        .collect();

    // 결과 변환 (RRF 순서 유지)
    let results: Vec<SearchResult> = hybrid_results
        .into_iter()
        .filter_map(|hr| {
            chunk_map.get(&hr.chunk_id).map(|chunk| {
                let highlight_ranges = fts::find_highlight_ranges(&chunk.content, &query);
                SearchResult {
                    file_path: chunk.file_path.clone(),
                    file_name: chunk.file_name.clone(),
                    chunk_index: chunk.chunk_index,
                    content_preview: truncate_preview(&chunk.content, 200),
                    score: hr.score as f64,
                    highlight_ranges,
                }
            })
        })
        .collect();

    let total_count = results.len();
    let search_time_ms = start.elapsed().as_millis() as u64;

    tracing::info!(
        "Hybrid search '{}': {} results in {}ms",
        query,
        total_count,
        search_time_ms
    );

    Ok(SearchResponse {
        results,
        total_count,
        search_time_ms,
        search_mode: "hybrid".to_string(),
    })
}

/// 미리보기 텍스트 자르기
fn truncate_preview(content: &str, max_len: usize) -> String {
    if content.chars().count() <= max_len {
        content.to_string()
    } else {
        let truncated: String = content.chars().take(max_len).collect();
        format!("{}...", truncated)
    }
}
