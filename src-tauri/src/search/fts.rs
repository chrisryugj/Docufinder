use crate::search::KeywordMode;
use crate::tokenizer::TextTokenizer;
use rusqlite::Connection;

/// FTS 검색에 함께 적용할 메타데이터 필터 (날짜·파일타입).
///
/// `chunks_fts MATCH` 절에 SQL WHERE 로 합성되어, 키워드 검색 단계에서
/// 날짜/타입까지 좁힌다. 비어 있으면(Default) 아무 제약도 추가하지 않아
/// 기존 동작과 동일하다. (키워드+필터 질의가 BM25 상위 N 밖으로 밀려
/// 누락되던 문제를 막기 위함.)
#[derive(Debug, Clone, Default)]
pub struct MetaFilter {
    /// (start, end) modified_at Unix timestamp 범위 (inclusive)
    pub date_range: Option<(i64, i64)>,
    /// 매칭할 확장자 목록 (소문자, 점 없이). 비면 타입 제약 없음.
    pub file_types: Vec<String>,
}

/// MetaFilter → (SQL 절, 바인드 파라미터). `f` 별칭의 files 테이블을 전제.
/// 절은 항상 " AND ..." 로 시작하므로 기존 WHERE 뒤에 그대로 이어 붙인다.
fn build_meta_clause(filter: &MetaFilter) -> (String, Vec<Box<dyn rusqlite::types::ToSql>>) {
    let mut clause = String::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some((start, end)) = filter.date_range {
        clause.push_str(" AND f.modified_at >= ? AND f.modified_at <= ?");
        params.push(Box::new(start));
        params.push(Box::new(end));
    }
    if !filter.file_types.is_empty() {
        let placeholders = vec!["?"; filter.file_types.len()].join(", ");
        clause.push_str(&format!(" AND LOWER(f.file_type) IN ({})", placeholders));
        for ft in &filter.file_types {
            params.push(Box::new(ft.clone()));
        }
    }

    (clause, params)
}

/// FTS5 키워드 검색 (파일 정보 포함)
/// snippet()으로 매칭 컨텍스트 자동 추출
/// 짧은 한글 쿼리(1~2자)에서 FTS5가 빈 결과 반환 시 LIKE 폴백
pub fn search(
    conn: &Connection,
    query: &str,
    limit: usize,
    folder_scope: Option<&str>,
    mode: KeywordMode,
    filter: &MetaFilter,
) -> Result<Vec<FtsResult>, rusqlite::Error> {
    // FTS5 쿼리 전처리 (특수문자 이스케이프, 토크나이저 미사용)
    let safe_query = sanitize_fts_query(query, None, mode);

    if safe_query.is_empty() {
        return Ok(vec![]);
    }

    let results = search_internal(conn, &safe_query, limit, folder_scope, filter)?;

    // 짧은 쿼리(한글 1~2자)에서 FTS가 빈 결과이면 LIKE 폴백
    if results.is_empty() && query.trim().chars().count() <= 2 {
        return search_like_fallback(conn, query.trim(), limit, folder_scope, filter);
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
    filter: &MetaFilter,
) -> Result<Vec<FtsResult>, rusqlite::Error> {
    // FTS5 쿼리 전처리 (형태소 분석 포함)
    let safe_query = sanitize_fts_query(query, Some(tokenizer), mode);

    if safe_query.is_empty() {
        return Ok(vec![]);
    }

    let results = search_internal(conn, &safe_query, limit, folder_scope, filter)?;

    // 짧은 쿼리에서 FTS가 빈 결과이면 LIKE 폴백
    if results.is_empty() && query.trim().chars().count() <= 2 {
        return search_like_fallback(conn, query.trim(), limit, folder_scope, filter);
    }

    Ok(results)
}

/// FTS5 검색 내부 구현
fn search_internal(
    conn: &Connection,
    safe_query: &str,
    limit: usize,
    folder_scope: Option<&str>,
    filter: &MetaFilter,
) -> Result<Vec<FtsResult>, rusqlite::Error> {
    if safe_query.is_empty() {
        return Ok(vec![]);
    }

    // folder_scope가 있으면 segment 경계(scope/)에서 끊기는 LIKE 패턴 사용.
    // sibling 폴더 오탐 차단을 위해 path 와 pattern 모두 `/` 로 통일 + 소문자 비교.
    let (scope_clause, scope_pattern) =
        match crate::utils::folder_scope::scope_like_pattern(folder_scope.unwrap_or("")) {
            Some(pat) => (
                "AND REPLACE(LOWER(f.path), '\\', '/') LIKE ? ESCAPE '\\'",
                Some(pat),
            ),
            None => ("", None),
        };

    // 날짜·파일타입 메타 필터 (키워드와 함께 SQL 단계에서 좁힘)
    let (meta_clause, mut meta_params) = build_meta_clause(filter);

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
         {scope}{meta}
         ORDER BY score
         LIMIT ?",
        scope = scope_clause,
        meta = meta_clause
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

    // 바인드 순서: MATCH 쿼리 → [scope 패턴] → [meta 파라미터] → limit
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    params.push(Box::new(safe_query.to_string()));
    if let Some(pattern) = scope_pattern {
        params.push(Box::new(pattern));
    }
    params.append(&mut meta_params);
    params.push(Box::new(limit as i64));

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let results: Vec<FtsResult> = stmt
        .query_map(param_refs.as_slice(), map_row)?
        .collect::<Result<Vec<_>, _>>()?;

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

/// 단일 파일 내부 FTS5 검색.
///
/// 전역 top-N 에서 파일 필터를 걸던 기존 방식은 큰 문서에서 질문 관련 청크가
/// 전역 top-N 밖으로 밀려날 수 있다. 이 함수는 처음부터 `f.path = ?` 로 좁혀
/// 해당 파일 내부에서 BM25 상위 limit 만 반환한다.
pub fn search_in_file(
    conn: &Connection,
    query: &str,
    limit: usize,
    file_path: &str,
    tokenizer: Option<&dyn TextTokenizer>,
    mode: KeywordMode,
) -> Result<Vec<FtsResult>, rusqlite::Error> {
    let safe_query = sanitize_fts_query(query, tokenizer, mode);
    if safe_query.is_empty() {
        return Ok(vec![]);
    }

    let sql = "SELECT
            c.id, f.path, f.name, c.chunk_index,
            COALESCE(c.content, fts.content) AS content,
            bm25(chunks_fts) as score,
            c.start_offset, c.end_offset, c.page_number, c.page_end, c.location_hint,
            snippet(chunks_fts, 0, '[[HL]]', '[[/HL]]', '...', 64) as snippet,
            f.modified_at
         FROM chunks_fts fts
         JOIN chunks c ON c.id = fts.rowid
         JOIN files f ON f.id = c.file_id
         WHERE chunks_fts MATCH ? AND f.path = ?
         ORDER BY score
         LIMIT ?";

    let mut stmt = conn.prepare(sql)?;
    let results: Vec<FtsResult> = stmt
        .query_map(
            rusqlite::params![safe_query, file_path, limit as i64],
            |row| {
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
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(results)
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
    filter: &MetaFilter,
) -> Result<Vec<FtsResult>, rusqlite::Error> {
    let like_pattern = format!("%{}%", query);

    let (scope_clause, scope_pattern) =
        match crate::utils::folder_scope::scope_like_pattern(folder_scope.unwrap_or("")) {
            Some(pat) => (
                "AND REPLACE(LOWER(f.path), '\\', '/') LIKE ? ESCAPE '\\'",
                Some(pat),
            ),
            None => ("", None),
        };

    let (meta_clause, mut meta_params) = build_meta_clause(filter);

    let sql = format!(
        "SELECT
            c.id, f.path, f.name, c.chunk_index, c.content,
            1.0 as score, c.start_offset, c.end_offset,
            c.page_number, c.page_end, c.location_hint,
            '' as snippet, f.modified_at
         FROM chunks c
         JOIN files f ON f.id = c.file_id
         WHERE c.content LIKE ?
         {scope}{meta}
         ORDER BY f.modified_at DESC
         LIMIT ?",
        scope = scope_clause,
        meta = meta_clause
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

    // 바인드 순서: LIKE 패턴 → [scope 패턴] → [meta 파라미터] → limit
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    params.push(Box::new(like_pattern));
    if let Some(pattern) = scope_pattern {
        params.push(Box::new(pattern));
    }
    params.append(&mut meta_params);
    params.push(Box::new(limit as i64));

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let results: Vec<FtsResult> = stmt
        .query_map(param_refs.as_slice(), map_row)?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(results)
}
