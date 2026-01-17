use rusqlite::{Connection, params};

/// FTS5 키워드 검색 (파일 정보 포함)
/// snippet()으로 매칭 컨텍스트 자동 추출
pub fn search(conn: &Connection, query: &str, limit: usize) -> Result<Vec<FtsResult>, rusqlite::Error> {
    // FTS5 쿼리 전처리 (특수문자 이스케이프)
    let safe_query = sanitize_fts_query(query);

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
            snippet(chunks_fts, 0, '[[HL]]', '[[/HL]]', '...', 32) as snippet
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
        })
    })?;

    results.collect()
}

/// FTS5 쿼리 전처리 (특수문자 처리 + prefix match)
fn sanitize_fts_query(query: &str) -> String {
    // 빈 쿼리 처리
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // 각 단어를 쌍따옴표로 감싸고 와일드카드 추가 (prefix match)
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
pub fn find_highlight_ranges(content: &str, query: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let query_lower = query.to_lowercase();
    let content_lower = content.to_lowercase();

    // 문자 단위로 변환
    let content_chars: Vec<char> = content_lower.chars().collect();
    let query_chars: Vec<char> = query_lower.chars().collect();

    if query_chars.is_empty() {
        return ranges;
    }

    let query_len = query_chars.len();
    let content_len = content_chars.len();

    let mut i = 0;
    while i + query_len <= content_len {
        if content_chars[i..i + query_len] == query_chars[..] {
            ranges.push((i, i + query_len));
            i += query_len; // 다음 검색은 매칭 끝에서
        } else {
            i += 1;
        }
    }

    ranges
}

#[derive(Debug, Clone)]
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
}
