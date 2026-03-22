use crate::{db, ApiError, ApiResult};
use chrono::{NaiveDate, Local};
use serde::Serialize;
use std::sync::RwLock;
use tauri::State;
use crate::application::container::AppContainer;

#[derive(Debug, Serialize, Clone)]
pub struct ExpiryDocument {
    pub file_path: String,
    pub file_name: String,
    pub expiry_date: String,
    pub days_remaining: i64,
    pub context: String,
    pub urgency: String,
}

#[derive(Debug, Serialize)]
pub struct ExpiryResponse {
    pub documents: Vec<ExpiryDocument>,
    pub scan_time_ms: u64,
    pub total_scanned: usize,
}

/// 만료 임박 문서 스캔
#[tauri::command]
pub async fn scan_expiry_dates(
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<ExpiryResponse> {
    let start = std::time::Instant::now();

    let db_path = {
        let container = state
            .read()
            .map_err(|_| ApiError::IndexingFailed("Lock error".into()))?;
        container.db_path.clone()
    };

    let (documents, total) = tokio::task::spawn_blocking(move || {
        scan_documents_for_expiry(&db_path)
    })
    .await
    .map_err(|e| ApiError::IndexingFailed(e.to_string()))??;

    Ok(ExpiryResponse {
        documents,
        scan_time_ms: start.elapsed().as_millis() as u64,
        total_scanned: total,
    })
}

fn scan_documents_for_expiry(
    db_path: &std::path::Path,
) -> ApiResult<(Vec<ExpiryDocument>, usize)> {
    let conn =
        db::get_connection(db_path).map_err(|e| ApiError::IndexingFailed(e.to_string()))?;

    let mut stmt = conn
        .prepare(
            "SELECT f.path, f.name, c.content
             FROM files f
             JOIN chunks c ON c.file_id = f.id AND c.chunk_index = 0
             ORDER BY f.path
             LIMIT 50000",
        )
        .map_err(|e| ApiError::IndexingFailed(e.to_string()))?;

    let docs: Vec<(String, String, String)> = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| ApiError::IndexingFailed(e.to_string()))?
        .filter_map(|r| r.ok())
        .collect();

    let total = docs.len();
    let today = Local::now().date_naive();
    let mut results: Vec<ExpiryDocument> = Vec::new();

    for (path, name, content) in &docs {
        if let Some(expiry) = extract_expiry_date(content, today) {
            results.push(ExpiryDocument {
                file_path: path.clone(),
                file_name: name.clone(),
                expiry_date: expiry.date.format("%Y-%m-%d").to_string(),
                days_remaining: expiry.days_remaining,
                context: expiry.context,
                urgency: classify_urgency(expiry.days_remaining),
            });
        }
    }

    results.sort_by_key(|d| d.days_remaining);
    Ok((results, total))
}

struct ExtractedExpiry {
    date: NaiveDate,
    days_remaining: i64,
    context: String,
}

const EXPIRY_KEYWORDS: &[&str] = &[
    "만료", "유효기간", "까지", "종료", "기한", "시한",
    "계약기간", "유효", "만기", "폐기",
];

/// 문서 내용에서 만료 관련 날짜 추출 (regex 없이 직접 파싱)
fn extract_expiry_date(content: &str, today: NaiveDate) -> Option<ExtractedExpiry> {
    let mut best: Option<ExtractedExpiry> = None;
    let mut best_distance = i64::MAX;

    let chars: Vec<char> = content.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // 4자리 숫자(년도) 시작점 탐색
        if chars[i].is_ascii_digit() {
            if let Some((date, end_pos)) = try_parse_date(&chars, i) {
                let days_remaining = (date - today).num_days();

                // 범위 필터: 3년 전 ~ 5년 후
                if days_remaining >= -365 * 3 && days_remaining <= 365 * 5 {
                    // 주변 텍스트 추출 (±50 chars)
                    let ctx_start = i.saturating_sub(50);
                    let ctx_end = (end_pos + 50).min(len);
                    let context: String = chars[ctx_start..ctx_end].iter().collect();
                    let context = context.replace('\n', " ").replace('\r', "");

                    // 만료 키워드 확인
                    if EXPIRY_KEYWORDS.iter().any(|kw| context.contains(kw)) {
                        let distance = days_remaining.abs();
                        if distance < best_distance {
                            best_distance = distance;
                            best = Some(ExtractedExpiry {
                                date,
                                days_remaining,
                                context: truncate_str(&context, 100),
                            });
                        }
                    }
                }

                i = end_pos;
                continue;
            }
        }
        i += 1;
    }

    best
}

/// chars[pos..]에서 날짜 파싱 시도. 성공 시 (NaiveDate, end_pos) 반환
fn try_parse_date(chars: &[char], pos: usize) -> Option<(NaiveDate, usize)> {
    let len = chars.len();
    if pos + 7 > len {
        return None;
    }

    // 4자리 숫자 (년도)
    let year_str: String = chars[pos..pos + 4].iter().collect();
    let year: i32 = year_str.parse().ok()?;
    if !(1990..=2040).contains(&year) {
        return None;
    }

    let mut cur = pos + 4;

    // 구분자/키워드: 년, ., -, /
    let (sep_type, after_sep) = parse_separator(chars, cur)?;
    cur = after_sep;

    // 월 (1-2자리)
    let (month, after_month) = parse_number(chars, cur, 1, 2)?;
    if !(1..=12).contains(&month) {
        return None;
    }
    cur = after_month;

    // 구분자: 월, ., -, /
    let (sep2, after_sep2) = parse_separator(chars, cur)?;
    // 구분자 일관성 체크 (년.월.일 or 년-월-일, 혼용 허용)
    if sep_type == SepType::Korean && sep2 != SepType::Korean {
        return None;
    }
    cur = after_sep2;

    // 일 (1-2자리)
    let (day, after_day) = parse_number(chars, cur, 1, 2)?;
    if !(1..=31).contains(&day) {
        return None;
    }
    cur = after_day;

    // 선택적 '일' 키워드
    if cur < len && chars[cur] == '일' {
        cur += 1;
    }

    NaiveDate::from_ymd_opt(year, month, day).map(|d| (d, cur))
}

#[derive(PartialEq)]
enum SepType {
    Korean,  // 년, 월
    Punct,   // . - /
}

fn parse_separator(chars: &[char], pos: usize) -> Option<(SepType, usize)> {
    if pos >= chars.len() {
        return None;
    }

    let mut cur = pos;

    // 공백 스킵
    while cur < chars.len() && chars[cur] == ' ' {
        cur += 1;
    }

    if cur >= chars.len() {
        return None;
    }

    match chars[cur] {
        '년' | '월' => Some((SepType::Korean, cur + 1)),
        '.' | '-' | '/' => Some((SepType::Punct, cur + 1)),
        _ => {
            // 공백만 있고 구분자가 없으면 Korean 패턴 (이미 "년" 처리됨) 확인
            if cur > pos {
                // 숫자가 바로 오면 공백 구분으로 간주
                if chars[cur].is_ascii_digit() {
                    return Some((SepType::Korean, cur));
                }
            }
            None
        }
    }
}

fn parse_number(chars: &[char], pos: usize, min_digits: usize, max_digits: usize) -> Option<(u32, usize)> {
    // 공백 스킵
    let mut cur = pos;
    while cur < chars.len() && chars[cur] == ' ' {
        cur += 1;
    }

    let start = cur;
    while cur < chars.len() && chars[cur].is_ascii_digit() && cur - start < max_digits {
        cur += 1;
    }

    let digit_count = cur - start;
    if digit_count < min_digits {
        return None;
    }

    let num_str: String = chars[start..cur].iter().collect();
    num_str.parse().ok().map(|n| (n, cur))
}

fn classify_urgency(days: i64) -> String {
    if days < 0 {
        "expired".into()
    } else if days <= 7 {
        "urgent".into()
    } else if days <= 30 {
        "warning".into()
    } else {
        "normal".into()
    }
}

fn truncate_str(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        format!("{}...", s.chars().take(max_chars).collect::<String>())
    }
}
