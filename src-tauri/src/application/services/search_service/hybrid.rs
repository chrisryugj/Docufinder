//! 하이브리드 검색 (FTS5 키워드 + usearch 벡터 + RRF 병합)

use super::helpers::*;
use super::SearchService;
use crate::application::dto::search::{MatchType, SearchResponse, SearchResult};
use crate::application::errors::{AppError, AppResult};
use crate::db;
use crate::search::{fts, hybrid, query_syntax, KeywordMode};
use std::collections::{HashMap, HashSet};
use std::time::Instant;

/// 벡터 전용 히트는 SQL 메타 필터를 거치지 않으므로 연산자 조건을 후처리로 적용
fn vector_chunk_passes_operators(
    chunk: &db::ChunkInfo,
    op: &query_syntax::OperatorQuery,
) -> bool {
    if !op.ext_filters.is_empty() {
        let name = chunk.file_name.to_lowercase();
        if !op
            .ext_filters
            .iter()
            .any(|e| name.ends_with(&format!(".{}", e)))
        {
            return false;
        }
    }
    if !op.path_filters.is_empty() {
        let path = chunk.file_path.to_lowercase().replace('\\', "/");
        if !op.path_filters.iter().all(|p| path.contains(p.as_str())) {
            return false;
        }
    }
    if let Some((start, end)) = op.date_range() {
        match chunk.modified_at {
            Some(ts) if ts >= start && ts <= end => {}
            _ => return false,
        }
    }
    if !op.excludes.is_empty() {
        let content = crate::utils::normalize_text(&chunk.content).to_lowercase();
        let excluded = op.excludes.iter().any(|e| {
            let needle = crate::utils::normalize_text(e).to_lowercase();
            !needle.is_empty() && content.contains(&needle)
        });
        if excluded {
            return false;
        }
    }
    true
}

impl SearchService {
    /// 하이브리드 검색 (FTS5 키워드 + usearch 벡터 + RRF 병합)
    pub async fn search_hybrid(
        &self,
        query: &str,
        max_results: usize,
        folder_scope: Option<&str>,
    ) -> AppResult<SearchResponse> {
        self.search_hybrid_with_mode(
            query,
            max_results,
            folder_scope,
            KeywordMode::And,
            &fts::MetaFilter::default(),
        )
        .await
    }

    /// 하이브리드 검색 — 검색 모드 + 메타 필터 지정
    ///
    /// 메타 필터(날짜·파일타입)는 FTS 단계에 SQL 로 적용해 키워드+필터 질의가
    /// BM25 상위 N 밖으로 밀려 누락되는 것을 막는다. 벡터 검색은 메타 후처리를
    /// `search_smart` 의 최종 필터에 위임한다(FTS 가 타깃을 이미 보장).
    pub async fn search_hybrid_with_mode(
        &self,
        query: &str,
        max_results: usize,
        folder_scope: Option<&str>,
        mode: KeywordMode,
        filter: &fts::MetaFilter,
    ) -> AppResult<SearchResponse> {
        self.search_hybrid_impl(query, None, max_results, folder_scope, mode, filter)
            .await
    }

    /// 하이브리드 검색 — 인라인 연산자 지원 (v3.0.0)
    ///
    /// FTS 측은 연산자 합성 MATCH + SQL 메타 필터, 벡터 측은 연산자를 제거한
    /// 텍스트로 임베딩하고 메타/제외 조건을 후처리로 적용한다.
    /// 연산자가 없으면 기존 경로와 완전 동일.
    pub async fn search_hybrid_with_operators(
        &self,
        query: &str,
        max_results: usize,
        folder_scope: Option<&str>,
        mode: KeywordMode,
    ) -> AppResult<SearchResponse> {
        let op = query_syntax::parse_operators(query);
        if !op.has_operators() {
            return self
                .search_hybrid_impl(
                    query,
                    None,
                    max_results,
                    folder_scope,
                    mode,
                    &fts::MetaFilter::default(),
                )
                .await;
        }

        let filter = fts::MetaFilter {
            date_range: op.date_range(),
            file_types: op.ext_filters.clone(),
            path_contains: op.path_filters.clone(),
        };
        self.search_hybrid_impl(query, Some(&op), max_results, folder_scope, mode, &filter)
            .await
    }

    async fn search_hybrid_impl(
        &self,
        query: &str,
        op: Option<&query_syntax::OperatorQuery>,
        max_results: usize,
        folder_scope: Option<&str>,
        mode: KeywordMode,
        filter: &fts::MetaFilter,
    ) -> AppResult<SearchResponse> {
        let start = Instant::now();
        let use_tokenizer = self.tokenizer.is_some();

        let conn = self.get_connection()?;

        // 벡터 임베딩·스니펫 보정용 텍스트 — 연산자 사용 시 연산자 제거본
        let display_query = match op {
            Some(o) => o.semantic_text(),
            None => query.to_string(),
        };

        // 1. FTS5 검색 (mode + 메타 필터 적용 / 연산자 합성)
        let fts_results = match op {
            Some(o) => {
                let tok_ref = self.tokenizer.as_ref().map(|a| a.as_ref());
                fts::search_with_operators(
                    &conn,
                    o,
                    max_results,
                    tok_ref,
                    folder_scope,
                    mode,
                    filter,
                )
                .map_err(|e| AppError::SearchFailed(e.to_string()))?
            }
            None => match self.tokenizer.as_ref() {
                Some(tok) => fts::search_with_tokenizer(
                    &conn,
                    query,
                    max_results,
                    tok.as_ref(),
                    folder_scope,
                    mode,
                    filter,
                )
                .map_err(|e| AppError::SearchFailed(e.to_string()))?,
                None => fts::search(&conn, query, max_results, folder_scope, mode, filter)
                    .map_err(|e| AppError::SearchFailed(e.to_string()))?,
            },
        };

        // 2. 벡터 검색
        let vector_fetch_limit = if folder_scope.is_some() {
            max_results * 3
        } else {
            max_results
        };
        let (vector_results, query_embedding) =
            match (self.embedder.as_ref(), self.vector_index.as_ref()) {
                // 필터 전용 질의(검색어 없음)는 임베딩할 텍스트가 없으므로 벡터 생략
                (Some(_), Some(_)) if display_query.trim().is_empty() => (vec![], None),
                (Some(emb), Some(vi)) => match emb.embed(&display_query, true) {
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
                    let improved =
                        ensure_keyword_in_snippet(&fts_r.snippet, &fts_r.content, &display_query);
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
                        // 연산자 검색: 벡터 전용 히트에도 ext/path/날짜/제외 조건 적용
                        if let Some(o) = op {
                            if !vector_chunk_passes_operators(chunk, o) {
                                return None;
                            }
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
