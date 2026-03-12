use rusqlite::Connection;

/// LIKE 패턴 특수문자 이스케이프 (%, _, \ 처리)
fn escape_like_pattern(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// 파일명 검색 (LIKE 기반 - 부분문자열 매칭 지원)
/// FTS5 unicode61은 한글 부분문자열 매칭이 안 되므로 LIKE 사용
pub fn search(
    conn: &Connection,
    query: &str,
    limit: usize,
    folder_scope: Option<&str>,
) -> Result<Vec<FilenameResult>, rusqlite::Error> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(vec![]);
    }

    // 여러 검색어를 AND로 연결
    let terms: Vec<&str> = trimmed.split_whitespace().collect();

    if terms.is_empty() {
        return Ok(vec![]);
    }

    // 동적으로 WHERE 절 생성 (모든 검색어가 파일명에 포함되어야 함)
    // ESCAPE 절 추가로 특수문자 처리
    let mut where_clauses: Vec<String> = terms
        .iter()
        .enumerate()
        .map(|(i, _)| format!("name LIKE ?{} ESCAPE '\\'", i + 1))
        .collect();

    // folder_scope 필터 추가
    let scope_pattern = match folder_scope {
        Some(scope) if !scope.is_empty() => {
            let escaped = escape_like_pattern(scope);
            where_clauses.push(format!("path LIKE ?{} ESCAPE '\\'", terms.len() + 1));
            Some(format!("{}%", escaped))
        }
        _ => None,
    };

    let sql = format!(
        "SELECT
            id,
            path,
            name,
            file_type,
            size,
            modified_at
         FROM files
         WHERE {}
         ORDER BY name
         LIMIT ?",
        where_clauses.join(" AND ")
    );

    let mut stmt = conn.prepare(&sql)?;

    // 파라미터 바인딩 (LIKE 패턴 이스케이프 + scope + limit)
    let like_patterns: Vec<String> = terms
        .iter()
        .map(|term| format!("%{}%", escape_like_pattern(term)))
        .collect();

    let limit_i64 = limit as i64;

    // rusqlite의 params! 매크로 대신 직접 바인딩
    let results: Vec<FilenameResult> = {
        let mut param_values: Vec<&dyn rusqlite::ToSql> = like_patterns
            .iter()
            .map(|s| s as &dyn rusqlite::ToSql)
            .collect();
        if let Some(ref pattern) = scope_pattern {
            param_values.push(pattern as &dyn rusqlite::ToSql);
        }
        param_values.push(&limit_i64);

        let mut rows = stmt.query(param_values.as_slice())?;
        let mut results = Vec::new();

        while let Some(row) = rows.next()? {
            let file_name: String = row.get(2)?;
            results.push(FilenameResult {
                file_id: row.get(0)?,
                file_path: row.get(1)?,
                file_name: file_name.clone(),
                file_type: row.get(3)?,
                size: row.get(4)?,
                modified_at: row.get(5)?,
                score: 1.0,           // LIKE 검색은 스코어 없음
                highlight: file_name, // 하이라이트는 프론트엔드에서 처리
            });
        }

        results
    };

    Ok(results)
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // 검색 결과 필드 (일부만 현재 소비)
pub struct FilenameResult {
    pub file_id: i64,
    pub file_path: String,
    pub file_name: String,
    pub file_type: String,
    pub size: Option<i64>,
    pub modified_at: Option<i64>,
    pub score: f64,
    /// 하이라이트된 파일명 (LIKE에서는 원본 파일명)
    pub highlight: String,
}
