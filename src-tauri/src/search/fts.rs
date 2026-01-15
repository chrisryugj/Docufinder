use rusqlite::{Connection, params};

/// FTS5 키워드 검색 (파일 정보 포함)
pub fn search(conn: &Connection, query: &str, limit: usize) -> Result<Vec<FtsResult>, rusqlite::Error> {
    // FTS5 쿼리 전처리 (특수문자 이스케이프)
    let safe_query = sanitize_fts_query(query);

    if safe_query.is_empty() {
        return Ok(vec![]);
    }

    let mut stmt = conn.prepare(
        "SELECT
            c.id,
            f.path,
            f.name,
            c.chunk_index,
            fts.content,
            bm25(chunks_fts) as score,
            c.start_offset,
            c.end_offset
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
        })
    })?;

    results.collect()
}

/// FTS5 쿼리 전처리 (특수문자 처리)
fn sanitize_fts_query(query: &str) -> String {
    // 빈 쿼리 처리
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // FTS5 특수문자 이스케이프
    // 쌍따옴표로 감싸서 안전하게 검색
    let escaped = trimmed.replace('"', "\"\"");
    format!("\"{}\"", escaped)
}

/// 하이라이트 범위 계산
pub fn find_highlight_ranges(content: &str, query: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let query_lower = query.to_lowercase();
    let content_lower = content.to_lowercase();

    let mut start = 0;
    while let Some(pos) = content_lower[start..].find(&query_lower) {
        let abs_pos = start + pos;
        ranges.push((abs_pos, abs_pos + query.len()));
        start = abs_pos + 1;
    }

    ranges
}

#[derive(Debug)]
pub struct FtsResult {
    pub chunk_id: i64,
    pub file_path: String,
    pub file_name: String,
    pub chunk_index: i64,
    pub content: String,
    pub score: f64,
    pub start_offset: i64,
    pub end_offset: i64,
}
