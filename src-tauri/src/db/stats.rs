//! 검색어 히스토리와 통계 대시보드 집계.

use super::folders::get_watched_folders;
use super::{current_timestamp, escape_like_pattern};
use rusqlite::{params, Connection, Result};

// ==================== 검색어 히스토리 (사이드바 "최근 검색") ====================

/// 검색어 저장/빈도 증가 (최대 500개 유지)
pub fn upsert_search_query(conn: &Connection, query: &str) -> Result<()> {
    let now = current_timestamp();
    conn.execute(
        "INSERT INTO search_queries (query, frequency, last_searched_at)
         VALUES (?1, 1, ?2)
         ON CONFLICT(query) DO UPDATE SET
           frequency = frequency + 1,
           last_searched_at = ?2",
        params![query, now],
    )?;

    // 오래된 저빈도 레코드 정리 (확률적: ~5% 호출 시)
    if now % 20 == 0 {
        let _ = conn.execute(
            "DELETE FROM search_queries WHERE id NOT IN (
                SELECT id FROM search_queries ORDER BY frequency DESC, last_searched_at DESC LIMIT 500
            )",
            [],
        );
    }

    Ok(())
}

// ==================== 통계 대시보드 (v2.3) ====================

/// 파일 유형별 문서 수
pub fn get_file_type_distribution(conn: &Connection) -> Result<Vec<(String, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT file_type, COUNT(*) as cnt FROM files GROUP BY file_type ORDER BY cnt DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    rows.collect()
}

/// 연도별 문서 수 (modified_at 기준)
///
/// 파일시스템 mtime 이 깨진 문서(2100년 등 미래·음수 타임스탬프)는 실제 연도로
/// 볼 수 없으므로 '미분류' 로 묶는다. 하루(86400s) 여유는 클럭 스큐 방어.
pub fn get_year_distribution(conn: &Connection) -> Result<Vec<(String, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT CASE
                  WHEN modified_at IS NULL OR modified_at <= 0
                    OR modified_at > CAST(strftime('%s', 'now') AS INTEGER) + 86400
                  THEN '미분류'
                  ELSE strftime('%Y', datetime(modified_at, 'unixepoch'))
                END as year,
                COUNT(*) as cnt
         FROM files
         GROUP BY year
         ORDER BY (year = '미분류'), year DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    rows.collect()
}

/// 최근 수정된 문서 Top N
///
/// 미래·음수 타임스탬프(깨진 mtime)는 정렬 최상위를 오염시키므로 제외한다 —
/// 이걸 걸러야 실제 최근 문서가 노출된다. 하루 여유는 클럭 스큐 방어.
pub fn get_recent_files(conn: &Connection, limit: usize) -> Result<Vec<(String, String, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT path, name, modified_at FROM files
         WHERE modified_at IS NOT NULL
           AND modified_at > 0
           AND modified_at <= CAST(strftime('%s', 'now') AS INTEGER) + 86400
         ORDER BY modified_at DESC
         LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit as i64], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;
    rows.collect()
}

/// 사용자가 최근 연 문서 Top N (last_opened_at 기준) — 홈 화면 "최근 작업한 문서"용.
/// open_file 시 기록되는 열람 신호(open_count/last_opened_at)를 노출한다.
pub fn get_recently_opened_files(
    conn: &Connection,
    limit: usize,
) -> Result<Vec<(String, String, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT path, name, last_opened_at FROM files
         WHERE last_opened_at IS NOT NULL
         ORDER BY last_opened_at DESC
         LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit as i64], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;
    rows.collect()
}

/// "최근 작업한 문서" 목록에서 제거 — last_opened_at 만 지운다.
/// open_count 등 다른 열람 신호와 파일/인덱스는 건드리지 않는다.
pub fn clear_last_opened(conn: &Connection, path: &str) -> Result<usize> {
    conn.execute(
        "UPDATE files SET last_opened_at = NULL WHERE path = ?1",
        params![path],
    )
}

/// 가장 큰 문서 Top N
pub fn get_largest_files(conn: &Connection, limit: usize) -> Result<Vec<(String, String, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT path, name, size FROM files
         WHERE size IS NOT NULL
         ORDER BY size DESC
         LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit as i64], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;
    rows.collect()
}

/// 폴더별 문서 수 (watched_folders 기준, prepared statement 재사용)
pub fn get_folder_distribution(conn: &Connection) -> Result<Vec<(String, i64)>> {
    let folders = get_watched_folders(conn)?;
    let mut result = Vec::new();

    let mut stmt = conn.prepare(
        "SELECT COUNT(*) FROM files WHERE path LIKE ?1 ESCAPE '\\' OR path LIKE ?2 ESCAPE '\\'",
    )?;

    for folder in folders {
        let clean_folder = folder.trim_end_matches(['/', '\\']);
        let escaped_unix = escape_like_pattern(&clean_folder.replace('\\', "/"));
        let escaped_win = escape_like_pattern(&clean_folder.replace('/', "\\"));
        let pattern_unix = format!("{}/%", escaped_unix);
        let pattern_win = format!("{}\\\\%", escaped_win);

        let count: i64 = stmt.query_row(params![pattern_unix, pattern_win], |row| row.get(0))?;

        if count > 0 {
            result.push((folder, count));
        }
    }

    result.sort_by_key(|b| std::cmp::Reverse(b.1));
    Ok(result)
}

/// 총 문서 크기 (바이트)
pub fn get_total_size(conn: &Connection) -> Result<i64> {
    conn.query_row("SELECT COALESCE(SUM(size), 0) FROM files", [], |row| {
        row.get(0)
    })
}

/// 모든 파일의 vector_indexed_at을 NULL로 리셋
///
/// 벡터 인덱스 파일이 손실됐을 때 DB와 동기화하기 위해 사용
pub fn reset_all_vector_indexed(conn: &Connection) -> Result<usize> {
    let affected = conn.execute(
        "UPDATE files SET vector_indexed_at = NULL WHERE vector_indexed_at IS NOT NULL",
        [],
    )?;
    Ok(affected)
}
