//! 자연어(스마트) 검색 + 문서 분류

use super::helpers::*;
use super::SearchService;
use crate::application::dto::search::{
    MatchType, SearchResponse, SearchResult, SmartSearchResponse,
};
use crate::application::errors::{AppError, AppResult};
use crate::search::fts::MetaFilter;
use crate::search::nl_query::DateFilter;
use crate::search::KeywordMode;
use std::time::Instant;

impl SearchService {
    /// 필터 전용 검색: 키워드 없이 날짜/파일타입 필터만으로 문서 조회.
    ///
    /// 날짜·파일타입을 SQL WHERE로 직접 적용해 전체 인덱스를 대상으로 거른다.
    /// (예전에는 최근 N건만 가져와 후처리로 걸러서, 과거 파일이나 드문 타입은
    ///  최근 목록 범위 밖이면 0건이 되는 버그가 있었다.)
    pub(super) async fn browse_recent_files(
        &self,
        max_results: usize,
        folder_scope: Option<&str>,
        date_filter: &Option<DateFilter>,
        file_type: &Option<String>,
    ) -> AppResult<SearchResponse> {
        let conn = self.get_connection()?;

        let mut where_clauses: Vec<String> = vec!["f.modified_at IS NOT NULL".to_string()];
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        // 폴더 스코프 — FTS 경로(fts.rs)와 동일한 공용 패턴 사용.
        // 자체 prefix LIKE는 segment 경계가 없어 `2024` 스코프가 `2024-archive`까지
        // 포함(sibling 누수)하고, 구분자 정규화가 없어 `/` 표기 스코프가
        // Windows `\` 저장 경로와 불일치해 0건이 되는 문제가 있었다.
        if let Some(s) = folder_scope {
            if let Some(pat) = crate::utils::folder_scope::scope_like_pattern(s) {
                params.push(Box::new(pat));
                where_clauses.push(format!(
                    "REPLACE(LOWER(f.path), '\\', '/') LIKE ?{} ESCAPE '\\'",
                    params.len()
                ));
            }
        }

        // 날짜 필터 → modified_at 범위 (후처리와 동일 경계 공유)
        if let Some(df) = date_filter {
            let (start, end) = date_filter_range(df);
            params.push(Box::new(start));
            let s_idx = params.len();
            params.push(Box::new(end));
            let e_idx = params.len();
            where_clauses.push(format!(
                "f.modified_at >= ?{s_idx} AND f.modified_at <= ?{e_idx}"
            ));
        }

        // 파일타입 필터 → 그룹 확장자 IN (hwpx → hwp/hwpx, xlsx → xls/xlsx ...)
        if let Some(ft) = file_type {
            let placeholders: Vec<String> = file_type_extensions(ft)
                .into_iter()
                .map(|ext| {
                    params.push(Box::new(ext));
                    format!("?{}", params.len())
                })
                .collect();
            where_clauses.push(format!(
                "LOWER(f.file_type) IN ({})",
                placeholders.join(", ")
            ));
        }

        // LIMIT
        params.push(Box::new(max_results as i64));
        let limit_idx = params.len();

        let sql = format!(
            "SELECT f.path, f.name, f.file_type, f.size, f.modified_at
             FROM files f
             WHERE {}
             ORDER BY f.modified_at DESC
             LIMIT ?{limit_idx}",
            where_clauses.join(" AND ")
        );

        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| AppError::SearchFailed(e.to_string()))?;

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let results: Vec<SearchResult> = stmt
            .query_map(param_refs.as_slice(), |row| {
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
                    total_chunks: 0,
                    lineage_id: None,
                    lineage_role: None,
                    version_label: None,
                    version_count: 0,
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
        let parsed = match self.tokenizer.as_ref() {
            Some(tok) => NlQueryParser::parse_with_tokenizer(query, tok.as_ref()),
            None => NlQueryParser::parse(query),
        };

        let has_filters = parsed.date_filter.is_some()
            || parsed.file_type.is_some()
            || parsed.filename_filter.is_some()
            || !parsed.exclude_keywords.is_empty();

        if parsed.keywords.trim().is_empty() && !has_filters {
            return Ok(SmartSearchResponse {
                results: vec![],
                total_count: 0,
                search_time_ms: 0,
                parsed_query: parsed,
            });
        }

        let over_fetch = if has_filters {
            max_results * 10
        } else {
            max_results * 3
        };

        // base 결과 생성 전략:
        // 1. filename_filter만 있고 keywords 비어있음 → search_filename (전체 인덱스 대상)
        // 2. keywords 있음 → hybrid/keyword 검색
        // 3. 둘 다 비어있고 다른 필터만 → browse_recent_files
        let has_keywords = !parsed.keywords.trim().is_empty();
        let has_filename = parsed.filename_filter.is_some();

        let base = if has_keywords {
            // 키워드 우선: 날짜·파일타입을 FTS 단계에 함께 적용한다.
            // (후처리로만 거르면 흔한 키워드의 BM25 상위 N 밖으로 타깃이 밀려 누락됨)
            let fts_filter = MetaFilter {
                date_range: parsed.date_filter.as_ref().map(date_filter_range),
                file_types: parsed
                    .file_type
                    .as_deref()
                    .map(file_type_extensions)
                    .unwrap_or_default(),
                ..Default::default()
            };
            if self.embedder.is_some() && self.vector_index.is_some() {
                self.search_hybrid_with_mode(
                    &parsed.keywords,
                    over_fetch,
                    folder_scope,
                    KeywordMode::And,
                    &fts_filter,
                )
                .await?
            } else {
                self.search_keyword_with_mode(
                    &parsed.keywords,
                    over_fetch,
                    folder_scope,
                    KeywordMode::And,
                    &fts_filter,
                )
                .await?
            }
        } else if has_filename {
            // 키워드 없고 파일명 필터만 → 파일명 검색 엔진 활용 (전체 인덱스 대상)
            self.search_filename(
                parsed.filename_filter.as_deref().unwrap_or(""),
                over_fetch,
                folder_scope,
            )
            .await?
        } else {
            // 키워드도 파일명도 없고 다른 필터만 → 필터 조건으로 직접 브라우즈
            self.browse_recent_files(
                over_fetch,
                folder_scope,
                &parsed.date_filter,
                &parsed.file_type,
            )
            .await?
        };

        let now = chrono::Utc::now().timestamp();
        let filtered: Vec<SearchResult> = base
            .results
            .into_iter()
            .filter(|r| smart_apply_date_filter(r, &parsed.date_filter, now))
            .filter(|r| smart_apply_file_type_filter(r, &parsed.file_type))
            .filter(|r| smart_apply_filename_filter(r, &parsed.filename_filter))
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

    /// 문서 첫 청크 기반 카테고리 분류
    pub fn classify_document(&self, text: &str) -> AppResult<String> {
        Ok(Self::classify_by_keyword(text))
    }

    /// 키워드 패턴 매칭 기반 문서 분류
    pub fn classify_by_keyword(text: &str) -> String {
        let article_count = count_article_pattern(text);

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
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ClassifyTestCase {
        name: &'static str,
        text: &'static str,
        expected: &'static str,
    }

    fn test_scenarios() -> Vec<ClassifyTestCase> {
        vec![
            ClassifyTestCase { name: "민법 조문", text: "제750조(불법행위의 내용) 고의 또는 과실로 인한 위법행위로 타인에게 손해를 가한 자는 그 손해를 배상할 책임이 있다. 제751조(재산 이외의 손해의 배상) 타인의 신체, 자유 또는 명예를 해하거나 기타 정신상고통을 가한 자는 재산 이외의 손해에 대하여도 배상할 책임이 있다.", expected: "법령" },
            ClassifyTestCase { name: "근로기준법 시행령", text: "근로기준법 시행령 제1조(목적) 이 영은 「근로기준법」에서 위임된 사항과 그 시행에 필요한 사항을 규정함을 목적으로 한다. 제2조(통상임금) 법과 이 영에서 통상임금이란 근로자에게 정기적이고 일률적으로 소정근로 또는 총근로에 대하여 지급하기로 정한 시간급 금액을 말한다.", expected: "법령" },
            ClassifyTestCase { name: "고용산재보험료 엑셀", text: "58.00 청소과 김하늘 기간제근로자 청소과 인건비 177200.00", expected: "기타" },
            ClassifyTestCase { name: "행정 공문", text: "수신 : 각 과장 (경유) 시행 2024-0301-001 제목: 2024년 상반기 업무보고 계획 안내 관인생략 1. 관련: 기획예산과-1234(2024.02.15.)", expected: "공문" },
            ClassifyTestCase { name: "부서 회의록", text: "회의록 회의일시: 2024년 3월 15일 14:00 회의 장소: 3층 대회의실 참석자: 과장 김철수 안건1: 상반기 사업계획 검토 결정사항: 3월 말까지 세부 일정 확정", expected: "회의록" },
        ]
    }

    #[test]
    fn classify_by_keyword_test_scenarios() {
        for tc in &test_scenarios() {
            let result = SearchService::classify_by_keyword(tc.text);
            assert_eq!(
                result, tc.expected,
                "분류 실패: {} (예상={}, 결과={})",
                tc.name, tc.expected, result
            );
        }
    }
}

/// 필터 전용(키워드 없는) 스마트 검색 end-to-end 통합 테스트.
///
/// pool이 전역 static + db_path 변경 시 drain되므로, 커넥션 혼선을 피하려
/// 모든 시나리오를 단일 테스트(단일 db_path)에서 순차 검증한다.
/// 실행: `cargo test filter_only_smart_search_end_to_end -- --test-threads=1`
#[cfg(test)]
mod filter_search_integration {
    use super::*;
    use crate::application::services::search_service::helpers::date_filter_range;
    use crate::db;

    #[tokio::test]
    async fn filter_only_smart_search_end_to_end() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("test.db");
        db::init_database(&db_path).unwrap();

        let now = chrono::Utc::now().timestamp();
        let (ly_s, ly_e) = date_filter_range(&DateFilter::LastYear);
        let last_year = ly_s + (ly_e - ly_s) / 2;
        let (lm_s, lm_e) = date_filter_range(&DateFilter::LastMonth);
        let last_month = lm_s + (lm_e - lm_s) / 2;

        let insert = |name: &str, ft: &str, modified_at: i64| {
            let conn = db::get_connection(&db_path).unwrap();
            db::upsert_file(
                &conn,
                &format!("C:/docs/{name}"),
                name,
                ft,
                1024,
                modified_at,
            )
            .unwrap();
        };

        insert("작년보고서.hwp", "hwp", last_year);
        insert("작년계획.hwpx", "hwpx", last_year);
        insert("오래된명부.xls", "xls", last_year);
        insert("예산.xlsx", "xlsx", last_year);
        insert("지난달안내.pdf", "pdf", last_month);
        insert("지난달기안.hwp", "hwp", last_month);
        // 노이즈: 최근 txt 다수 → 옛 browse(최근순 LIMIT)였다면 타깃을 전부 밀어냄
        for i in 0..60 {
            insert(&format!("메모{i}.txt"), "txt", now - i as i64);
        }

        let svc = SearchService::new(db_path, None, None, None, None);
        let names = |r: &SmartSearchResponse| -> Vec<String> {
            r.results.iter().map(|x| x.file_name.clone()).collect()
        };

        // 1) "xlsx" → .xls + .xlsx 둘 다, 최근 txt 노이즈에 안 밀림
        let r = svc.search_smart("xlsx", 5, None).await.unwrap();
        let n = names(&r);
        assert!(n.contains(&"예산.xlsx".to_string()), "xlsx 누락: {n:?}");
        assert!(
            n.contains(&"오래된명부.xls".to_string()),
            "레거시 .xls 누락: {n:?}"
        );
        assert!(n.iter().all(|x| !x.ends_with(".txt")), "txt 오검출: {n:?}");

        // 2) "작년 hwp" → 작년 .hwp + .hwpx, 지난달/타입 불일치 제외
        let r = svc.search_smart("작년 hwp", 50, None).await.unwrap();
        let n = names(&r);
        assert!(
            n.contains(&"작년보고서.hwp".to_string()),
            "작년 .hwp 누락: {n:?}"
        );
        assert!(
            n.contains(&"작년계획.hwpx".to_string()),
            "작년 .hwpx 누락: {n:?}"
        );
        assert!(
            !n.contains(&"지난달기안.hwp".to_string()),
            "지난달 오검출: {n:?}"
        );
        assert!(!n.contains(&"예산.xlsx".to_string()), "xlsx 오검출: {n:?}");

        // 3) "지난달 pdf" → 지난달 .pdf만
        let r = svc.search_smart("지난달 pdf", 50, None).await.unwrap();
        let n = names(&r);
        assert_eq!(
            n,
            vec!["지난달안내.pdf".to_string()],
            "지난달 pdf 정확히 1건: {n:?}"
        );

        // 4) "지난달 한글 파일" → 지난달 .hwp (한글=hwp 그룹)
        let r = svc
            .search_smart("지난달 한글 파일", 50, None)
            .await
            .unwrap();
        let n = names(&r);
        assert!(
            n.contains(&"지난달기안.hwp".to_string()),
            "지난달 한글 누락: {n:?}"
        );
        assert!(
            !n.contains(&"작년보고서.hwp".to_string()),
            "작년 오검출: {n:?}"
        );

        // 키워드+필터(FTS push) 경로도 동일 격리 컨텍스트에서 순차 검증
        keyword_with_filter_scenario().await;
    }

    /// 키워드 + 날짜/파일타입 조합("budget 작년 hwp")이 FTS 단계에서 필터되어,
    /// 노이즈가 BM25 상위를 점령해도 타깃이 누락되지 않음을 검증.
    /// (content 는 FTS5 한글 토큰화 의존을 피하려 영문 "budget" 사용)
    /// 단일 #[tokio::test] 에서 순차 호출되어 전역 pool(db_path) race 를 피한다.
    async fn keyword_with_filter_scenario() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("kw.db");
        db::init_database(&db_path).unwrap();

        let now = chrono::Utc::now().timestamp();
        let (ly_s, ly_e) = date_filter_range(&DateFilter::LastYear);
        let last_year = ly_s + (ly_e - ly_s) / 2;
        let (lm_s, lm_e) = date_filter_range(&DateFilter::LastMonth);
        let last_month = lm_s + (lm_e - lm_s) / 2;

        let add = |name: &str, ft: &str, modified: i64, content: &str| {
            let conn = db::get_connection(&db_path).unwrap();
            let fid =
                db::upsert_file(&conn, &format!("C:/d/{name}"), name, ft, 100, modified).unwrap();
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

        add("작년예산.hwp", "hwp", last_year, "budget plan");
        add("작년예산.hwpx", "hwpx", last_year, "annual budget");
        add("지난달예산.hwp", "hwp", last_month, "budget monthly");
        add("작년예산.pdf", "pdf", last_year, "budget pdf");
        // 노이즈: "budget" 밀도 높은 최근 txt 60개 (over_fetch=5*10=50 초과 → 옛 코드면 타깃 밀려남)
        for i in 0..60 {
            add(
                &format!("메모{i}.txt"),
                "txt",
                now - i as i64,
                "budget budget note",
            );
        }

        let svc = SearchService::new(db_path, None, None, None, None);
        let r = svc.search_smart("budget 작년 hwp", 5, None).await.unwrap();
        let names: Vec<String> = r.results.iter().map(|x| x.file_name.clone()).collect();

        assert!(
            names.contains(&"작년예산.hwp".to_string()),
            "작년 hwp 누락: {names:?}"
        );
        assert!(
            names.contains(&"작년예산.hwpx".to_string()),
            "작년 hwpx 누락: {names:?}"
        );
        assert!(
            !names.contains(&"지난달예산.hwp".to_string()),
            "지난달 오검출: {names:?}"
        );
        assert!(
            !names.iter().any(|x| x.ends_with(".pdf")),
            "pdf 오검출: {names:?}"
        );
        assert!(
            !names.iter().any(|x| x.ends_with(".txt")),
            "txt 노이즈 오검출: {names:?}"
        );
    }
}
