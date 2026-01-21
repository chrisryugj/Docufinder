use crate::commands::settings::get_settings_sync;
use crate::db;
use crate::error::{ApiError, ApiResult};
use crate::search::{filename, fts, hybrid};
use crate::AppState;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::Instant;
use tauri::State;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum MatchType {
    Keyword,
    Semantic,
    Hybrid,
    Filename,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResult {
    pub file_path: String,
    pub file_name: String,
    pub chunk_index: i64,
    pub content_preview: String,
    pub full_content: String,
    pub score: f64,
    /// 정규화된 신뢰도 (0-100)
    pub confidence: u8,
    /// 검색 매칭 타입 (keyword/semantic/hybrid)
    pub match_type: MatchType,
    pub highlight_ranges: Vec<(usize, usize)>,
    pub page_number: Option<i64>,
    pub start_offset: i64,
    /// 위치 힌트 (XLSX: "Sheet1!행1-50", PDF: "페이지 3", HWPX: "섹션 2" 등)
    pub location_hint: Option<String>,
    /// FTS5 snippet - 매칭 컨텍스트 (하이라이트 마커 포함)
    /// [[HL]]매칭[[/HL]] 형식
    pub snippet: Option<String>,
    /// 파일 수정 시간 (Unix timestamp, 초)
    pub modified_at: Option<i64>,
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
) -> ApiResult<SearchResponse> {
    let start = Instant::now();

    if query.trim().is_empty() {
        return Ok(SearchResponse {
            results: vec![],
            total_count: 0,
            search_time_ms: 0,
            search_mode: "keyword".to_string(),
        });
    }

    let (db_path, max_results) = {
        let state = state.lock()?;
        let app_data_dir = state.db_path.parent().map(|p| p.to_path_buf());
        let max_results = app_data_dir
            .as_ref()
            .map(|dir| get_settings_sync(dir).max_results)
            .unwrap_or(50);
        (state.db_path.clone(), max_results)
    };

    let conn = db::get_connection(&db_path)
        .map_err(|e| ApiError::DatabaseConnection(e.to_string()))?;

    // 디버그: DB 상태 확인
    let chunks_count: usize = conn.query_row("SELECT COUNT(*) FROM chunks", [], |r| r.get(0)).unwrap_or(0);
    let fts_count: usize = conn.query_row("SELECT COUNT(*) FROM chunks_fts", [], |r| r.get(0)).unwrap_or(0);
    tracing::info!("DB state: chunks={}, chunks_fts={}", chunks_count, fts_count);

    // FTS5 검색 실행 (page_number, location_hint 포함 - N+1 쿼리 제거)
    let fts_results = fts::search(&conn, &query, max_results)
        .map_err(|e| ApiError::SearchFailed(e.to_string()))?;

    // 스코어 정규화 (BM25 → 0-100 confidence)
    let scores: Vec<f64> = fts_results.iter().map(|r| r.score).collect();
    let confidences = normalize_fts_confidence(&scores);

    // 결과 변환 (snippet 활용, 추가 DB 조회 불필요)
    let results: Vec<SearchResult> = fts_results
        .into_iter()
        .enumerate()
        .map(|(idx, r)| {
            let highlight_ranges = parse_highlight_ranges(&r.highlight);
            SearchResult {
                file_path: r.file_path,
                file_name: r.file_name,
                chunk_index: r.chunk_index,
                content_preview: strip_highlight_markers(&r.snippet),
                full_content: r.content,
                score: r.score,
                confidence: confidences.get(idx).copied().unwrap_or(50),
                match_type: MatchType::Keyword,
                highlight_ranges,
                page_number: r.page_number,
                start_offset: r.start_offset,
                location_hint: r.location_hint,
                snippet: Some(r.snippet),
                modified_at: r.modified_at,
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

/// 파일명 검색 (FTS5)
#[tauri::command]
pub async fn search_filename(
    query: String,
    state: State<'_, Mutex<AppState>>,
) -> ApiResult<SearchResponse> {
    let start = Instant::now();

    if query.trim().is_empty() {
        return Ok(SearchResponse {
            results: vec![],
            total_count: 0,
            search_time_ms: 0,
            search_mode: "filename".to_string(),
        });
    }

    let (db_path, max_results) = {
        let state = state.lock()?;
        let app_data_dir = state.db_path.parent().map(|p| p.to_path_buf());
        let max_results = app_data_dir
            .as_ref()
            .map(|dir| get_settings_sync(dir).max_results)
            .unwrap_or(50);
        (state.db_path.clone(), max_results)
    };

    let conn = db::get_connection(&db_path)
        .map_err(|e| ApiError::DatabaseConnection(e.to_string()))?;

    // 디버그: files vs files_fts 카운트
    let files_count: usize = conn.query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0)).unwrap_or(0);
    let files_fts_count: usize = conn.query_row("SELECT COUNT(*) FROM files_fts", [], |r| r.get(0)).unwrap_or(0);
    // LIKE 검색으로 실제 매칭 수 확인
    let like_pattern = format!("%{}%", query);
    let like_count: usize = conn.query_row(
        "SELECT COUNT(*) FROM files WHERE name LIKE ?",
        [&like_pattern],
        |r| r.get(0)
    ).unwrap_or(0);
    tracing::info!("Filename search DB: files={}, files_fts={}, LIKE match={}", files_count, files_fts_count, like_count);

    // 파일명 FTS5 검색 실행
    let filename_results = filename::search(&conn, &query, max_results)
        .map_err(|e| ApiError::SearchFailed(e.to_string()))?;

    // 스코어 정규화 (BM25 → 0-100 confidence)
    let scores: Vec<f64> = filename_results.iter().map(|r| r.score).collect();
    let confidences = normalize_fts_confidence(&scores);

    // 결과 변환 (파일명 검색은 청크가 없으므로 파일 단위 결과)
    let results: Vec<SearchResult> = filename_results
        .into_iter()
        .enumerate()
        .map(|(idx, r)| {
            let highlight_ranges = parse_highlight_ranges(&r.highlight);
            SearchResult {
                file_path: r.file_path,
                file_name: r.file_name.clone(),
                chunk_index: 0, // 파일명 검색은 청크 없음
                content_preview: r.file_name.clone(), // 파일명 자체가 preview
                full_content: r.file_name,
                score: r.score,
                confidence: confidences.get(idx).copied().unwrap_or(50),
                match_type: MatchType::Filename,
                highlight_ranges,
                page_number: None,
                start_offset: 0,
                location_hint: Some(r.file_type),
                snippet: None,
                modified_at: r.modified_at,
            }
        })
        .collect();

    let total_count = results.len();
    let search_time_ms = start.elapsed().as_millis() as u64;

    tracing::info!(
        "Filename search '{}': {} results in {}ms",
        query,
        total_count,
        search_time_ms
    );

    Ok(SearchResponse {
        results,
        total_count,
        search_time_ms,
        search_mode: "filename".to_string(),
    })
}

/// 시맨틱 검색 (벡터)
#[tauri::command]
pub async fn search_semantic(
    query: String,
    state: State<'_, Mutex<AppState>>,
) -> ApiResult<SearchResponse> {
    let start = Instant::now();

    if query.trim().is_empty() {
        return Ok(SearchResponse {
            results: vec![],
            total_count: 0,
            search_time_ms: 0,
            search_mode: "semantic".to_string(),
        });
    }

    let (db_path, embedder, vector_index, max_results) = {
        let state = state.lock()?;
        let app_data_dir = state.db_path.parent().map(|p| p.to_path_buf());
        let max_results = app_data_dir
            .as_ref()
            .map(|dir| get_settings_sync(dir).max_results)
            .unwrap_or(50);
        (
            state.db_path.clone(),
            state.get_embedder()?,
            state.get_vector_index()?,
            max_results,
        )
    };

    // 벡터 인덱스 상태 확인
    let index_size = vector_index.size();
    let map_size = vector_index.id_map_size();

    tracing::debug!(
        "Semantic search: index_size={}, map_size={}",
        index_size,
        map_size
    );

    if index_size == 0 {
        return Err(ApiError::VectorIndexEmpty);
    }

    if map_size == 0 {
        return Err(ApiError::VectorIndexCorrupted);
    }

    // 1. 쿼리 임베딩 (락 불필요 - 내부 Mutex 사용)
    let query_embedding = embedder
        .embed(&query, true)
        .map_err(|e| ApiError::EmbeddingFailed(e.to_string()))?;

    // 2. 벡터 검색
    let vector_results = vector_index
        .search(&query_embedding, max_results)
        .map_err(|e| ApiError::SearchFailed(e.to_string()))?;

    // 3. chunk_id로 파일 정보 조회
    let conn = db::get_connection(&db_path)
        .map_err(|e| ApiError::DatabaseConnection(e.to_string()))?;
    let chunk_ids: Vec<i64> = vector_results.iter().map(|r| r.chunk_id).collect();
    let chunks = db::get_chunks_by_ids(&conn, &chunk_ids)?;

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
                full_content: chunk.content.clone(),
                score: vr.score as f64,
                confidence: normalize_vector_confidence(vr.score as f64),
                match_type: MatchType::Semantic,
                highlight_ranges: vec![], // 시맨틱 검색은 하이라이트 없음
                page_number: chunk.page_number,
                start_offset: chunk.start_offset,
                location_hint: chunk.location_hint.clone(),
                snippet: None, // 시맨틱 검색은 snippet 없음
                modified_at: chunk.modified_at,
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
) -> ApiResult<SearchResponse> {
    let start = Instant::now();

    if query.trim().is_empty() {
        return Ok(SearchResponse {
            results: vec![],
            total_count: 0,
            search_time_ms: 0,
            search_mode: "hybrid".to_string(),
        });
    }

    let (db_path, embedder, vector_index, max_results) = {
        let state = state.lock()?;
        let app_data_dir = state.db_path.parent().map(|p| p.to_path_buf());
        let max_results = app_data_dir
            .as_ref()
            .map(|dir| get_settings_sync(dir).max_results)
            .unwrap_or(50);
        (
            state.db_path.clone(),
            state.get_embedder().ok(),
            state.get_vector_index().ok(),
            max_results,
        )
    };

    let conn = db::get_connection(&db_path)
        .map_err(|e| ApiError::DatabaseConnection(e.to_string()))?;

    // 1. FTS5 검색
    let fts_results = fts::search(&conn, &query, max_results)
        .map_err(|e| ApiError::SearchFailed(e.to_string()))?;

    // 2. 벡터 검색 (가능한 경우, 락 불필요 - 내부 Mutex 사용)
    let vector_results = match (embedder.as_ref(), vector_index.as_ref()) {
        (Some(emb), Some(vi)) => {
            match emb.embed(&query, true) {
                Ok(query_embedding) => vi.search(&query_embedding, max_results).unwrap_or_default(),
                Err(e) => {
                    tracing::warn!("Failed to embed query: {}", e);
                    vec![]
                }
            }
        }
        _ => vec![],
    };

    // 3. RRF 병합
    let hybrid_results = hybrid::merge_results(fts_results.clone(), vector_results.clone(), 60.0);

    // 4. chunk_id로 파일 정보 조회
    let chunk_ids: Vec<i64> = hybrid_results.iter().map(|r| r.chunk_id).collect();
    let chunks = db::get_chunks_by_ids(&conn, &chunk_ids)?;

    // chunk_id를 키로 하는 맵 생성
    let chunk_map: HashMap<i64, db::ChunkInfo> = chunks
        .into_iter()
        .map(|c| (c.chunk_id, c))
        .collect();

    // FTS 결과에서 snippet 맵 생성
    let fts_snippet_map: HashMap<i64, String> = fts_results
        .iter()
        .map(|r| (r.chunk_id, r.snippet.clone()))
        .collect();
    let fts_highlight_map: HashMap<i64, Vec<(usize, usize)>> = fts_results
        .iter()
        .map(|r| (r.chunk_id, parse_highlight_ranges(&r.highlight)))
        .collect();
    let fts_chunk_ids: HashSet<i64> = fts_results.iter().map(|r| r.chunk_id).collect();
    let vector_chunk_ids: HashSet<i64> = vector_results.iter().map(|r| r.chunk_id).collect();

    // RRF k 상수 (merge_results와 동일하게)
    const RRF_K: f64 = 60.0;

    // 결과 변환 (RRF 순서 유지)
    let results: Vec<SearchResult> = hybrid_results
        .into_iter()
        .filter_map(|hr| {
            chunk_map.get(&hr.chunk_id).map(|chunk| {
                let snippet = fts_snippet_map.get(&hr.chunk_id).cloned();
                let (content_preview, highlight_ranges) = match &snippet {
                    Some(s) => (strip_highlight_markers(s), vec![]),
                    None => (truncate_preview(&chunk.content, 200), vec![]),
                };
                let highlight_ranges = fts_highlight_map
                    .get(&hr.chunk_id)
                    .cloned()
                    .unwrap_or(highlight_ranges);
                let match_type = match (
                    fts_chunk_ids.contains(&hr.chunk_id),
                    vector_chunk_ids.contains(&hr.chunk_id),
                ) {
                    (true, true) => MatchType::Hybrid,
                    (true, false) => MatchType::Keyword,
                    (false, true) => MatchType::Semantic,
                    (false, false) => MatchType::Hybrid,
                };
                SearchResult {
                    file_path: chunk.file_path.clone(),
                    file_name: chunk.file_name.clone(),
                    chunk_index: chunk.chunk_index,
                    content_preview,
                    full_content: chunk.content.clone(),
                    score: hr.score as f64,
                    confidence: normalize_rrf_confidence(hr.score as f64, RRF_K),
                    match_type,
                    highlight_ranges,
                    page_number: chunk.page_number,
                    start_offset: chunk.start_offset,
                    location_hint: chunk.location_hint.clone(),
                    snippet,
                    modified_at: chunk.modified_at,
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

/// snippet에서 하이라이트 마커 제거
/// [[HL]]매칭[[/HL]] → 매칭
fn strip_highlight_markers(snippet: &str) -> String {
    snippet
        .replace("[[HL]]", "")
        .replace("[[/HL]]", "")
}

/// highlight() 결과에서 하이라이트 범위(문자 인덱스) 추출
/// [[HL]]매칭[[/HL]] 형식에서 (시작, 끝) 튜플 반환
fn parse_highlight_ranges(marked: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut clean_pos = 0;
    let mut i = 0;
    let chars: Vec<char> = marked.chars().collect();
    let len = chars.len();

    while i < len {
        if i + 6 <= len && &marked[char_offset(&chars, i)..char_offset(&chars, i + 6)] == "[[HL]]" {
            let start = clean_pos;
            i += 6;
            while i < len {
                if i + 7 <= len && &marked[char_offset(&chars, i)..char_offset(&chars, i + 7)] == "[[/HL]]" {
                    ranges.push((start, clean_pos));
                    i += 7;
                    break;
                }
                clean_pos += 1;
                i += 1;
            }
        } else {
            clean_pos += 1;
            i += 1;
        }
    }

    ranges
}

/// 문자 인덱스를 바이트 오프셋으로 변환
fn char_offset(chars: &[char], char_idx: usize) -> usize {
    chars.iter().take(char_idx).map(|c| c.len_utf8()).sum()
}

// ============================================
// 스코어 정규화 함수들 (0-100 confidence)
// ============================================

/// FTS5 BM25 스코어를 confidence로 변환
/// BM25는 음수이고 낮을수록 좋음 → Min-Max 정규화 후 역전
fn normalize_fts_confidence(scores: &[f64]) -> Vec<u8> {
    if scores.is_empty() {
        return vec![];
    }

    // 모든 점수가 동일한 경우
    let min = scores.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    if (max - min).abs() < f64::EPSILON {
        return vec![100; scores.len()];
    }

    scores
        .iter()
        .map(|&score| {
            // BM25는 낮을수록 좋음 → 역전 (max - score) / (max - min)
            let normalized = (max - score) / (max - min);
            (normalized * 100.0).round().min(100.0).max(0.0) as u8
        })
        .collect()
}

/// 벡터 유사도 스코어를 confidence로 변환
/// 이미 0-1 범위이므로 단순히 100을 곱함
fn normalize_vector_confidence(score: f64) -> u8 {
    (score * 100.0).round().min(100.0).max(0.0) as u8
}

/// RRF 스코어를 confidence로 변환
/// 이론적 최대값: 2/(k+1) (양쪽 모두 1위일 때)
fn normalize_rrf_confidence(score: f64, k: f64) -> u8 {
    let max_possible = 2.0 / (k + 1.0);
    let normalized = (score / max_possible).min(1.0);
    (normalized * 100.0).round().min(100.0).max(0.0) as u8
}
