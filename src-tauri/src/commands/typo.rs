//! 한국어 오타 교정 — 자모 분해 기반 edit distance

use crate::db;
use crate::error::{ApiError, ApiResult};
use crate::AppContainer;
use serde::Serialize;
use std::sync::RwLock;
use tauri::State;

/// 교정 제안 결과
#[derive(Debug, Serialize)]
pub struct CorrectionSuggestion {
    pub original: String,
    pub suggestions: Vec<SuggestedWord>,
}

#[derive(Debug, Serialize)]
pub struct SuggestedWord {
    pub word: String,
    pub distance: usize,
    pub frequency: i64,
}

// ==================== 한국어 자모 분해 ====================

const CHOSEONG: [char; 19] = [
    'ㄱ', 'ㄲ', 'ㄴ', 'ㄷ', 'ㄸ', 'ㄹ', 'ㅁ', 'ㅂ', 'ㅃ', 'ㅅ',
    'ㅆ', 'ㅇ', 'ㅈ', 'ㅉ', 'ㅊ', 'ㅋ', 'ㅌ', 'ㅍ', 'ㅎ',
];

const JUNGSEONG: [char; 21] = [
    'ㅏ', 'ㅐ', 'ㅑ', 'ㅒ', 'ㅓ', 'ㅔ', 'ㅕ', 'ㅖ', 'ㅗ', 'ㅘ',
    'ㅙ', 'ㅚ', 'ㅛ', 'ㅜ', 'ㅝ', 'ㅞ', 'ㅟ', 'ㅠ', 'ㅡ', 'ㅢ', 'ㅣ',
];

const JONGSEONG: [char; 28] = [
    '\0', 'ㄱ', 'ㄲ', 'ㄳ', 'ㄴ', 'ㄵ', 'ㄶ', 'ㄷ', 'ㄹ', 'ㄺ',
    'ㄻ', 'ㄼ', 'ㄽ', 'ㄾ', 'ㄿ', 'ㅀ', 'ㅁ', 'ㅂ', 'ㅄ', 'ㅅ',
    'ㅆ', 'ㅇ', 'ㅈ', 'ㅊ', 'ㅋ', 'ㅌ', 'ㅍ', 'ㅎ',
];

/// 한글 음절을 자모로 분해
fn decompose_korean(text: &str) -> Vec<char> {
    let mut result = Vec::with_capacity(text.len() * 3);
    for ch in text.chars() {
        let code = ch as u32;
        if (0xAC00..=0xD7A3).contains(&code) {
            let offset = code - 0xAC00;
            let cho = (offset / 588) as usize;
            let jung = ((offset % 588) / 28) as usize;
            let jong = (offset % 28) as usize;
            result.push(CHOSEONG[cho]);
            result.push(JUNGSEONG[jung]);
            if jong > 0 {
                result.push(JONGSEONG[jong]);
            }
        } else {
            result.push(ch);
        }
    }
    result
}

/// 자모 기반 edit distance (Levenshtein)
fn jamo_distance(a: &str, b: &str) -> usize {
    let a_jamo = decompose_korean(a);
    let b_jamo = decompose_korean(b);
    levenshtein(&a_jamo, &b_jamo)
}

fn levenshtein(a: &[char], b: &[char]) -> usize {
    let m = a.len();
    let n = b.len();
    let mut prev = vec![0usize; n + 1];
    let mut curr = vec![0usize; n + 1];

    for j in 0..=n {
        prev[j] = j;
    }

    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1)
                .min(curr[j - 1] + 1)
                .min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[n]
}

/// 인접 키보드 자모 매핑 (두벌식)
fn is_adjacent_key(a: char, b: char) -> bool {
    // 두벌식 키보드 인접 키 매핑 (주요 오타 패턴)
    const ADJACENT: &[(char, char)] = &[
        ('ㄱ', 'ㄲ'), ('ㄷ', 'ㄸ'), ('ㅂ', 'ㅃ'), ('ㅅ', 'ㅆ'), ('ㅈ', 'ㅉ'),
        ('ㅏ', 'ㅑ'), ('ㅓ', 'ㅕ'), ('ㅗ', 'ㅛ'), ('ㅜ', 'ㅠ'),
        ('ㅐ', 'ㅔ'), ('ㅒ', 'ㅖ'),
        // 자주 혼동되는 자모
        ('ㄱ', 'ㅋ'), ('ㄷ', 'ㅌ'), ('ㅂ', 'ㅍ'), ('ㅅ', 'ㅎ'),
        ('ㅗ', 'ㅓ'), ('ㅜ', 'ㅡ'), ('ㅡ', 'ㅣ'),
    ];
    ADJACENT.iter().any(|&(x, y)| (x == a && y == b) || (x == b && y == a))
}

/// 가중 edit distance (인접 키보드 오타는 비용 감소)
fn weighted_jamo_distance(a: &str, b: &str) -> f64 {
    let a_jamo = decompose_korean(a);
    let b_jamo = decompose_korean(b);

    let m = a_jamo.len();
    let n = b_jamo.len();
    let mut prev = vec![0.0f64; n + 1];
    let mut curr = vec![0.0f64; n + 1];

    for j in 0..=n {
        prev[j] = j as f64;
    }

    for i in 1..=m {
        curr[0] = i as f64;
        for j in 1..=n {
            let cost = if a_jamo[i - 1] == b_jamo[j - 1] {
                0.0
            } else if is_adjacent_key(a_jamo[i - 1], b_jamo[j - 1]) {
                0.5 // 인접 키보드 오타는 반값
            } else {
                1.0
            };
            curr[j] = (prev[j] + 1.0)
                .min(curr[j - 1] + 1.0)
                .min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[n]
}

/// 오타 교정 제안
#[tauri::command]
pub async fn suggest_correction(
    query: String,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<CorrectionSuggestion> {
    let query = query.trim().to_lowercase();
    if query.is_empty() || query.len() > 100 {
        return Ok(CorrectionSuggestion {
            original: query,
            suggestions: vec![],
        });
    }

    let db_path = {
        let container = state.read()?;
        container.db_path.to_string_lossy().to_string()
    };

    let q = query.clone();
    tokio::task::spawn_blocking(move || -> ApiResult<CorrectionSuggestion> {
        let conn = db::get_connection(std::path::Path::new(&db_path))?;

        // 정확히 일치하는 단어가 vocab에 있으면 교정 불필요
        let exact: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM chunks_fts_vocab WHERE term = ?1",
                rusqlite::params![q],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if exact > 0 {
            return Ok(CorrectionSuggestion {
                original: q,
                suggestions: vec![],
            });
        }

        // 단어별 분리하여 각각 교정
        let words: Vec<&str> = q.split_whitespace().collect();
        let mut all_suggestions = Vec::new();

        for word in &words {
            // 빈도 높은 vocab 단어 중 길이 비슷한 것만 후보
            let word_len = word.chars().count();
            let min_len = if word_len > 1 { word_len - 1 } else { 1 };
            let max_len = word_len + 2;

            let mut stmt = conn
                .prepare(
                    "SELECT term, doc FROM chunks_fts_vocab
                     WHERE length(term) BETWEEN ?1 AND ?2
                     ORDER BY doc DESC
                     LIMIT 500",
                )
                .map_err(|e| ApiError::DatabaseQuery(e.to_string()))?;

            let candidates: Vec<(String, i64)> = stmt
                .query_map(
                    rusqlite::params![min_len as i64, max_len as i64],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
                )
                .map_err(|e| ApiError::DatabaseQuery(e.to_string()))?
                .filter_map(|r| r.ok())
                .collect();

            // 가중 edit distance 계산 후 상위 후보 선택
            let jamo_len = decompose_korean(word).len();
            let max_dist = (jamo_len as f64 * 0.4).max(2.0).min(5.0); // 자모 길이의 40%까지

            let mut scored: Vec<(String, f64, i64)> = candidates
                .into_iter()
                .filter_map(|(term, freq)| {
                    let dist = weighted_jamo_distance(word, &term);
                    if dist > 0.0 && dist <= max_dist {
                        Some((term, dist, freq))
                    } else {
                        None
                    }
                })
                .collect();

            // distance 우선, 같으면 빈도 높은 순
            scored.sort_by(|a, b| {
                a.1.partial_cmp(&b.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(b.2.cmp(&a.2))
            });

            for (term, dist, freq) in scored.into_iter().take(3) {
                all_suggestions.push(SuggestedWord {
                    word: term,
                    distance: dist.ceil() as usize,
                    frequency: freq,
                });
            }
        }

        // 전체 쿼리 교정 (단어별 최선 후보 조합)
        if all_suggestions.is_empty() && words.len() == 1 {
            // 단일 단어인데 후보 없으면 search_queries에서도 찾기
            let mut stmt2 = conn
                .prepare(
                    "SELECT query, frequency FROM search_queries
                     WHERE query != ?1
                     ORDER BY frequency DESC
                     LIMIT 100",
                )
                .map_err(|e| ApiError::DatabaseQuery(e.to_string()))?;

            let history: Vec<(String, i64)> = stmt2
                .query_map(rusqlite::params![q], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
                })
                .map_err(|e| ApiError::DatabaseQuery(e.to_string()))?
                .filter_map(|r| r.ok())
                .collect();

            for (hq, freq) in history {
                let dist = jamo_distance(&q, &hq);
                if dist > 0 && dist <= 3 {
                    all_suggestions.push(SuggestedWord {
                        word: hq,
                        distance: dist,
                        frequency: freq,
                    });
                }
            }
            all_suggestions.sort_by(|a, b| a.distance.cmp(&b.distance).then(b.frequency.cmp(&a.frequency)));
            all_suggestions.truncate(3);
        }

        Ok(CorrectionSuggestion {
            original: q,
            suggestions: all_suggestions,
        })
    })
    .await?
}
