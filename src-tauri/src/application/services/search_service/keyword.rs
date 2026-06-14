//! 키워드 검색 (FTS5) + 파일명 검색

use super::helpers::*;
use super::SearchService;
use crate::application::dto::search::{MatchType, SearchResponse, SearchResult};
use crate::application::errors::{AppError, AppResult};
use crate::search::{filename, fts, query_syntax, KeywordMode};
use std::collections::HashSet;
use std::path::Path;
use std::time::Instant;

impl SearchService {
    /// 키워드 검색 (FTS5) — 검색 모드 + 메타 필터 지정
    pub async fn search_keyword_with_mode(
        &self,
        query: &str,
        max_results: usize,
        folder_scope: Option<&str>,
        mode: KeywordMode,
        filter: &fts::MetaFilter,
    ) -> AppResult<SearchResponse> {
        let start = Instant::now();
        let conn = self.get_connection()?;
        let use_tokenizer = self.tokenizer.is_some();

        let fts_results = match self.tokenizer.as_ref() {
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
        };

        Ok(Self::keyword_response(
            &conn,
            fts_results,
            query,
            start,
            use_tokenizer,
        ))
    }

    /// 키워드 검색 — 인라인 연산자 지원 (v3.0.0)
    ///
    /// `"구문"` / `-제외` / `ext:` / `path:` / `before:`·`after:` / `~N` 을 파싱해
    /// FTS MATCH 식과 메타 필터로 합성한다. 연산자가 없으면 기존 경로와 완전 동일.
    pub async fn search_keyword_with_operators(
        &self,
        query: &str,
        max_results: usize,
        folder_scope: Option<&str>,
        mode: KeywordMode,
    ) -> AppResult<SearchResponse> {
        let op = query_syntax::parse_operators(query);
        if !op.has_operators() {
            return self
                .search_keyword_with_mode(
                    query,
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

        let start = Instant::now();
        let conn = self.get_connection()?;
        let use_tokenizer = self.tokenizer.is_some();
        let tok_ref = self.tokenizer.as_ref().map(|a| a.as_ref());

        let fts_results = fts::search_with_operators(
            &conn,
            &op,
            max_results,
            tok_ref,
            folder_scope,
            mode,
            &filter,
        )
        .map_err(|e| AppError::SearchFailed(e.to_string()))?;

        // 스니펫 키워드 보정은 연산자를 제거한 표시용 검색어 기준
        let display_query = op.semantic_text();
        Ok(Self::keyword_response(
            &conn,
            fts_results,
            &display_query,
            start,
            use_tokenizer,
        ))
    }

    /// FTS 결과 → 키워드 SearchResponse 변환 (스니펫 보정 + 신뢰도 + enrichment)
    fn keyword_response(
        conn: &rusqlite::Connection,
        fts_results: Vec<fts::FtsResult>,
        display_query: &str,
        start: Instant,
        use_tokenizer: bool,
    ) -> SearchResponse {
        let scores: Vec<f64> = fts_results.iter().map(|r| r.score).collect();
        let confidences = normalize_fts_confidence(&scores);

        let mut results: Vec<SearchResult> = fts_results
            .into_iter()
            .enumerate()
            .map(|(idx, r)| {
                let page_number = interpolate_page_from_snippet(
                    r.page_number,
                    r.page_end,
                    &r.content,
                    &r.snippet,
                );
                let improved = ensure_keyword_in_snippet(&r.snippet, &r.content, display_query);
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
                    total_chunks: 0,
                    lineage_id: None,
                    lineage_role: None,
                    version_label: None,
                    version_count: 0,
                }
            })
            .collect();

        enrich_total_chunks(conn, &mut results);
        enrich_lineage_info(conn, &mut results);
        let total_count = results.len();
        let search_time_ms = start.elapsed().as_millis() as u64;

        tracing::debug!(
            "Keyword search '{}': {} results in {}ms (tokenizer={})",
            display_query,
            total_count,
            search_time_ms,
            use_tokenizer
        );

        SearchResponse {
            results,
            total_count,
            search_time_ms,
            search_mode: "keyword".to_string(),
        }
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
                        total_chunks: 0,
                        lineage_id: None,
                        lineage_role: None,
                        version_label: None,
                        version_count: 0,
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
                    total_chunks: 0,
                    lineage_id: None,
                    lineage_role: None,
                    version_label: None,
                    version_count: 0,
                })
                .collect()
        };

        let mut results = Self::dedup_hwp_hwpx(results);
        if let Ok(conn) = self.get_connection() {
            enrich_total_chunks(&conn, &mut results);
            enrich_lineage_info(&conn, &mut results);
        }
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

/// 인라인 연산자 검색 end-to-end (임시 DB).
/// 단일 #[tokio::test] 에서 순차 검증해 전역 pool(db_path) race 를 피한다
/// (smart.rs filter_search_integration 과 동일 패턴).
#[cfg(test)]
mod operator_search_integration {
    use super::*;
    use crate::db;

    #[tokio::test]
    async fn operator_search_end_to_end() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("ops.db");
        db::init_database(&db_path).unwrap();

        let now = chrono::Utc::now().timestamp();
        let old = 1718409600; // 2024-06-15 UTC — after:2025 경계 밖 고정값

        let add = |path: &str, name: &str, ft: &str, modified: i64, content: &str| {
            let conn = db::get_connection(&db_path).unwrap();
            let fid = db::upsert_file(&conn, path, name, ft, 100, modified).unwrap();
            db::insert_chunk(
                &conn,
                fid,
                0,
                content,
                0,
                content.len(),
                None,
                None,
                None,
                None,
            )
            .unwrap();
        };

        add(
            "C:/docs/2024/budget_final.hwp",
            "budget_final.hwp",
            "hwp",
            now,
            "budget plan approved",
        );
        add(
            "C:/docs/2024/budget_draft.pdf",
            "budget_draft.pdf",
            "pdf",
            now,
            "budget plan draft monthly",
        );
        add(
            "C:/docs/archive/budget_old.hwpx",
            "budget_old.hwpx",
            "hwpx",
            old,
            "plan budget reversed order",
        );
        add(
            "C:/docs/2024/near_close.txt",
            "near_close.txt",
            "txt",
            now,
            "alpha beta gamma",
        );
        add(
            "C:/docs/2024/near_far.txt",
            "near_far.txt",
            "txt",
            now,
            "alpha one two three four five six seven eight nine ten eleven beta",
        );

        let svc = SearchService::new(db_path, None, None, None, None);
        let names = |r: &SearchResponse| -> Vec<String> {
            r.results.iter().map(|x| x.file_name.clone()).collect()
        };
        let search = |q: &'static str| {
            let svc = &svc;
            async move {
                svc.search_keyword_with_operators(q, 50, None, KeywordMode::And)
                    .await
                    .unwrap()
            }
        };

        // 1) 구문 검색: "budget plan" 인접 순서 — 역순 문서 제외
        let n = names(&search("\"budget plan\"").await);
        assert!(n.contains(&"budget_final.hwp".to_string()), "{n:?}");
        assert!(n.contains(&"budget_draft.pdf".to_string()), "{n:?}");
        assert!(!n.contains(&"budget_old.hwpx".to_string()), "역순 오검출: {n:?}");

        // 2) 제외어: monthly 포함 문서 탈락
        let n = names(&search("budget -monthly").await);
        assert!(n.contains(&"budget_final.hwp".to_string()), "{n:?}");
        assert!(!n.contains(&"budget_draft.pdf".to_string()), "제외 실패: {n:?}");

        // 3) ext: 레거시 그룹 확장 (hwp → hwp+hwpx)
        let n = names(&search("budget ext:hwp").await);
        assert!(n.contains(&"budget_final.hwp".to_string()), "{n:?}");
        assert!(n.contains(&"budget_old.hwpx".to_string()), "그룹 확장 누락: {n:?}");
        assert!(!n.iter().any(|x| x.ends_with(".pdf")), "pdf 오검출: {n:?}");

        // 4) path: 경로 부분 일치
        let n = names(&search("budget path:archive").await);
        assert_eq!(n, vec!["budget_old.hwpx".to_string()], "{n:?}");

        // 5) after: 최근 1년 — 오래된 문서 제외
        let n = names(&search("budget after:2025").await);
        assert!(n.contains(&"budget_final.hwp".to_string()), "{n:?}");
        assert!(!n.contains(&"budget_old.hwpx".to_string()), "날짜 필터 실패: {n:?}");

        // 6) ~N 근접 검색: 인접 문서만 매칭, 거리 확대 시 둘 다
        let n = names(&search("alpha beta ~3").await);
        assert!(n.contains(&"near_close.txt".to_string()), "{n:?}");
        assert!(!n.contains(&"near_far.txt".to_string()), "NEAR 거리 초과 오검출: {n:?}");

        let n = names(&search("alpha beta ~20").await);
        assert!(n.contains(&"near_close.txt".to_string()), "{n:?}");
        assert!(n.contains(&"near_far.txt".to_string()), "NEAR 거리 내 누락: {n:?}");

        // 7) 필터 전용 질의: 검색어 없이 ext: 만 → 메타 브라우즈
        let n = names(&search("ext:pdf").await);
        assert_eq!(n, vec!["budget_draft.pdf".to_string()], "{n:?}");

        // 8) 연산자 없는 일반 질의는 기존 경로 (회귀 가드)
        let n = names(&search("budget").await);
        assert!(n.len() >= 3, "일반 검색 회귀: {n:?}");
    }
}
