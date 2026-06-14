//! 검색창 인라인 연산자 파서 (v3.0.0)
//!
//! 지원 문법 (키워드/하이브리드 검색 전용 — 스마트(자연어)·RAG 경로는 미적용):
//! - `"정확 구문"`            : 구문 검색 (어절 인접 순서 매칭)
//! - `-제외어` / `-"제외 구문"` : 해당 단어 포함 문서 제외
//! - `ext:hwp` / `ext:hwp,pdf` : 확장자 필터 (hwp↔hwpx 등 레거시 그룹 자동 확장)
//! - `path:단어`               : 파일 경로 부분 일치 (대소문자 무시)
//! - `after:2024-06` / `before:2025-01-31` : 수정일 범위 (YYYY[-MM[-DD]], `.`/`/` 구분자 허용)
//! - `~N` (예: `~10`)          : 근접 검색 — 어절들이 N 토큰 이내 (FTS5 NEAR)
//!
//! 파서는 순수 함수로, FTS MATCH 식 합성은 `fts::compose_fts_match`,
//! 날짜·확장자·경로 필터는 `fts::MetaFilter` 로 매핑된다.

use chrono::{FixedOffset, NaiveDate};

/// 근접 검색(`~`)에 숫자가 없을 때 기본 토큰 거리
pub const DEFAULT_NEAR_DISTANCE: u32 = 10;

/// 인라인 연산자 파싱 결과
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OperatorQuery {
    /// 연산자를 제거하고 남은 일반 검색어 (공백 구분)
    pub terms: String,
    /// `"..."` 정확 구문 목록
    pub phrases: Vec<String>,
    /// `-단어` 제외어 목록
    pub excludes: Vec<String>,
    /// `ext:` 확장자 목록 (소문자, 점 없음, 그룹 확장 완료 — 예: hwp → [hwp, hwpx])
    pub ext_filters: Vec<String>,
    /// `path:` 경로 부분 일치 토큰 (소문자)
    pub path_filters: Vec<String>,
    /// `after:` — 이 시각 이후 수정 (inclusive, Unix ts)
    pub after: Option<i64>,
    /// `before:` — 이 시각 이전 수정 (inclusive, Unix ts)
    pub before: Option<i64>,
    /// `~N` 근접 검색 토큰 거리
    pub near: Option<u32>,
}

impl OperatorQuery {
    /// 연산자가 하나라도 사용됐는지 (terms만 있으면 false → 기존 경로 그대로)
    pub fn has_operators(&self) -> bool {
        !self.phrases.is_empty()
            || !self.excludes.is_empty()
            || !self.ext_filters.is_empty()
            || !self.path_filters.is_empty()
            || self.after.is_some()
            || self.before.is_some()
            || self.near.is_some()
    }

    /// 시맨틱(벡터) 검색용 텍스트 — 연산자 제거, 의미 있는 검색어만
    pub fn semantic_text(&self) -> String {
        let mut parts: Vec<&str> = Vec::new();
        if !self.terms.is_empty() {
            parts.push(&self.terms);
        }
        for p in &self.phrases {
            parts.push(p);
        }
        parts.join(" ")
    }

    /// after/before → MetaFilter date_range 형식
    pub fn date_range(&self) -> Option<(i64, i64)> {
        if self.after.is_none() && self.before.is_none() {
            return None;
        }
        Some((self.after.unwrap_or(0), self.before.unwrap_or(i64::MAX)))
    }
}

/// 검색어에서 인라인 연산자를 분리한다.
///
/// 인용부호가 닫히지 않으면 나머지 전체를 구문으로 취급한다.
/// 연산자 값이 비어 있으면(`ext:` 단독 등) 해당 토큰은 무시한다.
pub fn parse_operators(raw: &str) -> OperatorQuery {
    let mut result = OperatorQuery::default();
    let mut terms: Vec<String> = Vec::new();

    for token in tokenize_respecting_quotes(raw) {
        match token {
            QueryToken::Phrase(p) => {
                let p = p.trim().to_string();
                if !p.is_empty() {
                    result.phrases.push(p);
                }
            }
            QueryToken::Exclude(e) => {
                let e = e.trim().to_string();
                if !e.is_empty() {
                    result.excludes.push(e);
                }
            }
            QueryToken::Word(w) => {
                if let Some(rest) = strip_prefix_ci(&w, "ext:").or_else(|| strip_prefix_ci(&w, "type:")) {
                    for ext in rest.split(',') {
                        let ext = ext.trim().trim_start_matches('.').to_lowercase();
                        if !ext.is_empty() {
                            for expanded in expand_ext_group(&ext) {
                                if !result.ext_filters.contains(&expanded) {
                                    result.ext_filters.push(expanded);
                                }
                            }
                        }
                    }
                } else if let Some(rest) = strip_prefix_ci(&w, "path:") {
                    let p = rest.trim().to_lowercase();
                    if !p.is_empty() {
                        result.path_filters.push(p);
                    }
                } else if let Some(rest) = strip_prefix_ci(&w, "after:") {
                    if let Some((start, _)) = parse_date_token(rest) {
                        result.after = Some(start);
                    } else {
                        terms.push(w);
                    }
                } else if let Some(rest) = strip_prefix_ci(&w, "before:") {
                    if let Some((_, end)) = parse_date_token(rest) {
                        result.before = Some(end);
                    } else {
                        terms.push(w);
                    }
                } else if let Some(rest) = w.strip_prefix('~') {
                    if rest.is_empty() {
                        result.near = Some(DEFAULT_NEAR_DISTANCE);
                    } else if let Ok(n) = rest.parse::<u32>() {
                        result.near = Some(n.clamp(1, 100));
                    } else {
                        terms.push(w);
                    }
                } else {
                    terms.push(w);
                }
            }
        }
    }

    result.terms = terms.join(" ");
    result
}

/// 토큰 종류: 일반 단어 / 인용 구문 / 제외(`-`)
enum QueryToken {
    Word(String),
    Phrase(String),
    Exclude(String),
}

/// 인용부호를 존중하며 토큰화. `-"구문"`, `-단어`, `"구문"`, 일반 단어를 구분한다.
fn tokenize_respecting_quotes(raw: &str) -> Vec<QueryToken> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = raw.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // 공백 스킵
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= chars.len() {
            break;
        }

        let negated = chars[i] == '-' && i + 1 < chars.len() && !chars[i + 1].is_whitespace();
        if negated {
            i += 1;
        }

        if chars[i] == '"' {
            // 인용 구문: 닫는 따옴표까지 (미폐쇄 시 끝까지)
            i += 1;
            let start = i;
            while i < chars.len() && chars[i] != '"' {
                i += 1;
            }
            let phrase: String = chars[start..i].iter().collect();
            if i < chars.len() {
                i += 1; // 닫는 따옴표 소비
            }
            if negated {
                tokens.push(QueryToken::Exclude(phrase));
            } else {
                tokens.push(QueryToken::Phrase(phrase));
            }
        } else {
            let start = i;
            while i < chars.len() && !chars[i].is_whitespace() {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            if negated {
                tokens.push(QueryToken::Exclude(word));
            } else {
                tokens.push(QueryToken::Word(word));
            }
        }
    }

    tokens
}

/// 대소문자 무시 prefix 제거 (멀티바이트 경계 안전 — `get`으로 비경계 슬라이스 회피)
fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    let head = s.get(..prefix.len())?;
    if head.eq_ignore_ascii_case(prefix) {
        s.get(prefix.len()..)
    } else {
        None
    }
}

/// 레거시 확장자 그룹 확장 — 스마트 검색의 file_type_extensions(helpers.rs)와 동일 그룹.
/// (application 레이어 의존을 피하기 위한 의도적 미러 — 그룹 변경 시 양쪽 동기화)
fn expand_ext_group(ext: &str) -> Vec<String> {
    match ext {
        "hwp" | "hwpx" => vec!["hwp".into(), "hwpx".into()],
        "doc" | "docx" => vec!["doc".into(), "docx".into()],
        "xls" | "xlsx" => vec!["xls".into(), "xlsx".into()],
        "ppt" | "pptx" => vec!["ppt".into(), "pptx".into()],
        other => vec![other.to_string()],
    }
}

/// `YYYY[-MM[-DD]]` (구분자 `-`/`.`/`/`) → (기간 시작 ts, 기간 끝 ts).
/// KST 기준 — 스마트 검색 date_filter_range 와 동일한 시간대 의미론.
fn parse_date_token(s: &str) -> Option<(i64, i64)> {
    let normalized = s.trim().replace(['.', '/'], "-");
    let parts: Vec<&str> = normalized.split('-').filter(|p| !p.is_empty()).collect();

    let (start_date, end_date) = match parts.as_slice() {
        [y] => {
            let year: i32 = y.parse().ok().filter(|y| (1970..=2100).contains(y))?;
            (
                NaiveDate::from_ymd_opt(year, 1, 1)?,
                NaiveDate::from_ymd_opt(year, 12, 31)?,
            )
        }
        [y, m] => {
            let year: i32 = y.parse().ok().filter(|y| (1970..=2100).contains(y))?;
            let month: u32 = m.parse().ok().filter(|m| (1..=12).contains(m))?;
            let start = NaiveDate::from_ymd_opt(year, month, 1)?;
            (start, last_day_of_month(year, month)?)
        }
        [y, m, d] => {
            let year: i32 = y.parse().ok().filter(|y| (1970..=2100).contains(y))?;
            let month: u32 = m.parse().ok()?;
            let day: u32 = d.parse().ok()?;
            let date = NaiveDate::from_ymd_opt(year, month, day)?;
            (date, date)
        }
        _ => return None,
    };

    let kst = FixedOffset::east_opt(9 * 3600)?;
    let start = start_date.and_hms_opt(0, 0, 0)?;
    let end = end_date.and_hms_opt(23, 59, 59)?;
    Some((
        start.and_local_timezone(kst).single()?.timestamp(),
        end.and_local_timezone(kst).single()?.timestamp(),
    ))
}

fn last_day_of_month(year: i32, month: u32) -> Option<NaiveDate> {
    let next_month_first = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)?
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)?
    };
    next_month_first.pred_opt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_query_has_no_operators() {
        let q = parse_operators("예산 보고서");
        assert_eq!(q.terms, "예산 보고서");
        assert!(!q.has_operators());
    }

    #[test]
    fn quoted_phrase_extracted() {
        let q = parse_operators("\"감사 결과\" 보고서");
        assert_eq!(q.phrases, vec!["감사 결과"]);
        assert_eq!(q.terms, "보고서");
        assert!(q.has_operators());
    }

    #[test]
    fn unclosed_quote_treated_as_phrase() {
        let q = parse_operators("\"감사 결과");
        assert_eq!(q.phrases, vec!["감사 결과"]);
        assert_eq!(q.terms, "");
    }

    #[test]
    fn exclude_word_and_phrase() {
        let q = parse_operators("예산 -초안 -\"중간 보고\"");
        assert_eq!(q.terms, "예산");
        assert_eq!(q.excludes, vec!["초안", "중간 보고"]);
    }

    #[test]
    fn hyphen_inside_word_not_exclude() {
        // 공백 뒤 '-'만 제외 연산자 — 단어 중간 하이픈은 그대로
        let q = parse_operators("COVID-19 대응");
        assert_eq!(q.terms, "COVID-19 대응");
        assert!(q.excludes.is_empty());
    }

    #[test]
    fn standalone_hyphen_is_term() {
        let q = parse_operators("예산 - 보고");
        assert_eq!(q.terms, "예산 - 보고");
        assert!(q.excludes.is_empty());
    }

    #[test]
    fn ext_filter_expands_legacy_groups() {
        let q = parse_operators("예산 ext:hwp");
        assert_eq!(q.ext_filters, vec!["hwp", "hwpx"]);
        assert_eq!(q.terms, "예산");

        let q = parse_operators("ext:.PDF,xls");
        assert_eq!(q.ext_filters, vec!["pdf", "xls", "xlsx"]);
    }

    #[test]
    fn path_filter_lowercased() {
        let q = parse_operators("보고서 path:2024예산 path:DraftS");
        assert_eq!(q.path_filters, vec!["2024예산", "drafts"]);
        assert_eq!(q.terms, "보고서");
    }

    #[test]
    fn date_year_month_day_ranges() {
        let q = parse_operators("after:2024 before:2025-06");
        let (after, before) = (q.after.unwrap(), q.before.unwrap());
        assert!(after < before);
        // after:2024 → 2024-01-01 00:00:00 KST = 2023-12-31 15:00:00 UTC
        assert_eq!(after, 1704034800);
        // before:2025-06 → 2025-06-30 23:59:59 KST
        assert_eq!(before, 1751295599);
    }

    #[test]
    fn date_with_dot_separator() {
        let q = parse_operators("before:2025.01.31");
        assert!(q.before.is_some());
    }

    #[test]
    fn invalid_date_falls_back_to_term() {
        let q = parse_operators("after:내일 보고서");
        assert!(q.after.is_none());
        assert_eq!(q.terms, "after:내일 보고서");
    }

    #[test]
    fn near_operator_with_and_without_distance() {
        let q = parse_operators("예산 삭감 ~5");
        assert_eq!(q.near, Some(5));
        assert_eq!(q.terms, "예산 삭감");

        let q = parse_operators("예산 삭감 ~");
        assert_eq!(q.near, Some(DEFAULT_NEAR_DISTANCE));

        let q = parse_operators("예산 ~999");
        assert_eq!(q.near, Some(100), "거리는 100으로 클램프");
    }

    #[test]
    fn tilde_word_is_term() {
        let q = parse_operators("~abc");
        assert_eq!(q.terms, "~abc");
        assert!(q.near.is_none());
    }

    #[test]
    fn combined_operators() {
        let q = parse_operators("ext:hwp \"감사 결과\" -초안 path:2024 after:2024-01 예산");
        assert_eq!(q.terms, "예산");
        assert_eq!(q.phrases, vec!["감사 결과"]);
        assert_eq!(q.excludes, vec!["초안"]);
        assert_eq!(q.ext_filters, vec!["hwp", "hwpx"]);
        assert_eq!(q.path_filters, vec!["2024"]);
        assert!(q.after.is_some());
        assert!(q.date_range().is_some());
    }

    #[test]
    fn semantic_text_strips_operators() {
        let q = parse_operators("ext:hwp \"감사 결과\" -초안 예산");
        assert_eq!(q.semantic_text(), "예산 감사 결과");
    }

    #[test]
    fn empty_operator_values_ignored() {
        let q = parse_operators("ext: path: 보고서");
        assert!(q.ext_filters.is_empty());
        assert!(q.path_filters.is_empty());
        assert_eq!(q.terms, "보고서");
    }
}
