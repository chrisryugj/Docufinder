use crate::tokenizer::TextTokenizer;
use rusqlite::{Connection, params};

/// FTS5 키워드 검색 (파일 정보 포함)
/// snippet()으로 매칭 컨텍스트 자동 추출
pub fn search(conn: &Connection, query: &str, limit: usize) -> Result<Vec<FtsResult>, rusqlite::Error> {
    // FTS5 쿼리 전처리 (특수문자 이스케이프, 토크나이저 미사용)
    let safe_query = sanitize_fts_query(query, None);

    if safe_query.is_empty() {
        return Ok(vec![]);
    }

    search_internal(conn, &safe_query, limit)
}

/// FTS5 키워드 검색 (한국어 형태소 분석 포함)
///
/// 토크나이저를 사용하여 검색어를 형태소 분석 후 검색합니다.
/// 예: "사용했습니다" → "사용"* "했"* "습니다"*
pub fn search_with_tokenizer(
    conn: &Connection,
    query: &str,
    limit: usize,
    tokenizer: &dyn TextTokenizer,
) -> Result<Vec<FtsResult>, rusqlite::Error> {
    // FTS5 쿼리 전처리 (형태소 분석 포함)
    let safe_query = sanitize_fts_query(query, Some(tokenizer));

    if safe_query.is_empty() {
        return Ok(vec![]);
    }

    search_internal(conn, &safe_query, limit)
}

/// FTS5 검색 내부 구현
fn search_internal(conn: &Connection, safe_query: &str, limit: usize) -> Result<Vec<FtsResult>, rusqlite::Error> {
    let safe_query = safe_query;

    if safe_query.is_empty() {
        return Ok(vec![]);
    }

    // snippet(테이블, 컬럼인덱스, 시작마커, 끝마커, 생략기호, 토큰수)
    // page_number, location_hint 직접 포함 (N+1 쿼리 제거)
    let mut stmt = conn.prepare(
        "SELECT
            c.id,
            f.path,
            f.name,
            c.chunk_index,
            fts.content,
            bm25(chunks_fts) as score,
            c.start_offset,
            c.end_offset,
            c.page_number,
            c.location_hint,
            snippet(chunks_fts, 0, '[[HL]]', '[[/HL]]', '...', 32) as snippet,
            highlight(chunks_fts, 0, '[[HL]]', '[[/HL]]') as highlight,
            f.modified_at
         FROM chunks_fts fts
         JOIN chunks c ON c.id = fts.rowid
         JOIN files f ON f.id = c.file_id
         WHERE chunks_fts MATCH ?
         ORDER BY score
         LIMIT ?"
    )?;

    let results = stmt.query_map(params![safe_query, limit as i64], |row| {
        Ok(FtsResult {
            chunk_id: row.get(0)?,
            file_path: row.get(1)?,
            file_name: row.get(2)?,
            chunk_index: row.get(3)?,
            content: row.get(4)?,
            score: row.get(5)?,
            start_offset: row.get(6)?,
            end_offset: row.get(7)?,
            page_number: row.get(8)?,
            location_hint: row.get(9)?,
            snippet: row.get(10)?,
            highlight: row.get(11)?,
            modified_at: row.get(12)?,
        })
    })?;

    results.collect()
}

/// FTS5 쿼리 전처리 (특수문자 처리 + prefix match)
///
/// tokenizer가 Some이면 한국어 형태소 분석을 수행합니다.
fn sanitize_fts_query(query: &str, tokenizer: Option<&dyn TextTokenizer>) -> String {
    // 빈 쿼리 처리
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // 토크나이저가 있으면 형태소 분석 사용
    if let Some(tok) = tokenizer {
        return tok.tokenize_query(trimmed);
    }

    // 기본 처리: 각 단어를 쌍따옴표로 감싸고 와일드카드 추가 (prefix match)
    // "분장"* → "분장", "분장을", "분장이" 등 모두 매칭
    let terms: Vec<String> = trimmed
        .split_whitespace()
        .map(|word| {
            let escaped = word.replace('"', "\"\"");
            format!("\"{}\"*", escaped)
        })
        .collect();

    terms.join(" ")
}

/// 하이라이트 범위 계산 (문자 인덱스 반환, JavaScript 호환)
#[allow(dead_code)]
pub fn find_highlight_ranges(content: &str, query: &str) -> Vec<(usize, usize)> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return vec![];
    }

    let mut terms: Vec<String> = trimmed
        .split_whitespace()
        .map(|term| term.trim_matches('"').trim_matches('\''))
        .filter(|term| !term.is_empty())
        .map(|term| term.to_lowercase())
        .collect();

    if terms.is_empty() {
        return vec![];
    }

    terms.sort();
    terms.dedup();

    let content_lower = content.to_lowercase();
    let content_chars: Vec<char> = content_lower.chars().collect();
    let content_len = content_chars.len();
    let mut ranges: Vec<(usize, usize)> = Vec::new();

    for term in terms {
        let term_chars: Vec<char> = term.chars().collect();
        if term_chars.is_empty() {
            continue;
        }

        let term_len = term_chars.len();
        if term_len > content_len {
            continue;
        }

        let mut i = 0;
        while i + term_len <= content_len {
            if content_chars[i..i + term_len] == term_chars[..] {
                ranges.push((i, i + term_len));
                i += term_len;
            } else {
                i += 1;
            }
        }
    }

    if ranges.is_empty() {
        return ranges;
    }

    ranges.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

    let mut merged = Vec::with_capacity(ranges.len());
    let mut current = ranges[0];
    for range in ranges.into_iter().skip(1) {
        if range.0 <= current.1 {
            if range.1 > current.1 {
                current.1 = range.1;
            }
        } else {
            merged.push(current);
            current = range;
        }
    }
    merged.push(current);

    merged
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct FtsResult {
    pub chunk_id: i64,
    pub file_path: String,
    pub file_name: String,
    pub chunk_index: i64,
    pub content: String,
    pub score: f64,
    pub start_offset: i64,
    pub end_offset: i64,
    /// 페이지 번호 (DOCX/PDF/HWPX)
    pub page_number: Option<i64>,
    /// 위치 힌트 (XLSX: "Sheet1!행1-50", PDF: "페이지 3" 등)
    pub location_hint: Option<String>,
    /// FTS5 snippet() - 매칭 컨텍스트 (하이라이트 마커 포함)
    /// [[HL]]매칭[[/HL]] 형식
    pub snippet: String,
    /// FTS5 highlight() - 전체 컨텐츠에 하이라이트 마커 포함
    /// [[HL]]매칭[[/HL]] 형식
    pub highlight: String,
    /// 파일 수정 시간 (Unix timestamp, 초)
    pub modified_at: Option<i64>,
}
