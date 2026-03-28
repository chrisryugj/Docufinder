//! 키워드 검색 (FTS5) + 파일명 검색

use super::helpers::*;
use super::SearchService;
use crate::application::dto::search::{MatchType, SearchResponse, SearchResult};
use crate::application::errors::{AppError, AppResult};
use crate::search::{filename, fts};
use std::collections::HashSet;
use std::path::Path;
use std::time::Instant;

impl SearchService {
    /// 키워드 검색 (FTS5)
    pub async fn search_keyword(
        &self,
        query: &str,
        max_results: usize,
        folder_scope: Option<&str>,
    ) -> AppResult<SearchResponse> {
        let start = Instant::now();
        let conn = self.get_connection()?;
        let use_tokenizer = self.tokenizer.is_some();

        let fts_results = match self.tokenizer.as_ref() {
            Some(tok) => {
                fts::search_with_tokenizer(&conn, query, max_results, tok.as_ref(), folder_scope)
                    .map_err(|e| AppError::SearchFailed(e.to_string()))?
            }
            None => fts::search(&conn, query, max_results, folder_scope)
                .map_err(|e| AppError::SearchFailed(e.to_string()))?,
        };

        let scores: Vec<f64> = fts_results.iter().map(|r| r.score).collect();
        let confidences = normalize_fts_confidence(&scores);

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
            query, total_count, search_time_ms, use_tokenizer
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

        let use_cache = self
            .filename_cache
            .as_ref()
            .is_some_and(|c| !c.is_empty() && !c.is_truncated());

        let results: Vec<SearchResult> = if use_cache {
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
            cache
                .search_with_scope(query, max_results, folder_scope)
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

        let results = Self::dedup_hwp_hwpx(results);
        let total_count = results.len();
        let search_time_ms = start.elapsed().as_millis() as u64;

        tracing::debug!(
            "Filename search '{}': {} results in {}ms (cache={})",
            query, total_count, search_time_ms, use_cache
        );

        Ok(SearchResponse {
            results,
            total_count,
            search_time_ms,
            search_mode: "filename".to_string(),
        })
    }

    /// HWP/HWPX 중복 제거
    pub(super) fn dedup_hwp_hwpx(mut results: Vec<SearchResult>) -> Vec<SearchResult> {
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
