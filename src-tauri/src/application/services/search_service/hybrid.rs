//! 하이브리드 검색 (FTS + 벡터 + RRF + Reranking)

use super::helpers::*;
use super::SearchService;
use crate::application::dto::search::{MatchType, SearchResponse, SearchResult};
use crate::application::errors::{AppError, AppResult};
use crate::db;
use crate::search::{fts, hybrid, KeywordMode};
use std::collections::{HashMap, HashSet};
use std::time::Instant;

impl SearchService {
    /// 하이브리드 검색 (FTS + 벡터 + RRF + Reranking)
    pub async fn search_hybrid(
        &self,
        query: &str,
        max_results: usize,
        folder_scope: Option<&str>,
    ) -> AppResult<SearchResponse> {
        self.search_hybrid_with_mode(query, max_results, folder_scope, KeywordMode::And)
            .await
    }

    /// 하이브리드 검색 — 검색 모드 지정
    pub async fn search_hybrid_with_mode(
        &self,
        query: &str,
        max_results: usize,
        folder_scope: Option<&str>,
        mode: KeywordMode,
    ) -> AppResult<SearchResponse> {
        let start = Instant::now();
        let use_tokenizer = self.tokenizer.is_some();

        let conn = self.get_connection()?;

        // 1. FTS5 검색 (mode 적용)
        let fts_results = match self.tokenizer.as_ref() {
            Some(tok) => fts::search_with_tokenizer(
                &conn,
                query,
                max_results,
                tok.as_ref(),
                folder_scope,
                mode,
            )
            .map_err(|e| AppError::SearchFailed(e.to_string()))?,
            None => fts::search(&conn, query, max_results, folder_scope, mode)
                .map_err(|e| AppError::SearchFailed(e.to_string()))?,
        };

        // 2. 벡터 검색
        let vector_fetch_limit = if folder_scope.is_some() {
            max_results * 3
        } else {
            max_results
        };
        let (vector_results, query_embedding) =
            match (self.embedder.as_ref(), self.vector_index.as_ref()) {
                (Some(emb), Some(vi)) => match emb.embed(query, true) {
                    Ok(qe) => {
                        let raw_results = vi.search(&qe, vector_fetch_limit).unwrap_or_default();
                        let results = if folder_scope.is_some() && !raw_results.is_empty() {
                            let ids: Vec<i64> = raw_results.iter().map(|r| r.chunk_id).collect();
                            let path_map =
                                db::get_chunk_file_paths(&conn, &ids).unwrap_or_default();
                            raw_results
                                .into_iter()
                                .filter(|r| {
                                    path_map
                                        .get(&r.chunk_id)
                                        .map(|p| matches_folder_scope(p, folder_scope))
                                        .unwrap_or(false)
                                })
                                .collect()
                        } else {
                            raw_results
                        };
                        (results, Some(qe))
                    }
                    Err(e) => {
                        tracing::warn!("Failed to embed query: {}", e);
                        (vec![], None)
                    }
                },
                _ => (vec![], None),
            };

        // 3. FTS → HashMap
        let fts_map: HashMap<i64, &fts::FtsResult> =
            fts_results.iter().map(|r| (r.chunk_id, r)).collect();
        let vector_chunk_ids: HashSet<i64> = vector_results.iter().map(|r| r.chunk_id).collect();

        // 4. RRF 병합 (k=60: 학술 표준값)
        const RRF_K: f32 = 60.0;
        let hybrid_results = hybrid::merge_results(&fts_results, &vector_results, RRF_K);

        // 5. 벡터 전용 결과 DB 조회
        // 벡터 전용 결과는 유사도 임계값 적용 (FTS 미매칭 = 키워드 관련성 없음)
        // 벡터 유사도 0.5 미만이면 무관한 결과로 판단하여 제외
        const VECTOR_ONLY_MIN_SCORE: f32 = 0.5;
        let vector_score_map: HashMap<i64, f32> = vector_results
            .iter()
            .map(|r| (r.chunk_id, r.score))
            .collect();
        let vector_only_ids: Vec<i64> = hybrid_results
            .iter()
            .filter(|r| !fts_map.contains_key(&r.chunk_id))
            .filter(|r| {
                vector_score_map.get(&r.chunk_id).copied().unwrap_or(0.0) >= VECTOR_ONLY_MIN_SCORE
            })
            .map(|r| r.chunk_id)
            .collect();

        let vector_only_chunks: HashMap<i64, db::ChunkInfo> = if !vector_only_ids.is_empty() {
            db::get_chunks_by_ids(&conn, &vector_only_ids)
                .map_err(|e| AppError::SearchFailed(e.to_string()))?
                .into_iter()
                .map(|c| (c.chunk_id, c))
                .collect()
        } else {
            HashMap::new()
        };

        // 결과 변환
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
                        total_chunks: 0,
                        lineage_id: None,
                        lineage_role: None,
                        version_label: None,
                        version_count: 0,
                    })
                } else {
                    vector_only_chunks.get(&hr.chunk_id).and_then(|chunk| {
                        if !matches_folder_scope(&chunk.file_path, folder_scope) {
                            return None;
                        }
                        Some(SearchResult {
                            file_path: chunk.file_path.clone(),
                            file_name: chunk.file_name.clone(),
                            chunk_index: chunk.chunk_index,
                            content_preview: truncate_preview(&chunk.content, 200),
                            // RAG 경로가 full_content 를 그대로 LLM 컨텍스트로 보내기 때문에
                            // vector-only 히트도 원문을 채워야 한다. 비워두면 200자 preview 로
                            // 폴백되어 의미 검색이 찾아낸 핵심 증거가 잘린 채 전달된다.
                            full_content: chunk.content.clone(),
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
                            total_chunks: 0,
                            lineage_id: None,
                            lineage_role: None,
                            version_label: None,
                            version_count: 0,
                        })
                    })
                }
            })
            .collect();

        // 파일별 중복 제거 (최대 3개 청크)
        {
            const MAX_CHUNKS_PER_FILE: usize = 3;
            let mut file_counts: HashMap<String, usize> = HashMap::new();
            results.retain(|r| {
                let count = file_counts.entry(r.file_path.clone()).or_insert(0);
                *count += 1;
                *count <= MAX_CHUNKS_PER_FILE
            });
        }

        // 시맨틱 enrichment
        if let Some(qe) = query_embedding.as_ref() {
            if let Err(e) = self.enrich_semantic_results(&mut results, qe) {
                tracing::warn!("Hybrid semantic enrichment failed: {}", e);
            }
        }

        enrich_total_chunks(&conn, &mut results);
        enrich_lineage_info(&conn, &mut results);
        let total_count = results.len();
        let search_time_ms = start.elapsed().as_millis() as u64;

        tracing::debug!(
            "Hybrid search '{}': {} results in {}ms (tokenizer={})",
            query,
            total_count,
            search_time_ms,
            use_tokenizer
        );

        Ok(SearchResponse {
            results,
            total_count,
            search_time_ms,
            search_mode: "hybrid".to_string(),
        })
    }

    /// 단일 파일 내부 하이브리드 검색.
    ///
    /// 전역 top-N 에서 파일 필터를 거는 방식은 큰 문서의 관련 청크가 전역 랭킹 밖으로
    /// 밀려날 때 파일 QA 품질을 떨어뜨린다. 이 메서드는 처음부터 `f.path = ?` 로
    /// FTS 를 좁혀 파일 내부에서 BM25 상위 결과만 반환한다.
    ///
    /// 벡터 검색은 단일 파일 문맥에서는 파일 내 모든 청크가 이미 주제적으로 관련되어
    /// 있고, chunk_index 기반 순차 보충이 맥락 연속성을 보장하므로 여기선 사용하지
    /// 않는다 (vector_index 는 전역 top-N 만 반환하여 file-scoped recall 을 보장하지 못함).
    pub async fn search_hybrid_in_file(
        &self,
        query: &str,
        max_results: usize,
        file_path: &str,
    ) -> AppResult<SearchResponse> {
        let start = Instant::now();
        let conn = self.get_connection()?;

        let tok_ref = self.tokenizer.as_ref().map(|a| a.as_ref());
        let fts_results = fts::search_in_file(
            &conn,
            query,
            max_results,
            file_path,
            tok_ref,
            KeywordMode::And,
        )
        .map_err(|e| AppError::SearchFailed(e.to_string()))?;

        let mut results: Vec<SearchResult> = fts_results
            .iter()
            .map(|fts_r| {
                let page_number = interpolate_page_from_snippet(
                    fts_r.page_number,
                    fts_r.page_end,
                    &fts_r.content,
                    &fts_r.snippet,
                );
                let improved = ensure_keyword_in_snippet(&fts_r.snippet, &fts_r.content, query);
                let content_preview = strip_highlight_markers(&improved);
                let highlight_ranges = parse_highlight_ranges(&improved);

                SearchResult {
                    file_path: fts_r.file_path.clone(),
                    file_name: fts_r.file_name.clone(),
                    chunk_index: fts_r.chunk_index,
                    content_preview,
                    full_content: fts_r.content.clone(),
                    score: fts_r.score,
                    confidence: 0,
                    match_type: MatchType::Keyword,
                    highlight_ranges,
                    page_number,
                    start_offset: fts_r.start_offset,
                    location_hint: fts_r.location_hint.clone(),
                    snippet: Some(improved),
                    modified_at: fts_r.modified_at,
                    has_hwp_pair: false,
                    total_chunks: 0,
                    lineage_id: None,
                    lineage_role: None,
                    version_label: None,
                    version_count: 0,
                }
            })
            .collect();

        enrich_total_chunks(&conn, &mut results);
        let total_count = results.len();
        let search_time_ms = start.elapsed().as_millis() as u64;

        Ok(SearchResponse {
            results,
            total_count,
            search_time_ms,
            search_mode: "hybrid_in_file".to_string(),
        })
    }
}
