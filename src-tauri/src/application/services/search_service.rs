//! SearchService - 검색 비즈니스 로직
//!
//! 다양한 검색 모드 (keyword, semantic, hybrid, filename)를 처리하고
//! 결과를 정규화된 DTO로 반환합니다.

use crate::application::dto::search::{
    MatchType, SearchMode, SearchQuery, SearchResponse, SearchResult,
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
/// 0 = 비활성화 (i3에서 500~1000ms 추가 지연 방지, content_preview로 충분)
const SEMANTIC_ENRICH_MAX_RESULTS: usize = 0;

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
            SearchMode::Keyword => self.search_keyword(&query.query, query.max_results).await,
            SearchMode::Semantic => self.search_semantic(&query.query, query.max_results).await,
            SearchMode::Hybrid => self.search_hybrid(&query.query, query.max_results).await,
            SearchMode::Filename => self.search_filename(&query.query, query.max_results).await,
        }
    }

    /// 키워드 검색 (FTS5)
    pub async fn search_keyword(
        &self,
        query: &str,
        max_results: usize,
    ) -> AppResult<SearchResponse> {
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
            let cache_results = cache.search(query, max_results);

            cache_results
                .into_iter()
                .map(|r| SearchResult {
                    file_path: r.path,
                    file_name: r.name.clone(),
                    chunk_index: 0,
                    content_preview: r.name.clone(),
                    full_content: String::new(),
                    score: 1.0,
                    confidence: 100,
                    match_type: MatchType::Filename,
                    highlight_ranges: vec![],
                    page_number: None,
                    start_offset: 0,
                    location_hint: Some(r.file_type),
                    snippet: Some(r.name),
                    modified_at: Some(r.modified_at),
                })
                .collect()
        } else {
            // Fallback: DB LIKE 검색
            let conn = self.get_connection()?;
            let filename_results = filename::search(&conn, query, max_results)
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
                })
                .collect()
        };

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

        // 벡터 검색
        let vector_results = vector_index
            .search(&query_embedding, max_results)
            .map_err(|e| AppError::SearchFailed(e.to_string()))?;

        // chunk_id로 파일 정보 조회
        let conn = self.get_connection()?;
        let chunk_ids: Vec<i64> = vector_results.iter().map(|r| r.chunk_id).collect();
        let chunks = db::get_chunks_by_ids(&conn, &chunk_ids)
            .map_err(|e| AppError::SearchFailed(e.to_string()))?;

        let chunk_map: HashMap<i64, ChunkInfo> =
            chunks.into_iter().map(|c| (c.chunk_id, c)).collect();

        // 결과 변환 (⚡ full_content 제거)
        let mut results: Vec<SearchResult> = vector_results
            .into_iter()
            .filter_map(|vr| {
                chunk_map.get(&vr.chunk_id).map(|chunk| SearchResult {
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
    ) -> AppResult<SearchResponse> {
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
        let (vector_results, query_embedding) =
            match (self.embedder.as_ref(), self.vector_index.as_ref()) {
                (Some(emb), Some(vi)) => match emb.embed(query, true) {
                    Ok(qe) => {
                        let results = vi.search(&qe, max_results).unwrap_or_default();
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
        const RRF_K: f32 = 60.0;
        let mut hybrid_results = hybrid::merge_results(&fts_results, &vector_results, RRF_K);

        // 5. Cross-Encoder Reranking (상위 20개만)
        // FTS 결과에서 직접 content 조회 (DB 조회 제거)
        const RERANK_TOP_K: usize = 20;
        if let Some(rr) = self.reranker.as_ref() {
            if hybrid_results.len() > 1 {
                // Reranking 대상 문서 추출 (FTS 결과에서 직접)
                let documents: Vec<&str> = hybrid_results
                    .iter()
                    .take(RERANK_TOP_K)
                    .filter_map(|r| fts_map.get(&r.chunk_id).map(|f| f.content.as_str()))
                    .collect();

                if !documents.is_empty() {
                    match rr.rerank(query, &documents, documents.len()) {
                        Ok(reranked_indices) => {
                            // 상위 K개만 재정렬
                            let top_results: Vec<_> = hybrid_results
                                .drain(..documents.len().min(RERANK_TOP_K))
                                .collect();
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

        // 6. 벡터 전용 결과만 DB 조회 (FTS에 없는 것만)
        let vector_only_ids: Vec<i64> = hybrid_results
            .iter()
            .filter(|r| !fts_map.contains_key(&r.chunk_id))
            .map(|r| r.chunk_id)
            .collect();

        let vector_only_chunks: HashMap<i64, ChunkInfo> = if !vector_only_ids.is_empty() {
            db::get_chunks_by_ids(&conn, &vector_only_ids)
                .map_err(|e| AppError::SearchFailed(e.to_string()))?
                .into_iter()
                .map(|c| (c.chunk_id, c))
                .collect()
        } else {
            HashMap::new()
        };

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
                    })
                } else {
                    vector_only_chunks.get(&hr.chunk_id).map(|chunk| {
                        // 벡터 전용 결과 (DB 조회 결과 사용, ⚡ full_content 제거)
                        // snippet: None → enrich_semantic_results에서 문장 하이라이트 추가
                        SearchResult {
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
                        }
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
