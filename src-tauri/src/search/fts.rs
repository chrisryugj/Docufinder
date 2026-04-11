use crate::search::KeywordMode;
use crate::tokenizer::TextTokenizer;
use rusqlite::{params, Connection};

/// FTS5 키워드 검색 (파일 정보 포함)
/// snippet()으로 매칭 컨텍스트 자동 추출
/// 짧은 한글 쿼리(1~2자)에서 FTS5가 빈 결과 반환 시 LIKE 폴백
pub fn search(
    conn: &Connection,
    query: &str,
    limit: usize,
    folder_scope: Option<&str>,
    mode: KeywordMode,
) -> Result<Vec<FtsResult>, rusqlite::Error> {
    // FTS5 쿼리 전처리 (특수문자 이스케이프, 토크나이저 미사용)
    let safe_query = sanitize_fts_query(query, None, mode);

    if safe_query.is_empty() {
        return Ok(vec![]);
    }

    let results = search_internal(conn, &safe_query, limit, folder_scope)?;

    // 짧은 쿼리(한글 1~2자)에서 FTS가 빈 결과이면 LIKE 폴백
    if results.is_empty() && query.trim().chars().count() <= 2 {
        return search_like_fallback(conn, query.trim(), limit, folder_scope);
    }

    Ok(results)
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
    folder_scope: Option<&str>,
    mode: KeywordMode,
) -> Result<Vec<FtsResult>, rusqlite::Error> {
    // FTS5 쿼리 전처리 (형태소 분석 포함)
    let safe_query = sanitize_fts_query(query, Some(tokenizer), mode);

    if safe_query.is_empty() {
        return Ok(vec![]);
    }

    let results = search_internal(conn, &safe_query, limit, folder_scope)?;

    // 짧은 쿼리에서 FTS가 빈 결과이면 LIKE 폴백
    if results.is_empty() && query.trim().chars().count() <= 2 {
        return search_like_fallback(conn, query.trim(), limit, folder_scope);
    }

    Ok(results)
}

/// FTS5 검색 내부 구현
fn search_internal(
    conn: &Connection,
    safe_query: &str,
    limit: usize,
    folder_scope: Option<&str>,
) -> Result<Vec<FtsResult>, rusqlite::Error> {
    if safe_query.is_empty() {
        return Ok(vec![]);
    }

    // folder_scope가 있으면 path LIKE 조건 추가 (Windows: case-insensitive)
    let (scope_clause, scope_pattern) = match folder_scope {
        Some(scope) if !scope.is_empty() => {
            let escaped = crate::db::escape_like_pattern(&scope.to_lowercase());
            (
                "AND LOWER(f.path) LIKE ? ESCAPE '\\'",
                Some(format!("{}%", escaped)),
            )
        }
        _ => ("", None),
    };

    let sql = format!(
        "SELECT
            c.id,
            f.path,
            f.name,
            c.chunk_index,
            COALESCE(c.content, fts.content) AS content,
            bm25(chunks_fts) as score,
            c.start_offset,
            c.end_offset,
            c.page_number,
            c.page_end,
            c.location_hint,
            snippet(chunks_fts, 0, '[[HL]]', '[[/HL]]', '...', 64) as snippet,
            f.modified_at
         FROM chunks_fts fts
         JOIN chunks c ON c.id = fts.rowid
         JOIN files f ON f.id = c.file_id
         WHERE chunks_fts MATCH ?
         {}
         ORDER BY score
         LIMIT ?",
        scope_clause
    );

    let mut stmt = conn.prepare(&sql)?;

    let map_row = |row: &rusqlite::Row| {
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
            page_end: row.get(9)?,
            location_hint: row.get(10)?,
            snippet: row.get(11)?,
            modified_at: row.get(12)?,
        })
    };

    let results: Vec<FtsResult> = if let Some(ref pattern) = scope_pattern {
        let limit_i64 = limit as i64;
        stmt.query_map(rusqlite::params![safe_query, pattern, limit_i64], map_row)?
            .collect::<Result<Vec<_>, _>>()?
    } else {
        stmt.query_map(params![safe_query, limit as i64], map_row)?
            .collect::<Result<Vec<_>, _>>()?
    };

    Ok(results)
}

/// FTS5 쿼리 전처리 (특수문자 처리 + prefix match + 검색 모드)
///
/// tokenizer가 Some이면 한국어 형태소 분석을 수행합니다.
/// mode에 따라 AND, OR, 구문 검색(EXACT)을 생성합니다.
fn sanitize_fts_query(
    query: &str,
    tokenizer: Option<&dyn TextTokenizer>,
    mode: KeywordMode,
) -> String {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // EXACT 모드: 구문 검색 (형태소 분석 없이 따옴표로 감싸기)
    if mode == KeywordMode::Exact {
        let escaped = trimmed.replace('"', "\"\"");
        return format!("\"{}\"", escaped);
    }

    let join_op = match mode {
        KeywordMode::Or => " OR ",
        _ => " AND ",
    };

    // 토크나이저 사용 시 형태소 분석
    if let Some(tok) = tokenizer {
        let result = tok.tokenize_query(trimmed);
        if mode == KeywordMode::Or {
            // 토크나이저 출력에서 어절 간 AND → OR 전환
            // (어절 내 형태소 OR은 유지)
            return result.replace(" AND ", " OR ");
        }
        return result;
    }

    // 기본 처리: 각 단어를 쌍따옴표로 감싸고 와일드카드 추가
    let terms: Vec<String> = trimmed
        .split_whitespace()
        .map(|word| {
            let escaped = word.replace('"', "\"\"");
            format!("\"{}\"*", escaped)
        })
        .collect();

    if terms.len() == 1 {
        return terms[0].clone();
    }

    terms.join(join_op)
}

/// ParsedQuery 기반 FTS5 쿼리 생성 (NOT 연산자 지원)
///
/// exclude가 비어있으면 기존 sanitize_fts_query와 동일.
/// 양의 항 없이 NOT만 있으면 빈 문자열 반환 (후처리 필터로 위임).
#[allow(dead_code)]
pub fn build_fts_query(
    keywords: &str,
    exclude: &[String],
    tokenizer: Option<&dyn TextTokenizer>,
) -> String {
    let positive = sanitize_fts_query(keywords, tokenizer, KeywordMode::And);

    if positive.is_empty() || exclude.is_empty() {
        return positive;
    }

    let not_terms: Vec<String> = exclude
        .iter()
        .map(|t| format!("NOT \"{}\"*", t.replace('"', "\"\"")))
        .collect();

    format!("{} {}", positive, not_terms.join(" "))
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // FTS 결과 필드 (일부만 현재 소비)
pub struct FtsResult {
    pub chunk_id: i64,
    pub file_path: String,
    pub file_name: String,
    pub chunk_index: i64,
    pub content: String,
    pub score: f64,
    pub start_offset: i64,
    pub end_offset: i64,
    /// 페이지 번호 - 청크 시작 페이지 (DOCX/PDF/HWPX)
    pub page_number: Option<i64>,
    /// 청크 끝 페이지
    pub page_end: Option<i64>,
    /// 위치 힌트 (XLSX: "Sheet1!행1-50", PDF: "페이지 3" 등)
    pub location_hint: Option<String>,
    /// FTS5 snippet() - 매칭 컨텍스트 (하이라이트 마커 포함)
    /// [[HL]]매칭[[/HL]] 형식
    pub snippet: String,
    /// 파일 수정 시간 (Unix timestamp, 초)
    pub modified_at: Option<i64>,
}

/// 짧은 쿼리용 LIKE 폴백 검색 (FTS5가 못 잡는 한글 1~2자 대응)
fn search_like_fallback(
    conn: &Connection,
    query: &str,
    limit: usize,
    folder_scope: Option<&str>,
) -> Result<Vec<FtsResult>, rusqlite::Error> {
    let like_pattern = format!("%{}%", query);

    let (scope_clause, scope_pattern) = match folder_scope {
        Some(scope) if !scope.is_empty() => {
            let escaped = crate::db::escape_like_pattern(&scope.to_lowercase());
            (
                "AND LOWER(f.path) LIKE ? ESCAPE '\\'",
                Some(format!("{}%", escaped)),
            )
        }
        _ => ("", None),
    };

    let sql = format!(
        "SELECT
            c.id, f.path, f.name, c.chunk_index, c.content,
            1.0 as score, c.start_offset, c.end_offset,
            c.page_number, c.page_end, c.location_hint,
            '' as snippet, f.modified_at
         FROM chunks c
         JOIN files f ON f.id = c.file_id
         WHERE c.content LIKE ?
         {}
         ORDER BY f.modified_at DESC
         LIMIT ?",
        scope_clause
    );

    let mut stmt = conn.prepare(&sql)?;

    let map_row = |row: &rusqlite::Row| {
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
            page_end: row.get(9)?,
            location_hint: row.get(10)?,
            snippet: row.get(11)?,
            modified_at: row.get(12)?,
        })
    };

    let results: Vec<FtsResult> = if let Some(ref pattern) = scope_pattern {
        stmt.query_map(
            rusqlite::params![like_pattern, pattern, limit as i64],
            map_row,
        )?
        .collect::<Result<Vec<_>, _>>()?
    } else {
        stmt.query_map(params![like_pattern, limit as i64], map_row)?
            .collect::<Result<Vec<_>, _>>()?
    };

    Ok(results)
}
