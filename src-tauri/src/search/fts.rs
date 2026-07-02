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
    /// 경로 부분 일치 토큰 (소문자, `path:` 연산자). 각 토큰이 모두 포함돼야 함.
    pub path_contains: Vec<String>,
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
    for token in &filter.path_contains {
        // 경로 비교는 scope 패턴과 동일하게 `/` 통일 + 소문자.
        // LIKE 와일드카드(% _)는 이스케이프해 리터럴 매칭.
        let escaped = token
            .replace('\\', "/")
            .replace('%', "\\%")
            .replace('_', "\\_");
        clause.push_str(" AND REPLACE(LOWER(f.path), '\\', '/') LIKE ? ESCAPE '\\'");
        params.push(Box::new(format!("%{}%", escaped)));
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
    // 인덱싱 텍스트(parse_file → normalize_text)와 동일 정규화를 거쳐야
    // NFD 한글·비가시 문자가 섞인 쿼리도 매칭된다. 한쪽만 정규화하면 불일치.
    let normalized = crate::utils::normalize_text(query);
    let trimmed = normalized.trim();
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

/// 인라인 연산자 검색 (v3.0.0) — `OperatorQuery` 를 FTS5 MATCH 식으로 합성해 검색.
///
/// 검색어 없이 필터만 있는 질의(`ext:hwp path:2024` 등)는 메타 필터 브라우즈로
/// 폴백한다 (최근 수정순). 제외어만 있는 질의는 빈 결과.
pub fn search_with_operators(
    conn: &Connection,
    op: &crate::search::query_syntax::OperatorQuery,
    limit: usize,
    tokenizer: Option<&dyn TextTokenizer>,
    folder_scope: Option<&str>,
    mode: KeywordMode,
    filter: &MetaFilter,
) -> Result<Vec<FtsResult>, rusqlite::Error> {
    let safe_query = compose_fts_match(op, tokenizer, mode);
    if safe_query.is_empty() {
        let has_meta = filter.date_range.is_some()
            || !filter.file_types.is_empty()
            || !filter.path_contains.is_empty();
        if has_meta {
            return browse_by_meta(conn, limit, folder_scope, filter);
        }
        return Ok(vec![]);
    }
    search_internal(conn, &safe_query, limit, folder_scope, filter)
}

/// OperatorQuery → FTS5 MATCH 식 합성.
///
/// - 일반 검색어: 기존 sanitize 경로 (형태소 분석 + mode). `~N` 이 있으면 NEAR 식.
/// - `"구문"`: 인접 토큰 매칭 (인덱스가 원본 어절 순서를 보존하므로 동작)
/// - `-제외어`: FTS5 NOT 체인. NOT 이 AND 보다 우선순위가 높아 전체를 괄호로 묶는다.
pub fn compose_fts_match(
    op: &crate::search::query_syntax::OperatorQuery,
    tokenizer: Option<&dyn TextTokenizer>,
    mode: KeywordMode,
) -> String {
    let mut positives: Vec<String> = Vec::new();

    let terms = op.terms.trim();
    if !terms.is_empty() {
        let near_expr = op.near.and_then(|n| compose_near_expr(terms, tokenizer, n));
        match near_expr {
            Some(e) => positives.push(e),
            None => {
                let s = sanitize_fts_query(terms, tokenizer, mode);
                if !s.is_empty() {
                    positives.push(s);
                }
            }
        }
    }

    for p in &op.phrases {
        if let Some(expr) = phrase_match_expr(p) {
            positives.push(expr);
        }
    }

    if positives.is_empty() {
        return String::new();
    }

    // AND 결합 — OR 모드 출력이 섞여도 우선순위가 오염되지 않게 각 항을 괄호로
    let mut expr = if positives.len() == 1 {
        positives.remove(0)
    } else {
        positives
            .into_iter()
            .map(|p| format!("({})", p))
            .collect::<Vec<_>>()
            .join(" AND ")
    };

    for e in &op.excludes {
        if let Some(ex) = phrase_match_expr(e) {
            expr = format!("({}) NOT {}", expr, ex);
        }
    }

    expr
}

/// 구문 → FTS5 phrase 식. 인덱스 측 clean_token 과 동일 기준(ASCII 영숫자+한글)으로
/// 단어를 정제해 `"단어1 단어2"` 인접 매칭을 만든다. 전부 정제되면 None.
fn phrase_match_expr(phrase: &str) -> Option<String> {
    let normalized = crate::utils::normalize_text(phrase);
    let words: Vec<String> = normalized
        .split_whitespace()
        .map(clean_query_token)
        .filter(|w| !w.is_empty())
        .collect();
    if words.is_empty() {
        return None;
    }
    Some(format!("\"{}\"", words.join(" ")))
}

/// 인덱스 측 LinderaKoTokenizer::clean_token 과 동일 기준 (의도적 미러 — 변경 시 동기화)
fn clean_query_token(token: &str) -> String {
    token
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || ('\u{AC00}'..='\u{D7AF}').contains(c))
        .collect()
}

/// `~N` 근접 검색: 어절별 대표 토큰(명사 우선 — "예산이"→"예산")으로 NEAR 식 합성.
/// 어절이 2개 미만이면 None → 일반 검색으로 폴백.
fn compose_near_expr(
    terms: &str,
    tokenizer: Option<&dyn TextTokenizer>,
    distance: u32,
) -> Option<String> {
    let normalized = crate::utils::normalize_text(terms);
    let mut tokens: Vec<String> = Vec::new();
    for word in normalized.split_whitespace() {
        let noun = tokenizer.and_then(|t| t.extract_nouns(word).into_iter().next());
        let token = clean_query_token(&noun.unwrap_or_else(|| word.to_string()));
        if !token.is_empty() {
            tokens.push(token);
        }
    }
    if tokens.len() < 2 {
        return None;
    }
    let phrases: Vec<String> = tokens.iter().map(|t| format!("\"{}\"*", t)).collect();
    Some(format!("NEAR({}, {})", phrases.join(" "), distance))
}

/// 검색어 없는 필터 전용 질의 — 파일당 첫 청크를 최근 수정순으로 반환.
fn browse_by_meta(
    conn: &Connection,
    limit: usize,
    folder_scope: Option<&str>,
    filter: &MetaFilter,
) -> Result<Vec<FtsResult>, rusqlite::Error> {
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
         WHERE c.chunk_index = 0
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

    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
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
    // 인덱싱 content(정규화됨)와 매칭되도록 쿼리도 동일 정규화 (sanitize_fts_query 와 동일 기준).
    // 사용자 입력의 %/_ 는 와일드카드가 아니라 리터럴로 — `_` 검색이 전 청크에
    // 매칭되는 오탐 방지. ESCAPE 절은 아래 SQL의 `LIKE ? ESCAPE '\'`.
    let normalized = crate::utils::normalize_text(query);
    let like_pattern = format!("%{}%", crate::db::escape_like_pattern(normalized.trim()));

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
         WHERE c.content LIKE ? ESCAPE '\\'
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
