//! SearchService - 검색 비즈니스 로직
//!
//! 다양한 검색 모드 (keyword, semantic, hybrid, filename)를 처리하고
//! 결과를 정규화된 DTO로 반환합니다.

use crate::application::dto::search::{MatchType, SearchQuery, SearchResponse, SearchResult, SearchMode};
use crate::application::errors::{AppError, AppResult};
use crate::db::{self, ChunkInfo};
use crate::reranker::Reranker;
use crate::search::{filename, fts, hybrid};
use crate::tokenizer::TextTokenizer;
use rusqlite::Connection;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

/// 검색 서비스
pub struct SearchService {
    db_path: PathBuf,
    embedder: Option<Arc<crate::embedder::Embedder>>,
    vector_index: Option<Arc<crate::search::vector::VectorIndex>>,
    tokenizer: Option<Arc<dyn TextTokenizer>>,
    reranker: Option<Arc<Reranker>>,
}

impl SearchService {
    /// 새 SearchService 생성
    pub fn new(
        db_path: PathBuf,
        embedder: Option<Arc<crate::embedder::Embedder>>,
        vector_index: Option<Arc<crate::search::vector::VectorIndex>>,
        tokenizer: Option<Arc<dyn TextTokenizer>>,
        reranker: Option<Arc<Reranker>>,
    ) -> Self {
        Self {
            db_path,
            embedder,
            vector_index,
            tokenizer,
            reranker,
        }
    }

    /// 검색 실행 (모드에 따라 분기)
    pub async fn search(&self, query: SearchQuery) -> AppResult<SearchResponse> {
        if query.query.trim().is_empty() {
            return Ok(SearchResponse::empty(self.mode_to_string(query.mode)));
        }

        match query.mode {
            SearchMode::Keyword => self.search_keyword(&query.query, query.max_results).await,
            SearchMode::Semantic => self.search_semantic(&query.query, query.max_results).await,
            SearchMode::Hybrid => self.search_hybrid(&query.query, query.max_results).await,
            SearchMode::Filename => self.search_filename(&query.query, query.max_results).await,
        }
    }

    /// 키워드 검색 (FTS5)
    pub async fn search_keyword(&self, query: &str, max_results: usize) -> AppResult<SearchResponse> {
        let start = Instant::now();

        let conn = self.get_connection()?;

        // FTS5 검색 실행 (한국어 형태소 분석 포함)
        let use_tokenizer = self.tokenizer.is_some();
        let fts_results = match self.tokenizer.as_ref() {
            Some(tok) => fts::search_with_tokenizer(&conn, query, max_results, tok.as_ref())
                .map_err(|e| AppError::SearchFailed(e.to_string()))?,
            None => fts::search(&conn, query, max_results)
                .map_err(|e| AppError::SearchFailed(e.to_string()))?,
        };

        // 스코어 정규화
        let scores: Vec<f64> = fts_results.iter().map(|r| r.score).collect();
        let confidences = normalize_fts_confidence(&scores);

        // 결과 변환
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
            "Keyword search '{}': {} results in {}ms (tokenizer={})",
            query, total_count, search_time_ms, use_tokenizer
        );

        Ok(SearchResponse {
            results,
            total_count,
            search_time_ms,
            search_mode: "keyword".to_string(),
        })
    }

    /// 파일명 검색 (FTS5)
    pub async fn search_filename(&self, query: &str, max_results: usize) -> AppResult<SearchResponse> {
        let start = Instant::now();

        let conn = self.get_connection()?;

        // 파일명 FTS5 검색 실행
        let filename_results = filename::search(&conn, query, max_results)
            .map_err(|e| AppError::SearchFailed(e.to_string()))?;

        // 스코어 정규화
        let scores: Vec<f64> = filename_results.iter().map(|r| r.score).collect();
        let confidences = normalize_fts_confidence(&scores);

        // 결과 변환
        let results: Vec<SearchResult> = filename_results
            .into_iter()
            .enumerate()
            .map(|(idx, r)| {
                let highlight_ranges = parse_highlight_ranges(&r.highlight);
                SearchResult {
                    file_path: r.file_path,
                    file_name: r.file_name.clone(),
                    chunk_index: 0,
                    content_preview: r.file_name.clone(),
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
            query, total_count, search_time_ms
        );

        Ok(SearchResponse {
            results,
            total_count,
            search_time_ms,
            search_mode: "filename".to_string(),
        })
    }

    /// 시맨틱 검색 (벡터)
    pub async fn search_semantic(&self, query: &str, max_results: usize) -> AppResult<SearchResponse> {
        let start = Instant::now();

        let embedder = self.embedder.as_ref()
            .ok_or(AppError::SemanticSearchDisabled)?;
        let vector_index = self.vector_index.as_ref()
            .ok_or(AppError::SemanticSearchDisabled)?;

        // 벡터 인덱스 상태 확인
        if vector_index.size() == 0 {
            return Err(AppError::VectorIndexEmpty);
        }

        // 쿼리 임베딩 (락 불필요 - &self로 호출)
        let query_embedding = embedder
            .embed(query, true)
            .map_err(|e| AppError::EmbeddingFailed(e.to_string()))?;

        // 벡터 검색
        let vector_results = vector_index
            .search(&query_embedding, max_results)
            .map_err(|e| AppError::SearchFailed(e.to_string()))?;

        // chunk_id로 파일 정보 조회
        let conn = self.get_connection()?;
        let chunk_ids: Vec<i64> = vector_results.iter().map(|r| r.chunk_id).collect();
        let chunks = db::get_chunks_by_ids(&conn, &chunk_ids)
            .map_err(|e| AppError::SearchFailed(e.to_string()))?;

        let chunk_map: HashMap<i64, ChunkInfo> = chunks
            .into_iter()
            .map(|c| (c.chunk_id, c))
            .collect();

        // 결과 변환
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
                    highlight_ranges: vec![],
                    page_number: chunk.page_number,
                    start_offset: chunk.start_offset,
                    location_hint: chunk.location_hint.clone(),
                    snippet: None,
                    modified_at: chunk.modified_at,
                })
            })
            .collect();

        let total_count = results.len();
        let search_time_ms = start.elapsed().as_millis() as u64;

        tracing::info!(
            "Semantic search '{}': {} results in {}ms",
            query, total_count, search_time_ms
        );

        Ok(SearchResponse {
            results,
            total_count,
            search_time_ms,
            search_mode: "semantic".to_string(),
        })
    }

    /// 하이브리드 검색 (FTS + 벡터 + RRF + Reranking)
    pub async fn search_hybrid(&self, query: &str, max_results: usize) -> AppResult<SearchResponse> {
        let start = Instant::now();
        let use_tokenizer = self.tokenizer.is_some();
        let use_reranker = self.reranker.is_some();

        let conn = self.get_connection()?;

        // 1. FTS5 검색 (한국어 형태소 분석 포함)
        let fts_results = match self.tokenizer.as_ref() {
            Some(tok) => fts::search_with_tokenizer(&conn, query, max_results, tok.as_ref())
                .map_err(|e| AppError::SearchFailed(e.to_string()))?,
            None => fts::search(&conn, query, max_results)
                .map_err(|e| AppError::SearchFailed(e.to_string()))?,
        };

        // 2. 벡터 검색 (가능한 경우, 락 불필요)
        let vector_results = match (self.embedder.as_ref(), self.vector_index.as_ref()) {
            (Some(emb), Some(vi)) => {
                match emb.embed(query, true) {
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
        const RRF_K: f32 = 60.0;
        let mut hybrid_results = hybrid::merge_results(fts_results.clone(), vector_results.clone(), RRF_K);

        // 4. Cross-Encoder Reranking (상위 20개만)
        const RERANK_TOP_K: usize = 20;
        if let Some(rr) = self.reranker.as_ref() {
            if hybrid_results.len() > 1 {
                // chunk_id로 콘텐츠 조회
                let rerank_chunk_ids: Vec<i64> = hybrid_results
                    .iter()
                    .take(RERANK_TOP_K)
                    .map(|r| r.chunk_id)
                    .collect();
                let rerank_chunks = db::get_chunks_by_ids(&conn, &rerank_chunk_ids)
                    .map_err(|e| AppError::SearchFailed(e.to_string()))?;
                let rerank_chunk_map: HashMap<i64, ChunkInfo> = rerank_chunks
                    .into_iter()
                    .map(|c| (c.chunk_id, c))
                    .collect();

                // Reranking 대상 문서 추출
                let documents: Vec<&str> = hybrid_results
                    .iter()
                    .take(RERANK_TOP_K)
                    .filter_map(|r| rerank_chunk_map.get(&r.chunk_id).map(|c| c.content.as_str()))
                    .collect();

                if !documents.is_empty() {
                    match rr.rerank(query, &documents, documents.len()) {
                        Ok(reranked_indices) => {
                            // 상위 K개만 재정렬
                            let top_results: Vec<_> = hybrid_results.drain(..documents.len().min(RERANK_TOP_K)).collect();
                            let mut reranked: Vec<_> = reranked_indices
                                .into_iter()
                                .filter_map(|idx| top_results.get(idx).cloned())
                                .collect();
                            // 재정렬된 결과 + 나머지 결과
                            reranked.extend(hybrid_results);
                            hybrid_results = reranked;
                            tracing::debug!("Reranked top {} results", RERANK_TOP_K);
                        }
                        Err(e) => {
                            tracing::warn!("Reranking failed: {}", e);
                        }
                    }
                }
            }
        }

        // 5. chunk_id로 파일 정보 조회
        let chunk_ids: Vec<i64> = hybrid_results.iter().map(|r| r.chunk_id).collect();
        let chunks = db::get_chunks_by_ids(&conn, &chunk_ids)
            .map_err(|e| AppError::SearchFailed(e.to_string()))?;

        let chunk_map: HashMap<i64, ChunkInfo> = chunks
            .into_iter()
            .map(|c| (c.chunk_id, c))
            .collect();

        // FTS/벡터 결과 맵 생성
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

        // 결과 변환
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
                        confidence: normalize_rrf_confidence(hr.score as f64, RRF_K as f64),
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
            "Hybrid search '{}': {} results in {}ms (tokenizer={}, reranker={})",
            query, total_count, search_time_ms, use_tokenizer, use_reranker
        );

        Ok(SearchResponse {
            results,
            total_count,
            search_time_ms,
            search_mode: "hybrid".to_string(),
        })
    }

    // ============================================
    // Private Helpers
    // ============================================

    fn get_connection(&self) -> AppResult<Connection> {
        db::get_connection(&self.db_path)
            .map_err(|e| AppError::Internal(format!("DB connection failed: {}", e)))
    }

    fn mode_to_string(&self, mode: SearchMode) -> &'static str {
        match mode {
            SearchMode::Keyword => "keyword",
            SearchMode::Semantic => "semantic",
            SearchMode::Hybrid => "hybrid",
            SearchMode::Filename => "filename",
        }
    }
}

// ============================================
// Helper Functions
// ============================================

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
fn strip_highlight_markers(snippet: &str) -> String {
    snippet
        .replace("[[HL]]", "")
        .replace("[[/HL]]", "")
}

/// highlight() 결과에서 하이라이트 범위 추출
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

/// FTS5 BM25 스코어를 confidence로 변환
fn normalize_fts_confidence(scores: &[f64]) -> Vec<u8> {
    if scores.is_empty() {
        return vec![];
    }

    let min = scores.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    if (max - min).abs() < f64::EPSILON {
        return vec![100; scores.len()];
    }

    scores
        .iter()
        .map(|&score| {
            let normalized = (max - score) / (max - min);
            (normalized * 100.0).round().min(100.0).max(0.0) as u8
        })
        .collect()
}

/// 벡터 유사도 스코어를 confidence로 변환
fn normalize_vector_confidence(score: f64) -> u8 {
    (score * 100.0).round().min(100.0).max(0.0) as u8
}

/// RRF 스코어를 confidence로 변환
fn normalize_rrf_confidence(score: f64, k: f64) -> u8 {
    let max_possible = 2.0 / (k + 1.0);
    let normalized = (score / max_possible).min(1.0);
    (normalized * 100.0).round().min(100.0).max(0.0) as u8
}
