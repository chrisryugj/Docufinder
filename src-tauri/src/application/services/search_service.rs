//! SearchService - 검색 비즈니스 로직
//!
//! 다양한 검색 모드 (keyword, semantic, hybrid, filename)를 처리하고
//! 결과를 정규화된 DTO로 반환합니다.

use crate::application::dto::search::{
    MatchType, SearchMode, SearchQuery, SearchResponse, SearchResult, SmartSearchResponse,
};
use crate::application::errors::{AppError, AppResult};
use crate::db::{self, ChunkInfo};
use crate::reranker::Reranker;
use crate::search::{filename, filename_cache::FilenameCache, fts, hybrid, sentence};
use crate::tokenizer::TextTokenizer;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

/// 시맨틱 검색 결과 enrich 설정
/// snippet이 없는 벡터 전용 결과에 가장 유사한 문장 하이라이트 추가
const SEMANTIC_ENRICH_MAX_RESULTS: usize = 5;

/// 벡터 검색 결과의 folder_scope 후처리 필터 (usearch는 DB path 필터 불가)
/// Windows: case-insensitive 비교
fn matches_folder_scope(file_path: &str, folder_scope: Option<&str>) -> bool {
    match folder_scope {
        Some(scope) if !scope.is_empty() => {
            file_path.to_lowercase().starts_with(&scope.to_lowercase())
        }
        _ => true,
    }
}

/// 검색 서비스
pub struct SearchService {
    db_path: PathBuf,
    embedder: Option<Arc<crate::embedder::Embedder>>,
    vector_index: Option<Arc<crate::search::vector::VectorIndex>>,
    tokenizer: Option<Arc<dyn TextTokenizer>>,
    reranker: Option<Arc<Reranker>>,
    /// 파일명 캐시 (인메모리 빠른 검색)
    filename_cache: Option<Arc<FilenameCache>>,
}

impl SearchService {
    /// 새 SearchService 생성
    pub fn new(
        db_path: PathBuf,
        embedder: Option<Arc<crate::embedder::Embedder>>,
        vector_index: Option<Arc<crate::search::vector::VectorIndex>>,
        tokenizer: Option<Arc<dyn TextTokenizer>>,
        reranker: Option<Arc<Reranker>>,
        filename_cache: Option<Arc<FilenameCache>>,
    ) -> Self {
        Self {
            db_path,
            embedder,
            vector_index,
            tokenizer,
            reranker,
            filename_cache,
        }
    }

    /// 검색 실행 (모드에 따라 분기)
    pub async fn search(&self, query: SearchQuery) -> AppResult<SearchResponse> {
        if query.query.trim().is_empty() {
            return Ok(SearchResponse::empty(self.mode_to_string(query.mode)));
        }

        match query.mode {
            SearchMode::Keyword => {
                self.search_keyword(&query.query, query.max_results, None)
                    .await
            }
            SearchMode::Semantic => {
                self.search_semantic(&query.query, query.max_results, None)
                    .await
            }
            SearchMode::Hybrid => {
                self.search_hybrid(&query.query, query.max_results, None)
                    .await
            }
            SearchMode::Filename => {
                self.search_filename(&query.query, query.max_results, None)
                    .await
            }
        }
    }

    /// 키워드 검색 (FTS5)
    pub async fn search_keyword(
        &self,
        query: &str,
        max_results: usize,
        folder_scope: Option<&str>,
    ) -> AppResult<SearchResponse> {
        let start = Instant::now();

        let conn = self.get_connection()?;

        // FTS5 검색 실행 (한국어 형태소 분석 포함)
        let use_tokenizer = self.tokenizer.is_some();
        let fts_results = match self.tokenizer.as_ref() {
            Some(tok) => {
                fts::search_with_tokenizer(&conn, query, max_results, tok.as_ref(), folder_scope)
                    .map_err(|e| AppError::SearchFailed(e.to_string()))?
            }
            None => fts::search(&conn, query, max_results, folder_scope)
                .map_err(|e| AppError::SearchFailed(e.to_string()))?,
        };

        // 스코어 정규화
        let scores: Vec<f64> = fts_results.iter().map(|r| r.score).collect();
        let confidences = normalize_fts_confidence(&scores);

        // 결과 변환 + 키워드 위치 기반 페이지 보간 (page_start~page_end 내에서)
        // snippet에 키워드가 없으면 content에서 찾아 대체
        let results: Vec<SearchResult> = fts_results
            .into_iter()
            .enumerate()
            .map(|(idx, r)| {
                let page_number = interpolate_page_from_snippet(
                    r.page_number,
                    r.page_end,
                    &r.content,
                    &r.snippet,
                );
                let improved = ensure_keyword_in_snippet(&r.snippet, &r.content, query);
                let highlight_ranges = parse_highlight_ranges(&improved);
                let content_preview = strip_highlight_markers(&improved);
                SearchResult {
                    file_path: r.file_path,
                    file_name: r.file_name,
                    chunk_index: r.chunk_index,
                    content_preview,
                    full_content: r.content,
                    score: r.score,
                    confidence: confidences.get(idx).copied().unwrap_or(50),
                    match_type: MatchType::Keyword,
                    highlight_ranges,
                    page_number,
                    start_offset: r.start_offset,
                    location_hint: r.location_hint,
                    snippet: Some(improved),
                    modified_at: r.modified_at,
                    has_hwp_pair: false,
                }
            })
            .collect();

        let total_count = results.len();
        let search_time_ms = start.elapsed().as_millis() as u64;

        tracing::debug!(
            "Keyword search '{}': {} results in {}ms (tokenizer={})",
            query,
            total_count,
            search_time_ms,
            use_tokenizer
        );

        Ok(SearchResponse {
            results,
            total_count,
            search_time_ms,
            search_mode: "keyword".to_string(),
        })
    }

    /// 파일명 검색 (캐시 우선, fallback: LIKE 검색)
    pub async fn search_filename(
        &self,
        query: &str,
        max_results: usize,
        folder_scope: Option<&str>,
    ) -> AppResult<SearchResponse> {
        let start = Instant::now();

        // 캐시 사용 (있고, 비어있지 않고, truncated 아닐 때만)
        let use_cache = self
            .filename_cache
            .as_ref()
            .is_some_and(|c| !c.is_empty() && !c.is_truncated());

        let results: Vec<SearchResult> = if use_cache {
            // ⚡ 인메모리 캐시 검색 (~5ms)
            let cache = match self.filename_cache.as_ref() {
                Some(c) => c,
                None => {
                    return Ok(SearchResponse {
                        results: vec![],
                        total_count: 0,
                        search_time_ms: start.elapsed().as_millis() as u64,
                        search_mode: "filename".to_string(),
                    })
                }
            };
            let cache_results = cache.search_with_scope(query, max_results, folder_scope);

            cache_results
                .into_iter()
                .map(|r| {
                    let name = r.name().to_owned();
                    SearchResult {
                        file_path: r.path.into(),
                        file_name: name.clone(),
                        chunk_index: 0,
                        content_preview: name.clone(),
                        full_content: String::new(),
                        score: 1.0,
                        confidence: 100,
                        match_type: MatchType::Filename,
                        highlight_ranges: vec![],
                        page_number: None,
                        start_offset: 0,
                        location_hint: Some(r.file_type.into()),
                        snippet: Some(name),
                        modified_at: Some(r.modified_at),
                        has_hwp_pair: false,
                    }
                })
                .collect()
        } else {
            // Fallback: DB LIKE 검색
            let conn = self.get_connection()?;
            let filename_results = filename::search(&conn, query, max_results, folder_scope)
                .map_err(|e| AppError::SearchFailed(e.to_string()))?;

            let scores: Vec<f64> = filename_results.iter().map(|r| r.score).collect();
            let confidences = normalize_fts_confidence(&scores);

            filename_results
                .into_iter()
                .enumerate()
                .map(|(idx, r)| SearchResult {
                    file_path: r.file_path,
                    file_name: r.file_name.clone(),
                    chunk_index: 0,
                    content_preview: r.file_name.clone(),
                    full_content: String::new(),
                    score: r.score,
                    confidence: confidences.get(idx).copied().unwrap_or(50),
                    match_type: MatchType::Filename,
                    highlight_ranges: vec![],
                    page_number: None,
                    start_offset: 0,
                    location_hint: Some(r.file_type),
                    snippet: Some(r.file_name),
                    modified_at: r.modified_at,
                    has_hwp_pair: false,
                })
                .collect()
        };

        // HWP/HWPX 중복 제거: 같은 디렉토리에 동명 .hwpx가 있으면 .hwp 숨김
        let results = Self::dedup_hwp_hwpx(results);

        let total_count = results.len();
        let search_time_ms = start.elapsed().as_millis() as u64;

        tracing::debug!(
            "Filename search '{}': {} results in {}ms (cache={})",
            query,
            total_count,
            search_time_ms,
            use_cache
        );

        Ok(SearchResponse {
            results,
            total_count,
            search_time_ms,
            search_mode: "filename".to_string(),
        })
    }

    /// 시맨틱 검색 (벡터)
    pub async fn search_semantic(
        &self,
        query: &str,
        max_results: usize,
        folder_scope: Option<&str>,
    ) -> AppResult<SearchResponse> {
        let start = Instant::now();

        let embedder = self
            .embedder
            .as_ref()
            .ok_or(AppError::SemanticSearchDisabled)?;
        let vector_index = self
            .vector_index
            .as_ref()
            .ok_or(AppError::SemanticSearchDisabled)?;

        // 벡터 인덱스 상태 확인
        if vector_index.size() == 0 {
            return Err(AppError::VectorIndexEmpty);
        }

        // 쿼리 임베딩 (락 불필요 - &self로 호출)
        let query_embedding = embedder
            .embed(query, true)
            .map_err(|e| AppError::EmbeddingFailed(e.to_string()))?;

        // 벡터 검색 (folder_scope 후처리 필터로 인한 결과 누락 방지: over-fetch)
        let vector_fetch_limit = if folder_scope.is_some() {
            max_results * 5
        } else {
            max_results
        };
        let vector_results = vector_index
            .search(&query_embedding, vector_fetch_limit)
            .map_err(|e| AppError::SearchFailed(e.to_string()))?;

        // chunk_id로 파일 정보 조회
        let conn = self.get_connection()?;
        let chunk_ids: Vec<i64> = vector_results.iter().map(|r| r.chunk_id).collect();
        let chunks = db::get_chunks_by_ids(&conn, &chunk_ids)
            .map_err(|e| AppError::SearchFailed(e.to_string()))?;

        let chunk_map: HashMap<i64, ChunkInfo> =
            chunks.into_iter().map(|c| (c.chunk_id, c)).collect();

        // 결과 변환 (⚡ full_content 제거) + folder_scope 후처리 필터
        let mut results: Vec<SearchResult> = vector_results
            .into_iter()
            .filter_map(|vr| {
                chunk_map.get(&vr.chunk_id).and_then(|chunk| {
                    if !matches_folder_scope(&chunk.file_path, folder_scope) {
                        return None;
                    }
                    Some(SearchResult {
                        file_path: chunk.file_path.clone(),
                        file_name: chunk.file_name.clone(),
                        chunk_index: chunk.chunk_index,
                        content_preview: truncate_preview(&chunk.content, 200),
                        full_content: String::new(), // ⚡ 성능 최적화
                        score: vr.score as f64,
                        confidence: normalize_vector_confidence(vr.score as f64),
                        match_type: MatchType::Semantic,
                        highlight_ranges: vec![],
                        page_number: chunk.page_number,
                        start_offset: chunk.start_offset,
                        location_hint: chunk.location_hint.clone(),
                        snippet: Some(truncate_preview(&chunk.content, 200)), // snippet 추가
                        modified_at: chunk.modified_at,
                        has_hwp_pair: false,
                    })
                })
            })
            .collect();

        // 시맨틱 결과에 가장 유사한 문장 추가
        if let Err(e) = self.enrich_semantic_results(&mut results, &query_embedding) {
            tracing::warn!("Semantic enrichment failed: {}", e);
        }

        let total_count = results.len();
        let search_time_ms = start.elapsed().as_millis() as u64;

        tracing::debug!(
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

    /// 하이브리드 검색 (FTS + 벡터 + RRF + Reranking)
    pub async fn search_hybrid(
        &self,
        query: &str,
        max_results: usize,
        folder_scope: Option<&str>,
    ) -> AppResult<SearchResponse> {
        let start = Instant::now();
        let use_tokenizer = self.tokenizer.is_some();
        let use_reranker = self.reranker.is_some();

        let conn = self.get_connection()?;

        // 1. FTS5 검색 (한국어 형태소 분석 포함)
        let fts_results = match self.tokenizer.as_ref() {
            Some(tok) => {
                fts::search_with_tokenizer(&conn, query, max_results, tok.as_ref(), folder_scope)
                    .map_err(|e| AppError::SearchFailed(e.to_string()))?
            }
            None => fts::search(&conn, query, max_results, folder_scope)
                .map_err(|e| AppError::SearchFailed(e.to_string()))?,
        };

        // 2. 벡터 검색 (folder_scope 후처리 필터 대비 over-fetch)
        let vector_fetch_limit = if folder_scope.is_some() {
            max_results * 5
        } else {
            max_results
        };
        let (vector_results, query_embedding) =
            match (self.embedder.as_ref(), self.vector_index.as_ref()) {
                (Some(emb), Some(vi)) => match emb.embed(query, true) {
                    Ok(qe) => {
                        let results = vi.search(&qe, vector_fetch_limit).unwrap_or_default();
                        (results, Some(qe))
                    }
                    Err(e) => {
                        tracing::warn!("Failed to embed query: {}", e);
                        (vec![], None)
                    }
                },
                _ => (vec![], None),
            };

        // 3. FTS 결과를 HashMap으로 변환 (DB 중복 조회 제거)
        // FtsResult에 이미 content, file_path 등 모든 정보가 있음
        let fts_map: HashMap<i64, &fts::FtsResult> =
            fts_results.iter().map(|r| (r.chunk_id, r)).collect();
        // vector_chunk_ids만 유지 (매치 타입 판별용)
        let vector_chunk_ids: HashSet<i64> = vector_results.iter().map(|r| r.chunk_id).collect();

        // 4. RRF 병합 (슬라이스 참조로 clone 제거)
        // k=15: 소규모 데이터셋(max_results 20~50)에서 순위 차이가 더 뚜렷해짐
        const RRF_K: f32 = 15.0;
        let mut hybrid_results = hybrid::merge_results(&fts_results, &vector_results, RRF_K);

        // 5. 벡터 전용 결과의 content를 미리 확보 (reranking에 필요)
        let pre_rerank_vector_only_ids: Vec<i64> = hybrid_results
            .iter()
            .filter(|r| !fts_map.contains_key(&r.chunk_id))
            .map(|r| r.chunk_id)
            .collect();
        let pre_rerank_vector_chunks: HashMap<i64, ChunkInfo> =
            if !pre_rerank_vector_only_ids.is_empty() {
                db::get_chunks_by_ids(&conn, &pre_rerank_vector_only_ids)
                    .map_err(|e| AppError::SearchFailed(e.to_string()))?
                    .into_iter()
                    .map(|c| (c.chunk_id, c))
                    .collect()
            } else {
                HashMap::new()
            };

        // 6. Cross-Encoder Reranking (상위 40개 — 재현율 향상, MiniLM-L6 ~1ms/후보)
        const RERANK_TOP_K: usize = 40;
        if let Some(rr) = self.reranker.as_ref() {
            if hybrid_results.len() > 1 {
                let top_k = hybrid_results.len().min(RERANK_TOP_K);
                let top_results: Vec<_> = hybrid_results.drain(..top_k).collect();

                // FTS + 벡터전용 결과 모두 reranking 대상으로 포함
                let rerank_candidates: Vec<(usize, &str)> = top_results
                    .iter()
                    .enumerate()
                    .filter_map(|(i, r)| {
                        // FTS 결과에서 content 가져오기
                        if let Some(f) = fts_map.get(&r.chunk_id) {
                            return Some((i, f.content.as_str()));
                        }
                        // 벡터 전용 결과에서 content 가져오기
                        if let Some(c) = pre_rerank_vector_chunks.get(&r.chunk_id) {
                            return Some((i, c.content.as_str()));
                        }
                        None
                    })
                    .collect();

                let mut did_rerank = false;
                if !rerank_candidates.is_empty() {
                    let documents: Vec<&str> = rerank_candidates.iter().map(|(_, c)| *c).collect();
                    if let Ok(reranked_indices) = rr.rerank(query, &documents, documents.len()) {
                        let rerank_candidate_indices: Vec<usize> =
                            rerank_candidates.iter().map(|(i, _)| *i).collect();
                        let mut reranked = apply_reranked_top_results(
                            top_results.clone(),
                            &rerank_candidate_indices,
                            &reranked_indices,
                        );
                        reranked.extend(hybrid_results);
                        hybrid_results = reranked;
                        did_rerank = true;
                        tracing::debug!("Reranked top {} results (including vector-only)", top_k);
                    } else {
                        tracing::warn!("Reranking failed, using original order");
                    }
                }
                if !did_rerank {
                    let mut restored = top_results;
                    restored.extend(hybrid_results);
                    hybrid_results = restored;
                }
            }
        }

        // 7. 벡터 전용 결과 DB 조회 (pre_rerank에서 이미 조회한 것 재사용)
        let vector_only_ids: Vec<i64> = hybrid_results
            .iter()
            .filter(|r| !fts_map.contains_key(&r.chunk_id))
            .map(|r| r.chunk_id)
            .collect();

        // pre_rerank에서 이미 조회한 결과 재사용, 누락분만 추가 조회
        let mut vector_only_chunks = pre_rerank_vector_chunks;
        {
            let missing_ids: Vec<i64> = vector_only_ids
                .iter()
                .filter(|id| !vector_only_chunks.contains_key(id))
                .copied()
                .collect();
            if !missing_ids.is_empty() {
                let extra = db::get_chunks_by_ids(&conn, &missing_ids)
                    .map_err(|e| AppError::SearchFailed(e.to_string()))?;
                for c in extra {
                    vector_only_chunks.insert(c.chunk_id, c);
                }
            }
        }

        // 결과 변환 (FTS 결과 우선, 벡터 전용은 DB 조회 결과 사용)
        let mut results: Vec<SearchResult> = hybrid_results
            .into_iter()
            .filter_map(|hr| {
                let match_type = match (
                    fts_map.contains_key(&hr.chunk_id),
                    vector_chunk_ids.contains(&hr.chunk_id),
                ) {
                    (true, true) => MatchType::Hybrid,
                    (true, false) => MatchType::Keyword,
                    (false, true) => MatchType::Semantic,
                    (false, false) => MatchType::Hybrid,
                };

                // FTS 결과에서 직접 가져오기 (DB 조회 불필요)
                // snippet에 키워드가 없으면 content에서 찾아 대체
                if let Some(fts_r) = fts_map.get(&hr.chunk_id) {
                    let page_number = interpolate_page_from_snippet(
                        fts_r.page_number,
                        fts_r.page_end,
                        &fts_r.content,
                        &fts_r.snippet,
                    );
                    let improved = ensure_keyword_in_snippet(&fts_r.snippet, &fts_r.content, query);
                    let content_preview = strip_highlight_markers(&improved);
                    let highlight_ranges = parse_highlight_ranges(&improved);

                    Some(SearchResult {
                        file_path: fts_r.file_path.clone(),
                        file_name: fts_r.file_name.clone(),
                        chunk_index: fts_r.chunk_index,
                        content_preview,
                        full_content: fts_r.content.clone(),
                        score: hr.score as f64,
                        confidence: normalize_rrf_confidence(hr.score as f64, RRF_K as f64),
                        match_type,
                        highlight_ranges,
                        page_number,
                        start_offset: fts_r.start_offset,
                        location_hint: fts_r.location_hint.clone(),
                        snippet: Some(improved),
                        modified_at: fts_r.modified_at,
                        has_hwp_pair: false,
                    })
                } else {
                    vector_only_chunks.get(&hr.chunk_id).and_then(|chunk| {
                        if !matches_folder_scope(&chunk.file_path, folder_scope) {
                            return None;
                        }
                        // 벡터 전용 결과 (DB 조회 결과 사용, ⚡ full_content 제거)
                        // snippet: None → enrich_semantic_results에서 문장 하이라이트 추가
                        Some(SearchResult {
                            file_path: chunk.file_path.clone(),
                            file_name: chunk.file_name.clone(),
                            chunk_index: chunk.chunk_index,
                            content_preview: truncate_preview(&chunk.content, 200),
                            full_content: String::new(), // ⚡ 성능 최적화
                            score: hr.score as f64,
                            confidence: normalize_rrf_confidence(hr.score as f64, RRF_K as f64),
                            match_type,
                            highlight_ranges: vec![],
                            page_number: chunk.page_number,
                            start_offset: chunk.start_offset,
                            location_hint: chunk.location_hint.clone(),
                            snippet: None,
                            modified_at: chunk.modified_at,
                            has_hwp_pair: false,
                        })
                    })
                }
            })
            .collect();

        // 시맨틱 결과에 가장 유사한 문장 추가 (snippet이 없는 결과만)
        if let Some(qe) = query_embedding.as_ref() {
            if let Err(e) = self.enrich_semantic_results(&mut results, qe) {
                tracing::warn!("Hybrid semantic enrichment failed: {}", e);
            }
        }

        let total_count = results.len();
        let search_time_ms = start.elapsed().as_millis() as u64;

        tracing::debug!(
            "Hybrid search '{}': {} results in {}ms (tokenizer={}, reranker={})",
            query,
            total_count,
            search_time_ms,
            use_tokenizer,
            use_reranker
        );

        Ok(SearchResponse {
            results,
            total_count,
            search_time_ms,
            search_mode: "hybrid".to_string(),
        })
    }

    // ============================================
    // Smart (Natural Language) Search
    // ============================================

    /// 필터 전용 검색: 키워드 없이 날짜/파일타입 필터만으로 최근 문서 조회
    async fn browse_recent_files(
        &self,
        max_results: usize,
        folder_scope: Option<&str>,
    ) -> AppResult<SearchResponse> {
        let conn = self.get_connection()?;

        let scope_clause = folder_scope
            .map(|s| {
                let escaped = s
                    .replace('\\', "\\\\")
                    .replace('%', "\\%")
                    .replace('_', "\\_");
                format!(" AND f.path LIKE '{}%' ESCAPE '\\\\'", escaped)
            })
            .unwrap_or_default();

        let sql = format!(
            "SELECT f.path, f.name, f.file_type, f.size, f.modified_at
             FROM files f
             WHERE f.modified_at IS NOT NULL {}
             ORDER BY f.modified_at DESC
             LIMIT ?1",
            scope_clause
        );

        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| AppError::SearchFailed(e.to_string()))?;

        let results: Vec<SearchResult> = stmt
            .query_map(rusqlite::params![max_results as i64], |row| {
                Ok(SearchResult {
                    file_path: row.get(0)?,
                    file_name: row.get(1)?,
                    chunk_index: 0,
                    content_preview: String::new(),
                    full_content: String::new(),
                    score: 1.0,
                    confidence: 50,
                    match_type: MatchType::Keyword,
                    highlight_ranges: vec![],
                    page_number: None,
                    start_offset: 0,
                    location_hint: None,
                    snippet: None,
                    modified_at: row.get(4)?,
                    has_hwp_pair: false,
                })
            })
            .map_err(|e| AppError::SearchFailed(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        let total_count = results.len();
        Ok(SearchResponse {
            results,
            total_count,
            search_time_ms: 0,
            search_mode: "browse".to_string(),
        })
    }

    /// 자연어 검색: NL 파서 → 하이브리드 검색 위임 → 후처리 필터
    pub async fn search_smart(
        &self,
        query: &str,
        max_results: usize,
        folder_scope: Option<&str>,
    ) -> AppResult<SmartSearchResponse> {
        use crate::search::nl_query::NlQueryParser;

        let start = Instant::now();
        let parsed = NlQueryParser::parse(query);

        let has_filters = parsed.date_filter.is_some()
            || parsed.file_type.is_some()
            || !parsed.exclude_keywords.is_empty();

        if parsed.keywords.trim().is_empty() && !has_filters {
            return Ok(SmartSearchResponse {
                results: vec![],
                total_count: 0,
                search_time_ms: 0,
                parsed_query: parsed,
            });
        }

        // 키워드 없이 필터만 있는 경우: 최근 수정 파일 기반 검색
        // 키워드 있는 경우: 하이브리드 검색 후 필터 적용
        let over_fetch = if has_filters {
            max_results * 10
        } else {
            max_results * 3
        };

        let base = if parsed.keywords.trim().is_empty() {
            // 필터만 있는 경우: 최근 파일 목록에서 필터링
            self.browse_recent_files(over_fetch, folder_scope).await?
        } else if self.embedder.is_some() && self.vector_index.is_some() {
            self.search_hybrid(&parsed.keywords, over_fetch, folder_scope)
                .await?
        } else {
            self.search_keyword(&parsed.keywords, over_fetch, folder_scope)
                .await?
        };

        // 후처리 필터
        let now = chrono::Utc::now().timestamp();
        let filtered: Vec<SearchResult> = base
            .results
            .into_iter()
            .filter(|r| smart_apply_date_filter(r, &parsed.date_filter, now))
            .filter(|r| smart_apply_file_type_filter(r, &parsed.file_type))
            .filter(|r| smart_apply_exclude_filter(r, &parsed.exclude_keywords))
            .take(max_results)
            .collect();

        let total_count = filtered.len();
        let search_time_ms = start.elapsed().as_millis() as u64;

        tracing::debug!(
            "Smart search '{}': parsed keywords='{}', {} results in {}ms",
            query,
            parsed.keywords,
            total_count,
            search_time_ms
        );

        Ok(SmartSearchResponse {
            results: filtered,
            total_count,
            search_time_ms,
            parsed_query: parsed,
        })
    }

    // ============================================
    // Semantic Enrichment
    // ============================================

    /// 시맨틱 검색 결과에 가장 유사한 문장 추가
    ///
    /// 각 청크를 문장으로 분리하고, 쿼리 임베딩과 가장 유사한 문장을 찾아
    /// snippet 필드에 [[HL]]...[[/HL]] 형식으로 추가합니다.
    fn enrich_semantic_results(
        &self,
        results: &mut [SearchResult],
        query_embedding: &[f32],
    ) -> AppResult<()> {
        let embedder = match self.embedder.as_ref() {
            Some(e) => e,
            None => return Ok(()),
        };

        // 처리할 결과 제한 (성능)
        let results_to_process = results.len().min(SEMANTIC_ENRICH_MAX_RESULTS);

        // 1. 모든 청크에서 문장 추출
        // (result_idx, sentence_text, start, end)
        let mut all_sentences: Vec<(usize, String, usize, usize)> = Vec::new();

        for (idx, result) in results.iter().take(results_to_process).enumerate() {
            // 이미 FTS 하이라이트 snippet이 있으면 스킵
            if result
                .snippet
                .as_ref()
                .is_some_and(|s| s.contains("[[HL]]"))
            {
                continue;
            }

            // ⚡ full_content 대신 content_preview 사용 (성능 최적화)
            let sentences = sentence::split_sentences(&result.content_preview);
            for sent in sentences {
                all_sentences.push((idx, sent.text, sent.start, sent.end));
            }
        }

        if all_sentences.is_empty() {
            return Ok(());
        }

        // 2. 배치 임베딩
        let texts: Vec<String> = all_sentences.iter().map(|(_, t, _, _)| t.clone()).collect();
        let embeddings = match embedder.embed_batch(&texts) {
            Ok(emb) => emb,
            Err(e) => {
                tracing::warn!("Semantic enrichment embedding failed: {}", e);
                return Ok(());
            }
        };

        // 3. 각 청크별 최고 유사도 문장 선택
        let mut best_per_result: HashMap<usize, (String, f32, usize, usize)> = HashMap::new();

        for ((result_idx, sentence_text, start, end), embedding) in
            all_sentences.iter().zip(embeddings.iter())
        {
            let sim = sentence::cosine_similarity(query_embedding, embedding);

            best_per_result
                .entry(*result_idx)
                .and_modify(|e| {
                    if sim > e.1 {
                        *e = (sentence_text.clone(), sim, *start, *end);
                    }
                })
                .or_insert((sentence_text.clone(), sim, *start, *end));
        }

        // 4. 결과에 snippet 추가
        let enriched_count = best_per_result.len();
        for (idx, (sentence_text, _sim, start, end)) in best_per_result {
            if let Some(result) = results.get_mut(idx) {
                // snippet에 하이라이트 마커 추가
                result.snippet = Some(format!("[[HL]]{}[[/HL]]", sentence_text));
                // highlight_ranges는 content_preview 내 위치
                result.highlight_ranges = vec![(start, end)];
            }
        }

        tracing::debug!(
            "Enriched {} semantic results with best sentences",
            enriched_count
        );

        Ok(())
    }

    // ============================================
    // 유사 문서 검색
    // ============================================

    /// 주어진 파일과 유사한 문서를 벡터 검색으로 찾기
    pub async fn find_similar(
        &self,
        file_path: &str,
        max_results: usize,
    ) -> AppResult<SearchResponse> {
        let start = Instant::now();

        let embedder = self
            .embedder
            .as_ref()
            .ok_or(AppError::SemanticSearchDisabled)?;
        let vector_index = self
            .vector_index
            .as_ref()
            .ok_or(AppError::SemanticSearchDisabled)?;

        if vector_index.size() == 0 {
            return Err(AppError::VectorIndexEmpty);
        }

        let conn = self.get_connection()?;

        // 1. 원본 파일의 청크 조회
        let source_chunk_ids = db::get_chunk_ids_for_path(&conn, file_path)
            .map_err(|e| AppError::SearchFailed(e.to_string()))?;

        if source_chunk_ids.is_empty() {
            return Ok(SearchResponse::empty("similar"));
        }

        let source_chunks = db::get_chunks_by_ids(&conn, &source_chunk_ids)
            .map_err(|e| AppError::SearchFailed(e.to_string()))?;

        // 2. 각 청크 임베딩 → 평균 벡터 계산 (문서 대표 벡터)
        let texts: Vec<String> = source_chunks
            .iter()
            .take(10) // 최대 10개 청크만 (성능)
            .map(|c| c.content.clone())
            .collect();

        if texts.is_empty() {
            return Ok(SearchResponse::empty("similar"));
        }

        let embeddings = embedder
            .embed_batch(&texts)
            .map_err(|e| AppError::EmbeddingFailed(e.to_string()))?;

        // 평균 벡터
        let dim = embeddings[0].len();
        let mut avg_embedding = vec![0.0f32; dim];
        for emb in &embeddings {
            for (i, v) in emb.iter().enumerate() {
                avg_embedding[i] += v;
            }
        }
        let count = embeddings.len() as f32;
        for v in &mut avg_embedding {
            *v /= count;
        }
        // L2 정규화
        let norm: f32 = avg_embedding.iter().map(|v| v * v).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in &mut avg_embedding {
                *v /= norm;
            }
        }

        // 3. 벡터 검색 (over-fetch)
        let vector_results = vector_index
            .search(&avg_embedding, max_results * 5)
            .map_err(|e| AppError::SearchFailed(e.to_string()))?;

        // 4. 결과에서 원본 파일 제외 + 파일 단위 중복 제거
        let source_ids: HashSet<i64> = source_chunk_ids.into_iter().collect();
        let result_chunk_ids: Vec<i64> = vector_results
            .iter()
            .filter(|r| !source_ids.contains(&r.chunk_id))
            .map(|r| r.chunk_id)
            .collect();

        let chunks = db::get_chunks_by_ids(&conn, &result_chunk_ids)
            .map_err(|e| AppError::SearchFailed(e.to_string()))?;

        let chunk_map: HashMap<i64, ChunkInfo> =
            chunks.into_iter().map(|c| (c.chunk_id, c)).collect();

        // 파일별 최고 스코어 청크만 유지
        let mut file_best: HashMap<String, (f64, SearchResult)> = HashMap::new();
        for vr in &vector_results {
            if source_ids.contains(&vr.chunk_id) {
                continue;
            }
            if let Some(chunk) = chunk_map.get(&vr.chunk_id) {
                let score = vr.score as f64;
                let entry = file_best.entry(chunk.file_path.clone());
                entry
                    .and_modify(|(best_score, best_result)| {
                        if score > *best_score {
                            *best_score = score;
                            *best_result = SearchResult {
                                file_path: chunk.file_path.clone(),
                                file_name: chunk.file_name.clone(),
                                chunk_index: chunk.chunk_index,
                                content_preview: truncate_preview(&chunk.content, 200),
                                full_content: String::new(),
                                score,
                                confidence: normalize_vector_confidence(score),
                                match_type: MatchType::Semantic,
                                highlight_ranges: vec![],
                                page_number: chunk.page_number,
                                start_offset: chunk.start_offset,
                                location_hint: chunk.location_hint.clone(),
                                snippet: Some(truncate_preview(&chunk.content, 200)),
                                modified_at: chunk.modified_at,
                                has_hwp_pair: false,
                            };
                        }
                    })
                    .or_insert_with(|| {
                        (
                            score,
                            SearchResult {
                                file_path: chunk.file_path.clone(),
                                file_name: chunk.file_name.clone(),
                                chunk_index: chunk.chunk_index,
                                content_preview: truncate_preview(&chunk.content, 200),
                                full_content: String::new(),
                                score,
                                confidence: normalize_vector_confidence(score),
                                match_type: MatchType::Semantic,
                                highlight_ranges: vec![],
                                page_number: chunk.page_number,
                                start_offset: chunk.start_offset,
                                location_hint: chunk.location_hint.clone(),
                                snippet: Some(truncate_preview(&chunk.content, 200)),
                                modified_at: chunk.modified_at,
                                has_hwp_pair: false,
                            },
                        )
                    });
            }
        }

        let mut results: Vec<SearchResult> = file_best.into_values().map(|(_, r)| r).collect();
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(max_results);

        let total_count = results.len();
        let search_time_ms = start.elapsed().as_millis() as u64;

        tracing::debug!(
            "Similar search for '{}': {} results in {}ms",
            file_path,
            total_count,
            search_time_ms
        );

        Ok(SearchResponse {
            results,
            total_count,
            search_time_ms,
            search_mode: "similar".to_string(),
        })
    }

    // ============================================
    // 문서 자동 분류
    // ============================================

    /// 문서 첫 청크 기반 카테고리 분류 (키워드 패턴 매칭)
    ///
    /// 임베딩 유사도 대신 키워드 패턴으로 분류 — 빠르고 정확하며
    /// 시맨틱 검색이 꺼져 있어도 동작함.
    pub fn classify_document(&self, text: &str) -> AppResult<String> {
        Ok(Self::classify_by_keyword(text))
    }

    /// 키워드 패턴 매칭 기반 문서 분류 (순수 함수)
    pub fn classify_by_keyword(text: &str) -> String {
        // "제N조" 패턴 카운트 (법령 핵심 지표)
        let article_count = count_article_pattern(text);

        // 각 카테고리별 (키워드 목록, 필요 점수)
        let rules: &[(&str, &[&str], usize)] = &[
            (
                "법령",
                &[
                    "시행령",
                    "시행규칙",
                    "조례",
                    "법률 제",
                    "별표",
                    "동법",
                    "같은 법",
                ],
                2,
            ),
            (
                "공문",
                &[
                    "수신 :",
                    "수신:",
                    "발신 :",
                    "발신:",
                    "관인생략",
                    "시행 20",
                    "경유 :",
                ],
                2,
            ),
            (
                "기안문",
                &["기안자", "기안일자", "검토자", "결재일자", "협조자"],
                2,
            ),
            (
                "회의록",
                &[
                    "회의록",
                    "참석자",
                    "안건",
                    "결정사항",
                    "회의일시",
                    "회의 장소",
                ],
                2,
            ),
            (
                "보고서",
                &["보고서", "서론", "결론", "목차", "Ⅰ.", "Ⅱ.", "요약"],
                2,
            ),
            (
                "계획서",
                &["사업계획", "추진계획", "추진일정", "세부계획", "실행계획"],
                2,
            ),
            (
                "통계",
                &["전년대비", "전년동기", "증감률", "증감", "백분율"],
                2,
            ),
        ];

        for (category, keywords, threshold) in rules {
            let mut score: usize = 0;

            // 법령은 "제N조" 패턴에 가중치
            if *category == "법령" {
                score += article_count.min(5);
            }

            for kw in *keywords {
                if text.contains(kw) {
                    score += 1;
                }
            }

            if score >= *threshold {
                return category.to_string();
            }
        }

        "기타".to_string()
    }

    // ============================================
    // Private Helpers
    // ============================================

    fn get_connection(&self) -> AppResult<db::PooledConnection> {
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

    /// HWP/HWPX 중복 제거: 같은 디렉토리에 동명 .hwpx가 있으면 .hwp 결과 숨김
    /// HWPX 결과에는 has_hwp_pair = true 표시
    fn dedup_hwp_hwpx(mut results: Vec<SearchResult>) -> Vec<SearchResult> {
        use std::collections::HashSet;
        use std::path::Path;

        // 1. 결과에 포함된 HWPX 파일의 stem+dir 집합 구축
        let hwpx_stems: HashSet<String> = results
            .iter()
            .filter(|r| {
                r.file_name
                    .rsplit('.')
                    .next()
                    .map(|e| e.eq_ignore_ascii_case("hwpx"))
                    .unwrap_or(false)
            })
            .filter_map(|r| {
                let p = Path::new(&r.file_path);
                let dir = p.parent()?.to_string_lossy().to_lowercase();
                let stem = p.file_stem()?.to_string_lossy().to_lowercase();
                Some(format!("{}|{}", dir, stem))
            })
            .collect();

        if hwpx_stems.is_empty() {
            return results;
        }

        // 2. HWPX 결과에 has_hwp_pair 표시 (HWP가 존재하는지 확인)
        let hwp_stems: HashSet<String> = results
            .iter()
            .filter(|r| {
                r.file_name
                    .rsplit('.')
                    .next()
                    .map(|e| e.eq_ignore_ascii_case("hwp"))
                    .unwrap_or(false)
            })
            .filter_map(|r| {
                let p = Path::new(&r.file_path);
                let dir = p.parent()?.to_string_lossy().to_lowercase();
                let stem = p.file_stem()?.to_string_lossy().to_lowercase();
                Some(format!("{}|{}", dir, stem))
            })
            .collect();

        for r in &mut results {
            if r.file_name
                .rsplit('.')
                .next()
                .map(|e| e.eq_ignore_ascii_case("hwpx"))
                .unwrap_or(false)
            {
                let p = Path::new(&r.file_path);
                if let (Some(dir), Some(stem)) = (p.parent(), p.file_stem()) {
                    let key = format!(
                        "{}|{}",
                        dir.to_string_lossy().to_lowercase(),
                        stem.to_string_lossy().to_lowercase()
                    );
                    if hwp_stems.contains(&key) {
                        r.has_hwp_pair = true;
                    }
                }
            }
        }

        // 3. HWP 중 대응 HWPX가 있는 것 제거
        results.retain(|r| {
            let ext = r.file_name.rsplit('.').next().unwrap_or("");
            if !ext.eq_ignore_ascii_case("hwp") {
                return true;
            }
            let p = Path::new(&r.file_path);
            if let (Some(dir), Some(stem)) = (p.parent(), p.file_stem()) {
                let key = format!(
                    "{}|{}",
                    dir.to_string_lossy().to_lowercase(),
                    stem.to_string_lossy().to_lowercase()
                );
                !hwpx_stems.contains(&key)
            } else {
                true
            }
        });

        results
    }
}

fn apply_reranked_top_results<T: Clone>(
    top_results: Vec<T>,
    rerank_candidate_indices: &[usize],
    reranked_indices: &[usize],
) -> Vec<T> {
    let mut appended = HashSet::new();
    let mut reordered = Vec::with_capacity(top_results.len());

    for &idx in reranked_indices {
        if let Some(&orig_idx) = rerank_candidate_indices.get(idx) {
            if appended.insert(orig_idx) {
                reordered.push(top_results[orig_idx].clone());
            }
        }
    }

    for &orig_idx in rerank_candidate_indices {
        if appended.insert(orig_idx) {
            reordered.push(top_results[orig_idx].clone());
        }
    }

    for (idx, result) in top_results.iter().cloned().enumerate() {
        if !appended.contains(&idx) {
            reordered.push(result);
        }
    }

    reordered
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
    snippet.replace("[[HL]]", "").replace("[[/HL]]", "")
}

/// FTS5 snippet에 키워드가 없을 때 content에서 키워드를 찾아 커스텀 snippet 생성
///
/// 반환: "...앞문맥[[HL]]키워드[[/HL]]뒷문맥..." 형식
fn create_keyword_snippet(content: &str, query: &str) -> Option<String> {
    let query_trimmed = query.trim();
    if query_trimmed.is_empty() || content.is_empty() {
        return None;
    }

    let query_lower = query_trimmed.to_lowercase();
    let content_lower = content.to_lowercase();

    // 바이트 위치 → 문자 위치 변환 (한국어 안전)
    let byte_pos = content_lower.find(&query_lower)?;
    let char_pos = content_lower[..byte_pos].chars().count();
    let kw_char_len = query_trimmed.chars().count();

    let content_chars: Vec<char> = content.chars().collect();
    let total_chars = content_chars.len();

    if char_pos + kw_char_len > total_chars {
        return None;
    }

    // 컨텍스트: 40자 전, 140자 후 (프론트엔드 기본값과 동일)
    let start = char_pos.saturating_sub(40);
    let end = (char_pos + kw_char_len + 140).min(total_chars);

    let before: String = content_chars[start..char_pos].iter().collect();
    let keyword: String = content_chars[char_pos..char_pos + kw_char_len]
        .iter()
        .collect();
    let after: String = content_chars[char_pos + kw_char_len..end].iter().collect();

    let prefix = if start > 0 { "..." } else { "" };
    let suffix = if end < total_chars { "..." } else { "" };

    Some(format!(
        "{}{}[[HL]]{}[[/HL]]{}{}",
        prefix, before, keyword, after, suffix
    ))
}

/// FTS5 snippet에 검색 키워드가 포함되어 있지 않으면 content에서 찾아 대체
///
/// 1. snippet에 키워드가 있으면 → 원본 반환
/// 2. content에서 전체 쿼리 찾기 → 커스텀 snippet
/// 3. content에서 개별 키워드 찾기 → 커스텀 snippet
/// 4. 모두 실패 → 원본 반환
fn ensure_keyword_in_snippet(fts_snippet: &str, content: &str, query: &str) -> String {
    let query_trimmed = query.trim();
    if query_trimmed.is_empty() {
        return fts_snippet.to_string();
    }

    let stripped_lower = strip_highlight_markers(fts_snippet).to_lowercase();
    let keywords: Vec<&str> = query_trimmed.split_whitespace().collect();

    // snippet에 이미 키워드가 있으면 그대로 사용
    if keywords
        .iter()
        .any(|kw| stripped_lower.contains(&kw.to_lowercase()))
    {
        return fts_snippet.to_string();
    }

    // content에서 전체 쿼리 찾기
    if let Some(snippet) = create_keyword_snippet(content, query_trimmed) {
        return snippet;
    }

    // 개별 키워드 시도
    for kw in &keywords {
        if let Some(snippet) = create_keyword_snippet(content, kw) {
            return snippet;
        }
    }

    fts_snippet.to_string()
}

/// highlight() 결과에서 하이라이트 범위 추출 (O(n) 최적화)
fn parse_highlight_ranges(marked: &str) -> Vec<(usize, usize)> {
    const HL_START: &str = "[[HL]]";
    const HL_END: &str = "[[/HL]]";

    let mut ranges = Vec::new();
    let mut clean_pos = 0;
    let mut rest = marked;

    while !rest.is_empty() {
        if let Some(pos) = rest.find(HL_START) {
            // HL_START 이전 문자 수 계산
            clean_pos += rest[..pos].chars().count();
            rest = &rest[pos + HL_START.len()..];

            let start = clean_pos;

            // HL_END 찾기
            if let Some(end_pos) = rest.find(HL_END) {
                clean_pos += rest[..end_pos].chars().count();
                ranges.push((start, clean_pos));
                rest = &rest[end_pos + HL_END.len()..];
            } else {
                // HL_END 없으면 나머지 전체가 하이라이트
                clean_pos += rest.chars().count();
                ranges.push((start, clean_pos));
                break;
            }
        } else {
            // 더 이상 마커 없음
            break;
        }
    }

    ranges
}

/// FTS5 BM25 스코어를 confidence로 변환
///
/// min-max 정규화에 절대 스코어 기반 감쇠를 적용하여
/// 약한 매칭만 있는 결과 집합에서도 과대평가를 방지
fn normalize_fts_confidence(scores: &[f64]) -> Vec<u8> {
    if scores.is_empty() {
        return vec![];
    }

    // BM25 스코어는 음수 (더 음수 = 더 좋은 매칭)
    // 절대 스코어 기반 품질 감쇠: 최고 스코어의 절댓값이 낮으면 전체 감쇠
    let min = scores.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let best_abs = min.abs(); // min이 가장 음수 = 최고 매칭
    let quality_factor = (best_abs / 5.0).min(1.0); // abs >= 5.0이면 감쇠 없음

    if (max - min).abs() < f64::EPSILON {
        let confidence = (quality_factor * 100.0).round().clamp(0.0, 100.0) as u8;
        return vec![confidence; scores.len()];
    }

    scores
        .iter()
        .map(|&score| {
            let normalized = (max - score) / (max - min);
            (normalized * quality_factor * 100.0)
                .round()
                .clamp(0.0, 100.0) as u8
        })
        .collect()
}

/// 벡터 유사도 스코어를 confidence로 변환
fn normalize_vector_confidence(score: f64) -> u8 {
    (score * 100.0).round().clamp(0.0, 100.0) as u8
}

/// RRF 스코어를 confidence로 변환
fn normalize_rrf_confidence(score: f64, k: f64) -> u8 {
    let max_possible = 2.0 / (k + 1.0);
    let normalized = (score / max_possible).min(1.0);
    (normalized * 100.0).round().clamp(0.0, 100.0) as u8
}

/// 키워드 위치 기반 페이지 보간
/// snippet에서 첫 번째 하이라이트 키워드를 추출하고,
/// 전체 청크 텍스트 내 위치를 기반으로 page_start~page_end 사이를 보간
fn interpolate_page_from_snippet(
    page_start: Option<i64>,
    page_end: Option<i64>,
    chunk_content: &str,
    snippet: &str,
) -> Option<i64> {
    let ps = page_start?;
    let pe = page_end.unwrap_or(ps);

    // 같은 페이지면 보간 불필요
    if ps == pe {
        return Some(ps);
    }

    // snippet에서 첫 번째 [[HL]]...[[/HL]] 추출
    let hl_start = snippet.find("[[HL]]")?;
    let after_hl = &snippet[hl_start + 6..];
    let hl_end = after_hl.find("[[/HL]]")?;
    let keyword = &after_hl[..hl_end];

    if keyword.is_empty() {
        return Some(ps);
    }

    // 청크 텍스트에서 키워드 위치 찾기
    let keyword_pos = chunk_content.find(keyword)?;
    let chunk_len = chunk_content.len().max(1);

    // 비율 기반 보간
    let ratio = keyword_pos as f64 / chunk_len as f64;
    let page_span = (pe - ps) as f64;
    let interpolated = ps as f64 + ratio * page_span;

    Some(interpolated.round() as i64)
}

// ============================================
// Smart Search 후처리 필터
// ============================================

use crate::search::nl_query::DateFilter;

/// 날짜 필터 적용
fn smart_apply_date_filter(r: &SearchResult, filter: &Option<DateFilter>, _now: i64) -> bool {
    use chrono::{Datelike, Duration, FixedOffset};

    let Some(filter) = filter else { return true };
    let Some(modified) = r.modified_at else {
        return false;
    };

    // KST (UTC+9) 기준으로 날짜 계산 — 사용자 체감 시간과 일치
    let kst = FixedOffset::east_opt(9 * 3600).unwrap();
    let today = chrono::Utc::now().with_timezone(&kst).date_naive();

    let (start, end) = match filter {
        DateFilter::Today => {
            let s = today.and_hms_opt(0, 0, 0).unwrap();
            (kst_to_utc(&kst, s), i64::MAX)
        }
        DateFilter::ThisWeek => {
            // 이번 주 월요일 00:00 ~ now
            let days_since_mon = today.weekday().num_days_from_monday();
            let monday = today - Duration::days(days_since_mon as i64);
            let s = monday.and_hms_opt(0, 0, 0).unwrap();
            (kst_to_utc(&kst, s), i64::MAX)
        }
        DateFilter::LastWeek => {
            // 직전 주 월요일 00:00 ~ 일요일 23:59:59
            let days_since_mon = today.weekday().num_days_from_monday();
            let this_monday = today - Duration::days(days_since_mon as i64);
            let last_monday = this_monday - Duration::days(7);
            let last_sunday = this_monday - Duration::days(1);
            let s = last_monday.and_hms_opt(0, 0, 0).unwrap();
            let e = last_sunday.and_hms_opt(23, 59, 59).unwrap();
            (kst_to_utc(&kst, s), kst_to_utc(&kst, e))
        }
        DateFilter::ThisMonth => {
            let first = chrono::NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap();
            let s = first.and_hms_opt(0, 0, 0).unwrap();
            (kst_to_utc(&kst, s), i64::MAX)
        }
        DateFilter::LastMonth => {
            // 직전 달 1일 ~ 말일
            let first_this =
                chrono::NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap();
            let last_day_prev = first_this - Duration::days(1);
            let first_prev =
                chrono::NaiveDate::from_ymd_opt(last_day_prev.year(), last_day_prev.month(), 1)
                    .unwrap();
            let s = first_prev.and_hms_opt(0, 0, 0).unwrap();
            let e = last_day_prev.and_hms_opt(23, 59, 59).unwrap();
            (kst_to_utc(&kst, s), kst_to_utc(&kst, e))
        }
        DateFilter::ThisYear => {
            let year_start = chrono::NaiveDate::from_ymd_opt(today.year(), 1, 1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap();
            (kst_to_utc(&kst, year_start), i64::MAX)
        }
        DateFilter::LastYear => {
            let last_year = today.year() - 1;
            let s = chrono::NaiveDate::from_ymd_opt(last_year, 1, 1)
                .and_then(|d| d.and_hms_opt(0, 0, 0))
                .map(|dt| kst_to_utc(&kst, dt))
                .unwrap_or(0);
            let e = chrono::NaiveDate::from_ymd_opt(last_year, 12, 31)
                .and_then(|d| d.and_hms_opt(23, 59, 59))
                .map(|dt| kst_to_utc(&kst, dt))
                .unwrap_or(i64::MAX);
            (s, e)
        }
        DateFilter::Year(y) => {
            let s = chrono::NaiveDate::from_ymd_opt(*y, 1, 1)
                .and_then(|d| d.and_hms_opt(0, 0, 0))
                .map(|dt| kst_to_utc(&kst, dt))
                .unwrap_or(0);
            let e = chrono::NaiveDate::from_ymd_opt(*y, 12, 31)
                .and_then(|d| d.and_hms_opt(23, 59, 59))
                .map(|dt| kst_to_utc(&kst, dt))
                .unwrap_or(i64::MAX);
            (s, e)
        }
        DateFilter::Month(m) => {
            // 올해의 해당 월 (1일 ~ 말일)
            let year = today.year();
            let first = chrono::NaiveDate::from_ymd_opt(year, *m, 1);
            let last = if *m == 12 {
                chrono::NaiveDate::from_ymd_opt(year + 1, 1, 1).map(|d| d - Duration::days(1))
            } else {
                chrono::NaiveDate::from_ymd_opt(year, *m + 1, 1).map(|d| d - Duration::days(1))
            };
            match (first, last) {
                (Some(f), Some(l)) => {
                    let s = f.and_hms_opt(0, 0, 0).unwrap();
                    let e = l.and_hms_opt(23, 59, 59).unwrap();
                    (kst_to_utc(&kst, s), kst_to_utc(&kst, e))
                }
                _ => (0, i64::MAX),
            }
        }
        DateFilter::RecentDays(n) => {
            let past = today - Duration::days(*n as i64);
            let s = past.and_hms_opt(0, 0, 0).unwrap();
            (kst_to_utc(&kst, s), i64::MAX)
        }
    };

    modified >= start && modified <= end
}

/// NaiveDateTime(KST 해석) → UTC Unix timestamp
fn kst_to_utc(kst: &chrono::FixedOffset, dt: chrono::NaiveDateTime) -> i64 {
    use chrono::TimeZone;
    kst.from_local_datetime(&dt)
        .single()
        .map(|t: chrono::DateTime<chrono::FixedOffset>| t.timestamp())
        .unwrap_or(0)
}

/// "제N조" 패턴 카운트 (법령 분류용)
fn count_article_pattern(text: &str) -> usize {
    let mut count = 0;
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;
    while i < len {
        // "제" + 숫자 + "조" 패턴 찾기
        if chars[i] == '제' {
            let mut j = i + 1;
            // 공백 스킵
            while j < len && chars[j].is_whitespace() {
                j += 1;
            }
            // 숫자 확인
            let num_start = j;
            while j < len && chars[j].is_ascii_digit() {
                j += 1;
            }
            if j > num_start {
                // 공백 스킵
                while j < len && chars[j].is_whitespace() {
                    j += 1;
                }
                // "조" 확인
                if j < len && chars[j] == '조' {
                    count += 1;
                    i = j + 1;
                    continue;
                }
            }
        }
        i += 1;
    }
    count
}

/// 파일 타입 필터 적용
fn smart_apply_file_type_filter(r: &SearchResult, ft: &Option<String>) -> bool {
    let Some(ft) = ft else { return true };
    r.file_name.to_lowercase().ends_with(&format!(".{}", ft))
}

/// 제외 키워드 필터 적용
fn smart_apply_exclude_filter(r: &SearchResult, exclude: &[String]) -> bool {
    if exclude.is_empty() {
        return true;
    }
    let content = r.content_preview.to_lowercase();
    let snippet = r.snippet.as_deref().unwrap_or("").to_lowercase();
    !exclude.iter().any(|term| {
        let lower = term.to_lowercase();
        content.contains(&lower) || snippet.contains(&lower)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rerank_keeps_vector_only_results_in_their_original_tail_order() {
        let top_results = vec!["vector-only-a", "fts-b", "fts-c"];
        let reranked = apply_reranked_top_results(top_results, &[1, 2], &[1, 0]);

        assert_eq!(reranked, vec!["fts-c", "fts-b", "vector-only-a"]);
    }

    #[test]
    fn rerank_restores_missing_candidates_without_dropping_results() {
        let top_results = vec!["vector-a", "fts-b", "vector-c", "fts-d"];
        let reranked = apply_reranked_top_results(top_results, &[1, 3], &[1]);

        assert_eq!(reranked, vec!["fts-d", "fts-b", "vector-a", "vector-c"]);
    }

    // ==================== 문서 분류 테스트 ====================

    /// 테스트 시나리오 정의
    struct ClassifyTestCase {
        name: &'static str,
        text: &'static str,
        expected: &'static str,
    }

    fn test_scenarios() -> Vec<ClassifyTestCase> {
        vec![
            // --- 법령 (진짜) ---
            ClassifyTestCase {
                name: "민법 조문",
                text: "제750조(불법행위의 내용) 고의 또는 과실로 인한 위법행위로 타인에게 손해를 가한 자는 그 손해를 배상할 책임이 있다. 제751조(재산 이외의 손해의 배상) 타인의 신체, 자유 또는 명예를 해하거나 기타 정신상고통을 가한 자는 재산 이외의 손해에 대하여도 배상할 책임이 있다.",
                expected: "법령",
            },
            ClassifyTestCase {
                name: "근로기준법 시행령",
                text: "근로기준법 시행령 제1조(목적) 이 영은 「근로기준법」에서 위임된 사항과 그 시행에 필요한 사항을 규정함을 목적으로 한다. 제2조(통상임금) 법과 이 영에서 통상임금이란 근로자에게 정기적이고 일률적으로 소정근로 또는 총근로에 대하여 지급하기로 정한 시간급 금액을 말한다.",
                expected: "법령",
            },
            ClassifyTestCase {
                name: "광진구 조례",
                text: "서울특별시 광진구 공유재산 관리 조례 제1조(목적) 이 조례는 공유재산 및 물품 관리법에서 위임된 사항과 그 시행에 필요한 사항을 규정함을 목적으로 한다. 제2조(적용범위) 이 조례는 광진구가 소유하는 모든 공유재산에 적용한다.",
                expected: "법령",
            },
            // --- 법령 아닌데 오분류 위험 ---
            ClassifyTestCase {
                name: "고용산재보험료 엑셀 (오분류 케이스)",
                text: "58.00 청소과 김하늘 기간제근로자 청소과, 깨끗한도시만들기를위한청소행정서비스증대, 안정적생활폐기물처리로깨끗하고행복한도시실현, 쓰레기무단투기지역관리및예방, 인건비, 기간제근로자등보수 22150.00 20920.00 23260.00 59.00 청소과 남명호 기간제근로자 청소과 인건비 177200.00",
                expected: "기타",
            },
            ClassifyTestCase {
                name: "예산 집행현황 표",
                text: "2024년 상반기 예산 집행현황 부서 예산액 집행액 집행률 총무과 500,000 450,000 90.0% 재정과 800,000 720,000 90.0% 민원과 300,000 250,000 83.3% 합계 1,600,000 1,420,000 88.8%",
                expected: "기타",
            },
            ClassifyTestCase {
                name: "급여명세서",
                text: "2025년 3월 급여명세서 성명: 홍길동 부서: 총무과 기본급 3,200,000 직급보조비 200,000 가족수당 100,000 시간외근무수당 350,000 소득세 -180,000 국민연금 -144,000 건강보험 -115,200 실수령액 3,410,800",
                expected: "기타",
            },
            // --- 공문 ---
            ClassifyTestCase {
                name: "행정 공문",
                text: "수신 : 각 과장 (경유) 시행 2024-0301-001 제목: 2024년 상반기 업무보고 계획 안내 관인생략 1. 관련: 기획예산과-1234(2024.02.15.) 2. 위 호와 관련하여 2024년 상반기 업무보고 일정을 다음과 같이 안내합니다.",
                expected: "공문",
            },
            // --- 회의록 ---
            ClassifyTestCase {
                name: "부서 회의록",
                text: "회의록 회의일시: 2024년 3월 15일 14:00 회의 장소: 3층 대회의실 참석자: 과장 김철수, 팀장 이영희, 주무관 박민수 안건1: 상반기 사업계획 검토 결정사항: 3월 말까지 세부 일정 확정",
                expected: "회의록",
            },
            // --- 보고서 ---
            ClassifyTestCase {
                name: "업무 보고서",
                text: "2024년 1분기 민원처리 현황 보고서 Ⅰ. 서론 본 보고서는 2024년 1분기 민원처리 현황을 분석하고 개선방안을 제시하기 위해 작성되었습니다. Ⅱ. 현황 분석 총 민원접수 건수: 1,234건 Ⅲ. 결론 민원처리 기간 단축을 위한 업무 프로세스 개선이 필요합니다.",
                expected: "보고서",
            },
            // --- 계획서 ---
            ClassifyTestCase {
                name: "사업 계획서",
                text: "2024년 구정 주요 사업계획 추진계획 1. 추진배경 주민 생활편의 증진 및 지역경제 활성화를 위한 세부계획 수립 2. 추진일정 3월: 실행계획 확정 4월~6월: 1단계 시행 3. 소요예산 총 5억원",
                expected: "계획서",
            },
            // --- 기안문 ---
            ClassifyTestCase {
                name: "기안 문서",
                text: "기안일자: 2024.03.15 기안자: 주무관 박민수 검토자: 팀장 이영희 협조자: 회계팀장 결재일자: 2024.03.16 제목: 사무용품 구매 요청의 건",
                expected: "기안문",
            },
            // --- 통계 ---
            ClassifyTestCase {
                name: "인구 통계",
                text: "2024년 광진구 인구통계 현황 전년대비 0.3% 감소 증감률 추이 2022년: +0.1% 2023년: -0.2% 2024년: -0.3% 전년동기 대비 세대수 증감 1인가구 비율 42.3%",
                expected: "통계",
            },
        ]
    }

    #[test]
    fn classify_by_keyword_test_scenarios() {
        let scenarios = test_scenarios();
        let mut pass = 0;
        let mut fail = 0;

        println!("\n===== 키워드 패턴 매칭 분류 테스트 =====");
        for tc in &scenarios {
            let result = SearchService::classify_by_keyword(tc.text);
            let ok = result == tc.expected;
            if ok {
                pass += 1;
            } else {
                fail += 1;
            }
            println!(
                "  {} {}: 예상={}, 결과={} {}",
                if ok { "✅" } else { "❌" },
                tc.name,
                tc.expected,
                result,
                if !ok { "<-- MISMATCH" } else { "" }
            );
        }
        println!("  합계: {pass}/{} 통과 (실패 {fail})", pass + fail);
        assert_eq!(fail, 0, "키워드 분류 실패 {fail}건");
    }

    #[test]
    fn count_article_pattern_works() {
        assert_eq!(count_article_pattern("제1조 제2조 제10조"), 3);
        assert_eq!(count_article_pattern("제 750 조 내용"), 1);
        assert_eq!(count_article_pattern("제출 기한 제한"), 0);
        assert_eq!(count_article_pattern("보험료 22150.00"), 0);
    }
}
