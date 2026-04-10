//! 자연어(스마트) 검색 + 문서 분류

use super::helpers::*;
use super::SearchService;
use crate::application::dto::search::{MatchType, SearchResponse, SearchResult, SmartSearchResponse};
use crate::application::errors::{AppError, AppResult};
use std::time::Instant;

impl SearchService {
    /// 필터 전용 검색: 키워드 없이 날짜/파일타입 필터만으로 최근 문서 조회
    pub(super) async fn browse_recent_files(
        &self,
        max_results: usize,
        folder_scope: Option<&str>,
    ) -> AppResult<SearchResponse> {
        let conn = self.get_connection()?;

        let (sql, params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = if let Some(s) =
            folder_scope
        {
            let escaped = s
                .to_lowercase()
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_");
            let scope_pattern = format!("{}%", escaped);
            (
                "SELECT f.path, f.name, f.file_type, f.size, f.modified_at
                 FROM files f
                 WHERE f.modified_at IS NOT NULL AND LOWER(f.path) LIKE ?2 ESCAPE '\\'
                 ORDER BY f.modified_at DESC
                 LIMIT ?1"
                    .to_string(),
                vec![
                    Box::new(max_results as i64) as Box<dyn rusqlite::types::ToSql>,
                    Box::new(scope_pattern),
                ],
            )
        } else {
            (
                "SELECT f.path, f.name, f.file_type, f.size, f.modified_at
                 FROM files f
                 WHERE f.modified_at IS NOT NULL
                 ORDER BY f.modified_at DESC
                 LIMIT ?1"
                    .to_string(),
                vec![Box::new(max_results as i64) as Box<dyn rusqlite::types::ToSql>],
            )
        };

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

        let base = if parsed.keywords.trim().is_empty() {
            self.browse_recent_files(over_fetch, folder_scope).await?
        } else if self.embedder.is_some() && self.vector_index.is_some() {
            self.search_hybrid(&parsed.keywords, over_fetch, folder_scope)
                .await?
        } else {
            self.search_keyword(&parsed.keywords, over_fetch, folder_scope)
                .await?
        };

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
            query, parsed.keywords, total_count, search_time_ms
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
                &["시행령", "시행규칙", "조례", "법률 제", "별표", "동법", "같은 법"],
                2,
            ),
            (
                "공문",
                &["수신 :", "수신:", "발신 :", "발신:", "관인생략", "시행 20", "경유 :"],
                2,
            ),
            (
                "기안문",
                &["기안자", "기안일자", "검토자", "결재일자", "협조자"],
                2,
            ),
            (
                "회의록",
                &["회의록", "참석자", "안건", "결정사항", "회의일시", "회의 장소"],
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
