//! 검색 헬퍼 함수 — 스코어링, 스니펫, 페이지 보간, 스마트 검색 필터

use crate::application::dto::search::SearchResult;
use crate::db;
use crate::search::nl_query::DateFilter;
use rusqlite::Connection;
use std::collections::HashSet;

/// 검색 결과에 파일별 total_chunks를 채워 넣는다 (히트맵 절대 스케일용).
/// 실패 시 조용히 무시 (total_chunks는 0으로 남음).
pub fn enrich_total_chunks(conn: &Connection, results: &mut [SearchResult]) {
    if results.is_empty() {
        return;
    }
    let unique_paths: HashSet<String> = results.iter().map(|r| r.file_path.clone()).collect();
    let paths: Vec<String> = unique_paths.into_iter().collect();
    let counts = match db::get_chunk_counts_by_file_paths(conn, &paths) {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!("enrich_total_chunks failed: {}", e);
            return;
        }
    };
    for r in results.iter_mut() {
        if let Some(&count) = counts.get(&r.file_path) {
            r.total_chunks = count;
        }
    }
}

// ── 스코어 정규화 ─────────────────────────────────────

/// FTS5 BM25 스코어를 confidence로 변환
///
/// min-max 정규화에 절대 스코어 기반 감쇠를 적용하여
/// 약한 매칭만 있는 결과 집합에서도 과대평가를 방지
pub fn normalize_fts_confidence(scores: &[f64]) -> Vec<u8> {
    if scores.is_empty() {
        return vec![];
    }

    let min = scores.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let best_abs = min.abs();
    let quality_factor = (best_abs / 5.0).min(1.0);

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
pub fn normalize_vector_confidence(score: f64) -> u8 {
    (score * 100.0).round().clamp(0.0, 100.0) as u8
}

/// RRF 스코어를 confidence로 변환
pub fn normalize_rrf_confidence(score: f64, k: f64) -> u8 {
    let max_possible = 2.0 / (k + 1.0);
    let normalized = (score / max_possible).min(1.0);
    (normalized * 100.0).round().clamp(0.0, 100.0) as u8
}

// ── 스니펫 / 하이라이트 ───────────────────────────────

/// 미리보기 텍스트 자르기
pub fn truncate_preview(content: &str, max_len: usize) -> String {
    if content.chars().count() <= max_len {
        content.to_string()
    } else {
        let truncated: String = content.chars().take(max_len).collect();
        format!("{}...", truncated)
    }
}

/// snippet에서 하이라이트 마커 제거
pub fn strip_highlight_markers(snippet: &str) -> String {
    snippet.replace("[[HL]]", "").replace("[[/HL]]", "")
}

/// FTS5 snippet에 키워드가 없을 때 content에서 키워드를 찾아 커스텀 snippet 생성
fn create_keyword_snippet(content: &str, query: &str) -> Option<String> {
    let query_trimmed = query.trim();
    if query_trimmed.is_empty() || content.is_empty() {
        return None;
    }

    let query_lower = query_trimmed.to_lowercase();
    let content_lower = content.to_lowercase();

    let byte_pos = content_lower.find(&query_lower)?;
    let char_pos = content_lower[..byte_pos].chars().count();
    let kw_char_len = query_trimmed.chars().count();

    let content_chars: Vec<char> = content.chars().collect();
    let total_chars = content_chars.len();

    if char_pos + kw_char_len > total_chars {
        return None;
    }

    let start = char_pos.saturating_sub(60);
    let end = (char_pos + kw_char_len + 200).min(total_chars);

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
pub fn ensure_keyword_in_snippet(fts_snippet: &str, content: &str, query: &str) -> String {
    let query_trimmed = query.trim();
    if query_trimmed.is_empty() {
        return fts_snippet.to_string();
    }

    let stripped_lower = strip_highlight_markers(fts_snippet).to_lowercase();
    let keywords: Vec<&str> = query_trimmed.split_whitespace().collect();

    if keywords
        .iter()
        .any(|kw| stripped_lower.contains(&kw.to_lowercase()))
    {
        return fts_snippet.to_string();
    }

    if let Some(snippet) = create_keyword_snippet(content, query_trimmed) {
        return snippet;
    }

    for kw in &keywords {
        if let Some(snippet) = create_keyword_snippet(content, kw) {
            return snippet;
        }
    }

    fts_snippet.to_string()
}

/// highlight() 결과에서 하이라이트 범위 추출 (O(n) 최적화)
pub fn parse_highlight_ranges(marked: &str) -> Vec<(usize, usize)> {
    const HL_START: &str = "[[HL]]";
    const HL_END: &str = "[[/HL]]";

    let mut ranges = Vec::new();
    let mut clean_pos = 0;
    let mut rest = marked;

    while !rest.is_empty() {
        if let Some(pos) = rest.find(HL_START) {
            clean_pos += rest[..pos].chars().count();
            rest = &rest[pos + HL_START.len()..];

            let start = clean_pos;

            if let Some(end_pos) = rest.find(HL_END) {
                clean_pos += rest[..end_pos].chars().count();
                ranges.push((start, clean_pos));
                rest = &rest[end_pos + HL_END.len()..];
            } else {
                clean_pos += rest.chars().count();
                ranges.push((start, clean_pos));
                break;
            }
        } else {
            break;
        }
    }

    ranges
}

// ── 페이지 보간 ───────────────────────────────────────

/// 키워드 위치 기반 페이지 보간
pub fn interpolate_page_from_snippet(
    page_start: Option<i64>,
    page_end: Option<i64>,
    chunk_content: &str,
    snippet: &str,
) -> Option<i64> {
    let ps = page_start?;
    let pe = page_end.unwrap_or(ps);

    if ps == pe {
        return Some(ps);
    }

    let hl_start = snippet.find("[[HL]]")?;
    let after_hl = &snippet[hl_start + 6..];
    let hl_end = after_hl.find("[[/HL]]")?;
    let keyword = &after_hl[..hl_end];

    if keyword.is_empty() {
        return Some(ps);
    }

    let keyword_pos = chunk_content.find(keyword)?;
    let chunk_len = chunk_content.len().max(1);

    let ratio = keyword_pos as f64 / chunk_len as f64;
    let page_span = (pe - ps) as f64;
    let interpolated = ps as f64 + ratio * page_span;

    Some(interpolated.round() as i64)
}

// ── 벡터 검색 folder_scope 필터 ──────────────────────

/// 벡터 검색 결과의 folder_scope 후처리 필터 (Windows: case-insensitive)
pub fn matches_folder_scope(file_path: &str, folder_scope: Option<&str>) -> bool {
    match folder_scope {
        Some(scope) if !scope.is_empty() => {
            file_path.to_lowercase().starts_with(&scope.to_lowercase())
        }
        _ => true,
    }
}

// ── Smart Search 후처리 필터 ─────────────────────────

/// 날짜 필터 적용
pub fn smart_apply_date_filter(r: &SearchResult, filter: &Option<DateFilter>, _now: i64) -> bool {
    use chrono::{Datelike, Duration, FixedOffset};

    let Some(filter) = filter else { return true };
    let Some(modified) = r.modified_at else {
        return false;
    };

    let kst = FixedOffset::east_opt(9 * 3600).unwrap();
    let today = chrono::Utc::now().with_timezone(&kst).date_naive();

    let (start, end) = match filter {
        DateFilter::Today => {
            let s = today.and_hms_opt(0, 0, 0).unwrap();
            (kst_to_utc(&kst, s), i64::MAX)
        }
        DateFilter::ThisWeek => {
            let days_since_mon = today.weekday().num_days_from_monday();
            let monday = today - Duration::days(days_since_mon as i64);
            let s = monday.and_hms_opt(0, 0, 0).unwrap();
            (kst_to_utc(&kst, s), i64::MAX)
        }
        DateFilter::LastWeek => {
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

/// 파일명 필터 적용 (파일명에 지정 텍스트 포함 여부, case-insensitive)
pub fn smart_apply_filename_filter(r: &SearchResult, filename: &Option<String>) -> bool {
    let Some(filter) = filename else { return true };
    let name_lower = r.file_name.to_lowercase();
    let filter_lower = filter.to_lowercase();
    name_lower.contains(&filter_lower)
}

/// 파일 타입 필터 적용
pub fn smart_apply_file_type_filter(r: &SearchResult, ft: &Option<String>) -> bool {
    let Some(ft) = ft else { return true };
    r.file_name.to_lowercase().ends_with(&format!(".{}", ft))
}

/// 제외 키워드 필터 적용
pub fn smart_apply_exclude_filter(r: &SearchResult, exclude: &[String]) -> bool {
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

/// "제N조" 패턴 카운트 (법령 분류용)
pub fn count_article_pattern(text: &str) -> usize {
    let mut count = 0;
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;
    while i < len {
        if chars[i] == '제' {
            let mut j = i + 1;
            while j < len && chars[j].is_whitespace() {
                j += 1;
            }
            let num_start = j;
            while j < len && chars[j].is_ascii_digit() {
                j += 1;
            }
            if j > num_start {
                while j < len && chars[j].is_whitespace() {
                    j += 1;
                }
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

// ── Tests ─────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_article_pattern_works() {
        assert_eq!(count_article_pattern("제1조 제2조 제10조"), 3);
        assert_eq!(count_article_pattern("제 750 조 내용"), 1);
        assert_eq!(count_article_pattern("제출 기한 제한"), 0);
        assert_eq!(count_article_pattern("보험료 22150.00"), 0);
    }
}
