use rusqlite::{Connection, params};

/// 파일명 FTS5 검색
pub fn search(conn: &Connection, query: &str, limit: usize) -> Result<Vec<FilenameResult>, rusqlite::Error> {
    let safe_query = sanitize_fts_query(query);

    if safe_query.is_empty() {
        return Ok(vec![]);
    }

    let mut stmt = conn.prepare(
        "SELECT
            f.id,
            f.path,
            f.name,
            f.file_type,
            f.size,
            f.modified_at,
            bm25(files_fts) as score,
            highlight(files_fts, 0, '[[HL]]', '[[/HL]]') as highlight
         FROM files_fts fts
         JOIN files f ON f.id = fts.rowid
         WHERE files_fts MATCH ?
         ORDER BY score
         LIMIT ?"
    )?;

    let results = stmt.query_map(params![safe_query, limit as i64], |row| {
        Ok(FilenameResult {
            file_id: row.get(0)?,
            file_path: row.get(1)?,
            file_name: row.get(2)?,
            file_type: row.get(3)?,
            size: row.get(4)?,
            modified_at: row.get(5)?,
            score: row.get(6)?,
            highlight: row.get(7)?,
        })
    })?;

    results.collect()
}

/// FTS5 쿼리 전처리 (특수문자 처리 + prefix match)
fn sanitize_fts_query(query: &str) -> String {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // 각 단어를 쌍따옴표로 감싸고 와일드카드 추가 (prefix match)
    let terms: Vec<String> = trimmed
        .split_whitespace()
        .map(|word| {
            let escaped = word.replace('"', "\"\"");
            format!("\"{}\"*", escaped)
        })
        .collect();

    terms.join(" ")
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct FilenameResult {
    pub file_id: i64,
    pub file_path: String,
    pub file_name: String,
    pub file_type: String,
    pub size: Option<i64>,
    pub modified_at: Option<i64>,
    pub score: f64,
    /// FTS5 highlight() - 파일명에 하이라이트 마커 포함
    pub highlight: String,
}
