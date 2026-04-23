//! 시맨틱 검색 (벡터) + 유사 문서 + Enrichment

use super::helpers::*;
use super::SearchService;
use crate::application::dto::search::{MatchType, SearchResponse, SearchResult};
use crate::application::errors::{AppError, AppResult};
use crate::db::{self, ChunkInfo};
use crate::search::sentence;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

/// snippet이 없는 벡터 전용 결과에 가장 유사한 문장 하이라이트 추가
const SEMANTIC_ENRICH_MAX_RESULTS: usize = 5;

impl SearchService {
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

        if vector_index.size() == 0 {
            return Err(AppError::VectorIndexEmpty);
        }

        let query_embedding = embedder
            .embed(query, true)
            .map_err(|e| AppError::EmbeddingFailed(e.to_string()))?;

        let vector_fetch_limit = if folder_scope.is_some() {
            max_results * 3
        } else {
            max_results
        };
        let vector_results = vector_index
            .search(&query_embedding, vector_fetch_limit)
            .map_err(|e| AppError::SearchFailed(e.to_string()))?;

        let conn = self.get_connection()?;

        // folder_scope 프리필터
        let filtered_results = if folder_scope.is_some() {
            let all_ids: Vec<i64> = vector_results.iter().map(|r| r.chunk_id).collect();
            let path_map = db::get_chunk_file_paths(&conn, &all_ids)
                .map_err(|e| AppError::SearchFailed(e.to_string()))?;
            vector_results
                .into_iter()
                .filter(|vr| {
                    path_map
                        .get(&vr.chunk_id)
                        .map(|p| matches_folder_scope(p, folder_scope))
                        .unwrap_or(false)
                })
                .collect::<Vec<_>>()
        } else {
            vector_results
        };

        let chunk_ids: Vec<i64> = filtered_results.iter().map(|r| r.chunk_id).collect();
        let chunks = db::get_chunks_by_ids(&conn, &chunk_ids)
            .map_err(|e| AppError::SearchFailed(e.to_string()))?;

        let chunk_map: HashMap<i64, ChunkInfo> =
            chunks.into_iter().map(|c| (c.chunk_id, c)).collect();

        let mut results: Vec<SearchResult> = filtered_results
            .into_iter()
            .filter_map(|vr| {
                chunk_map.get(&vr.chunk_id).map(|chunk| SearchResult {
                    file_path: chunk.file_path.clone(),
                    file_name: chunk.file_name.clone(),
                    chunk_index: chunk.chunk_index,
                    content_preview: truncate_preview(&chunk.content, 200),
                    full_content: String::new(),
                    score: vr.score as f64,
                    confidence: normalize_vector_confidence(vr.score as f64),
                    match_type: MatchType::Semantic,
                    highlight_ranges: vec![],
                    page_number: chunk.page_number,
                    start_offset: chunk.start_offset,
                    location_hint: chunk.location_hint.clone(),
                    snippet: Some(truncate_preview(&chunk.content, 200)),
                    modified_at: chunk.modified_at,
                    has_hwp_pair: false,
                    total_chunks: 0,
                    lineage_id: None,
                    lineage_role: None,
                    version_label: None,
                    version_count: 0,
                })
            })
            .collect();

        if let Err(e) = self.enrich_semantic_results(&mut results, &query_embedding) {
            tracing::warn!("Semantic enrichment failed: {}", e);
        }

        enrich_total_chunks(&conn, &mut results);
        enrich_lineage_info(&conn, &mut results);
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

    /// 시맨틱 검색 결과에 가장 유사한 문장 추가
    pub(super) fn enrich_semantic_results(
        &self,
        results: &mut [SearchResult],
        query_embedding: &[f32],
    ) -> AppResult<()> {
        let embedder = match self.embedder.as_ref() {
            Some(e) => e,
            None => return Ok(()),
        };

        let results_to_process = results.len().min(SEMANTIC_ENRICH_MAX_RESULTS);

        let mut all_sentences: Vec<(usize, String, usize, usize)> = Vec::new();

        for (idx, result) in results.iter().take(results_to_process).enumerate() {
            if result
                .snippet
                .as_ref()
                .is_some_and(|s| s.contains("[[HL]]"))
            {
                continue;
            }

            let source = if !result.full_content.is_empty() {
                &result.full_content
            } else {
                &result.content_preview
            };
            let sentences = sentence::split_sentences(source);
            for sent in sentences {
                all_sentences.push((idx, sent.text, sent.char_start, sent.char_end));
            }
        }

        if all_sentences.is_empty() {
            return Ok(());
        }

        let texts: Vec<String> = all_sentences.iter().map(|(_, t, _, _)| t.clone()).collect();
        let embeddings = match embedder.embed_batch(&texts) {
            Ok(emb) => emb,
            Err(e) => {
                tracing::warn!("Semantic enrichment embedding failed: {}", e);
                return Ok(());
            }
        };

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

        let enriched_count = best_per_result.len();
        for (idx, (sentence_text, _sim, start, end)) in best_per_result {
            if let Some(result) = results.get_mut(idx) {
                result.snippet = Some(format!("[[HL]]{}[[/HL]]", sentence_text));
                result.highlight_ranges = vec![(start, end)];
            }
        }

        tracing::debug!(
            "Enriched {} semantic results with best sentences",
            enriched_count
        );

        Ok(())
    }

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

        let source_chunk_ids = db::get_chunk_ids_for_path(&conn, file_path)
            .map_err(|e| AppError::SearchFailed(e.to_string()))?;

        if source_chunk_ids.is_empty() {
            return Ok(SearchResponse::empty("similar"));
        }

        let source_chunks = db::get_chunks_by_ids(&conn, &source_chunk_ids)
            .map_err(|e| AppError::SearchFailed(e.to_string()))?;

        let texts: Vec<String> = source_chunks
            .iter()
            .take(10)
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
        let norm: f32 = avg_embedding.iter().map(|v| v * v).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in &mut avg_embedding {
                *v /= norm;
            }
        }

        let vector_results = vector_index
            .search(&avg_embedding, max_results * 5)
            .map_err(|e| AppError::SearchFailed(e.to_string()))?;

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

        let mut file_best: HashMap<String, (f64, SearchResult)> = HashMap::new();
        for vr in &vector_results {
            if source_ids.contains(&vr.chunk_id) {
                continue;
            }
            if let Some(chunk) = chunk_map.get(&vr.chunk_id) {
                let score = vr.score as f64;
                file_best
                    .entry(chunk.file_path.clone())
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
                                total_chunks: 0,
                                lineage_id: None,
                                lineage_role: None,
                                version_label: None,
                                version_count: 0,
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
                                total_chunks: 0,
                                lineage_id: None,
                                lineage_role: None,
                                version_label: None,
                                version_count: 0,
                            },
                        )
                    });
            }
        }

        let mut results: Vec<SearchResult> = file_best.into_values().map(|(_, r)| r).collect();
        results.sort_by(|a, b| b.score.total_cmp(&a.score));
        results.truncate(max_results);

        enrich_total_chunks(&conn, &mut results);
        enrich_lineage_info(&conn, &mut results);
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
}
