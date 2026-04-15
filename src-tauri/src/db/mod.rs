pub mod migration;
pub mod pool;

pub use migration::*;
pub use pool::*;

use rusqlite::{params, Connection, Result};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// SQLITE_BUSY 시 재시도하는 래퍼 (busy_timeout으로 부족한 경우를 위한 application-level retry)
/// 최대 3회, 지수 백오프 100→200→400ms.
/// ⚠️ sync-only: std::thread::sleep 사용. async 컨텍스트에서는 spawn_blocking 내에서 호출할 것.
pub fn retry_on_busy<F, T>(f: F) -> Result<T>
where
    F: Fn() -> Result<T>,
{
    const MAX_RETRIES: u32 = 3;
    const RETRY_BASE_MS: u64 = 100; // 지수 백오프: 100ms → 200ms → 400ms

    for attempt in 0..MAX_RETRIES {
        match f() {
            Ok(v) => return Ok(v),
            Err(e) => {
                let is_busy = matches!(
                    e,
                    rusqlite::Error::SqliteFailure(
                        rusqlite::ffi::Error {
                            code: rusqlite::ffi::ErrorCode::DatabaseBusy,
                            ..
                        },
                        _,
                    )
                );
                if is_busy && attempt < MAX_RETRIES - 1 {
                    let delay_ms = RETRY_BASE_MS << attempt; // 100, 200, 400
                    tracing::warn!(
                        "[DB retry] SQLITE_BUSY on attempt {}/{}, retrying in {}ms...",
                        attempt + 1,
                        MAX_RETRIES,
                        delay_ms
                    );
                    std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                    continue;
                }
                return Err(e);
            }
        }
    }
    unreachable!()
}

/// LIKE 패턴 특수문자 이스케이프 (SQL Injection 방지)
/// %, _, \ 문자를 이스케이프하여 리터럴로 처리
pub fn escape_like_pattern(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

// ==================== 감시 폴더 ====================

/// 현재 시간을 Unix timestamp로 반환 (패닉 방지)
fn current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// 감시 폴더가 이미 등록되어 있는지 확인
pub fn is_folder_watched(conn: &Connection, path: &str) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM watched_folders WHERE path = ?",
        params![path],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

/// 감시 폴더 추가
pub fn add_watched_folder(conn: &Connection, path: &str) -> Result<i64> {
    let now = current_timestamp();

    conn.execute(
        "INSERT OR IGNORE INTO watched_folders (path, added_at) VALUES (?, ?)",
        params![path, now],
    )?;

    Ok(conn.last_insert_rowid())
}

/// 감시 폴더 목록 조회
pub fn get_watched_folders(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT path FROM watched_folders")?;
    let rows = stmt.query_map([], |row| row.get(0))?;
    rows.collect()
}

/// 감시 폴더 삭제
pub fn remove_watched_folder(conn: &Connection, path: &str) -> Result<usize> {
    conn.execute("DELETE FROM watched_folders WHERE path = ?", params![path])
}

/// 즐겨찾기 토글
pub fn toggle_favorite(conn: &Connection, path: &str) -> Result<bool> {
    // 현재 상태 확인
    let current: i32 = conn.query_row(
        "SELECT COALESCE(is_favorite, 0) FROM watched_folders WHERE path = ?",
        params![path],
        |row| row.get(0),
    )?;

    let new_value = if current == 0 { 1 } else { 0 };

    conn.execute(
        "UPDATE watched_folders SET is_favorite = ? WHERE path = ?",
        params![new_value, path],
    )?;

    Ok(new_value == 1)
}

/// 폴더 정보 (즐겨찾기 포함)
#[derive(Debug, Clone)]
pub struct WatchedFolderInfo {
    pub path: String,
    pub is_favorite: bool,
    pub added_at: Option<i64>,
    pub indexing_status: String,
    pub last_synced_at: Option<i64>,
}

/// 감시 폴더 목록 조회 (상세 정보 포함)
pub fn get_watched_folders_with_info(conn: &Connection) -> Result<Vec<WatchedFolderInfo>> {
    let mut stmt = conn.prepare(
        "SELECT path, COALESCE(is_favorite, 0), added_at, COALESCE(indexing_status, 'completed'), last_synced_at FROM watched_folders ORDER BY is_favorite DESC, added_at DESC"
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(WatchedFolderInfo {
            path: row.get(0)?,
            is_favorite: row.get::<_, i32>(1)? == 1,
            added_at: row.get(2)?,
            indexing_status: row.get(3)?,
            last_synced_at: row.get(4)?,
        })
    })?;

    rows.collect()
}

/// 폴더 인덱싱 상태 업데이트
pub fn set_folder_indexing_status(conn: &Connection, path: &str, status: &str) -> Result<usize> {
    conn.execute(
        "UPDATE watched_folders SET indexing_status = ? WHERE path = ?",
        params![status, path],
    )
}

/// 폴더 마지막 동기화 시각 업데이트
pub fn update_last_synced_at(conn: &Connection, path: &str) -> Result<usize> {
    let now = current_timestamp();
    conn.execute(
        "UPDATE watched_folders SET last_synced_at = ? WHERE path = ?",
        params![now, path],
    )
}

/// 폴더 내 이미 FTS 인덱싱 완료된 파일 경로 조회 (resume 시 스킵용)
pub fn get_fts_indexed_paths_in_folder(
    conn: &Connection,
    folder_path: &str,
) -> Result<std::collections::HashSet<String>> {
    let folder_path = folder_path.trim_end_matches(['/', '\\']);
    let escaped_unix = escape_like_pattern(&folder_path.replace('\\', "/"));
    let escaped_win = escape_like_pattern(&folder_path.replace('/', "\\"));
    let pattern_unix = format!("{}/%", escaped_unix);
    let pattern_win = format!("{}\\\\%", escaped_win);

    let mut stmt = conn.prepare(
        "SELECT path FROM files WHERE fts_indexed_at IS NOT NULL AND (path LIKE ? ESCAPE '\\' OR path LIKE ? ESCAPE '\\')"
    )?;

    let rows = stmt.query_map(params![pattern_unix, pattern_win], |row| {
        row.get::<_, String>(0)
    })?;

    let mut set = std::collections::HashSet::new();
    for path in rows.flatten() {
        set.insert(path);
    }
    Ok(set)
}

// ==================== 파일 ====================

/// 파일 저장 (upsert)
pub fn upsert_file(
    conn: &Connection,
    path: &str,
    name: &str,
    file_type: &str,
    size: i64,
    modified_at: i64,
) -> Result<i64> {
    let now = current_timestamp();

    // RETURNING으로 INSERT/UPDATE 모두에서 id를 1회 쿼리로 획득
    let file_id: i64 = conn.query_row(
        "INSERT INTO files (path, name, file_type, size, modified_at, indexed_at)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(path) DO UPDATE SET
           name = excluded.name,
           file_type = excluded.file_type,
           size = excluded.size,
           modified_at = excluded.modified_at,
           indexed_at = excluded.indexed_at
         RETURNING id",
        params![path, name, file_type, size, modified_at, now],
        |row| row.get(0),
    )?;

    // files_fts 인덱스 갱신 (파일명 검색용)
    // FTS5는 UPSERT 미지원 → DELETE 후 INSERT
    conn.execute("DELETE FROM files_fts WHERE rowid = ?", params![file_id])?;
    conn.execute(
        "INSERT INTO files_fts (rowid, name) VALUES (?, ?)",
        params![file_id, name],
    )?;

    Ok(file_id)
}

/// 파일 삭제 (청크 + FTS 인덱스 포함) - 트랜잭션 보장
pub fn delete_file(conn: &Connection, path: &str) -> Result<usize> {
    // 트랜잭션 시작 (원자성 보장)
    conn.execute("BEGIN IMMEDIATE", [])?;

    let result = (|| -> Result<usize> {
        // 1. chunks_fts에서 삭제
        conn.execute(
            "DELETE FROM chunks_fts WHERE rowid IN (
                SELECT c.id FROM chunks c
                JOIN files f ON c.file_id = f.id
                WHERE f.path = ?
            )",
            params![path],
        )?;

        // 2. chunks 명시적 삭제 (foreign_keys 미활성화 환경 대비)
        conn.execute(
            "DELETE FROM chunks WHERE file_id IN (
                SELECT id FROM files WHERE path = ?
            )",
            params![path],
        )?;

        // 3. files_fts에서 삭제 (파일명 검색 인덱스)
        conn.execute(
            "DELETE FROM files_fts WHERE rowid IN (
                SELECT id FROM files WHERE path = ?
            )",
            params![path],
        )?;

        // 4. files 삭제
        conn.execute("DELETE FROM files WHERE path = ?", params![path])
    })();

    match result {
        Ok(count) => {
            conn.execute("COMMIT", [])?;
            Ok(count)
        }
        Err(e) => {
            let _ = conn.execute("ROLLBACK", []);
            Err(e)
        }
    }
}

/// 파일 개수 조회
pub fn get_file_count(conn: &Connection) -> Result<usize> {
    conn.query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))
}

/// FTS 인덱싱 완료된 파일 개수 (문서 수)
pub fn get_indexed_file_count(conn: &Connection) -> Result<usize> {
    conn.query_row(
        "SELECT COUNT(*) FROM files WHERE fts_indexed_at IS NOT NULL",
        [],
        |row| row.get(0),
    )
}

/// 폴더 내 파일 ID와 청크 ID 조회 (벡터 삭제용)
pub fn get_file_and_chunk_ids_in_folder(
    conn: &Connection,
    folder_path: &str,
) -> Result<Vec<(i64, Vec<i64>)>> {
    // 폴더 경로 이스케이프 (SQL Injection 방지)
    let folder_path = folder_path.trim_end_matches(['/', '\\']);
    let escaped_unix = escape_like_pattern(&folder_path.replace('\\', "/"));
    let escaped_win = escape_like_pattern(&folder_path.replace('/', "\\"));

    // Windows/Unix 경로 모두 지원
    let pattern_unix = format!("{}/%", escaped_unix);
    let pattern_win = format!("{}\\\\%", escaped_win);

    // 단일 JOIN 쿼리로 N+1 문제 해결
    let mut stmt = conn.prepare(
        "SELECT f.id, c.id FROM files f
         LEFT JOIN chunks c ON c.file_id = f.id
         WHERE f.path LIKE ? ESCAPE '\\' OR f.path LIKE ? ESCAPE '\\'
         ORDER BY f.id",
    )?;

    let rows = stmt.query_map(params![pattern_unix, pattern_win], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, Option<i64>>(1)?))
    })?;

    let mut results: Vec<(i64, Vec<i64>)> = Vec::new();
    let mut last_file_id: Option<i64> = None;

    for row in rows {
        let (file_id, chunk_id) = match row {
            Ok(r) => r,
            Err(e) => {
                tracing::trace!("Skipping row in folder query: {}", e);
                continue;
            }
        };

        if last_file_id != Some(file_id) {
            results.push((file_id, Vec::new()));
            last_file_id = Some(file_id);
        }

        if let Some(cid) = chunk_id {
            if let Some(last) = results.last_mut() {
                last.1.push(cid);
            }
        }
    }

    Ok(results)
}

/// 폴더 내 모든 파일 삭제 (FTS + 파일) - 트랜잭션 보장
pub fn delete_files_in_folder(conn: &Connection, folder_path: &str) -> Result<usize> {
    // 폴더 경로 이스케이프 (SQL Injection 방지)
    let folder_path = folder_path.trim_end_matches(['/', '\\']);
    let escaped_unix = escape_like_pattern(&folder_path.replace('\\', "/"));
    let escaped_win = escape_like_pattern(&folder_path.replace('/', "\\"));
    let pattern_unix = format!("{}/%", escaped_unix);
    let pattern_win = format!("{}\\\\%", escaped_win);

    // 트랜잭션 시작 (원자성 보장)
    conn.execute("BEGIN IMMEDIATE", [])?;

    let result = (|| -> Result<usize> {
        // chunks_fts 삭제
        conn.execute(
            "DELETE FROM chunks_fts WHERE rowid IN (
                SELECT c.id FROM chunks c
                JOIN files f ON c.file_id = f.id
                WHERE f.path LIKE ? ESCAPE '\\' OR f.path LIKE ? ESCAPE '\\'
            )",
            params![pattern_unix, pattern_win],
        )?;

        // files_fts 삭제 (파일명 검색 인덱스)
        conn.execute(
            "DELETE FROM files_fts WHERE rowid IN (
                SELECT id FROM files
                WHERE path LIKE ? ESCAPE '\\' OR path LIKE ? ESCAPE '\\'
            )",
            params![pattern_unix, pattern_win],
        )?;

        // 파일 삭제 (chunks는 CASCADE로 삭제됨)
        conn.execute(
            "DELETE FROM files WHERE path LIKE ? ESCAPE '\\' OR path LIKE ? ESCAPE '\\'",
            params![pattern_unix, pattern_win],
        )
    })();

    match result {
        Ok(count) => {
            conn.execute("COMMIT", [])?;
            Ok(count)
        }
        Err(e) => {
            let _ = conn.execute("ROLLBACK", []);
            Err(e)
        }
    }
}

/// 모든 데이터 초기화 (files, chunks, FTS, watched_folders) - 트랜잭션 보장
pub fn clear_all_data(conn: &Connection) -> Result<()> {
    // 트랜잭션 시작 (원자성 보장)
    conn.execute("BEGIN IMMEDIATE", [])?;

    let result = (|| -> Result<()> {
        // FTS 먼저 삭제
        conn.execute("DELETE FROM chunks_fts", [])?;
        conn.execute("DELETE FROM files_fts", [])?;

        // chunks (CASCADE로 자동 삭제되지만 명시적 삭제)
        conn.execute("DELETE FROM chunks", [])?;

        // files
        conn.execute("DELETE FROM files", [])?;

        // watched_folders
        conn.execute("DELETE FROM watched_folders", [])?;

        // bookmarks
        let _ = conn.execute("DELETE FROM bookmarks", []);

        // file_tags
        let _ = conn.execute("DELETE FROM file_tags", []);

        // search_queries (자동완성 히스토리)
        let _ = conn.execute("DELETE FROM search_queries", []);

        Ok(())
    })();

    match result {
        Ok(()) => {
            conn.execute("COMMIT", [])?;
            Ok(())
        }
        Err(e) => {
            let _ = conn.execute("ROLLBACK", []);
            Err(e)
        }
    }
}

// ==================== 청크 ====================

/// 파일의 기존 청크 삭제 - 트랜잭션 보장 (단독 호출용)
pub fn delete_chunks_for_file(conn: &Connection, file_id: i64) -> Result<()> {
    // 트랜잭션 시작 (원자성 보장)
    conn.execute("BEGIN IMMEDIATE", [])?;

    let result = delete_chunks_for_file_no_tx(conn, file_id);

    match result {
        Ok(()) => {
            conn.execute("COMMIT", [])?;
            Ok(())
        }
        Err(e) => {
            let _ = conn.execute("ROLLBACK", []);
            Err(e)
        }
    }
}

/// 파일의 기존 청크 삭제 - 트랜잭션 없음 (배치 파이프라인용)
///
/// 호출자가 이미 트랜잭션을 관리하는 경우 사용.
/// 중첩 BEGIN 방지로 배치 인덱싱 시 에러 해소.
pub fn delete_chunks_for_file_no_tx(conn: &Connection, file_id: i64) -> Result<()> {
    // FTS에서 먼저 삭제
    conn.execute(
        "DELETE FROM chunks_fts WHERE rowid IN (
            SELECT id FROM chunks WHERE file_id = ?
        )",
        params![file_id],
    )?;

    conn.execute("DELETE FROM chunks WHERE file_id = ?", params![file_id])?;
    Ok(())
}

/// 청크 저장 + FTS 인덱싱
///
/// `fts_extra_tokens`: 형태소 분석 결과 등 FTS에 추가로 인덱싱할 토큰들.
/// unicode61 토크나이저는 "고용보험료"를 하나의 토큰으로 처리하므로,
/// Lindera 형태소 분석 결과("고용", "보험료")를 함께 저장해야
/// "보험료"로 검색했을 때도 매칭됨.
#[allow(clippy::too_many_arguments)]
pub fn insert_chunk(
    conn: &Connection,
    file_id: i64,
    chunk_index: usize,
    content: &str,
    start_offset: usize,
    end_offset: usize,
    page_number: Option<usize>,
    page_end: Option<usize>,
    location_hint: Option<&str>,
    fts_extra_tokens: Option<&str>,
) -> Result<i64> {
    // 청크 메타데이터 + 원본 content 저장
    conn.execute(
        "INSERT INTO chunks (file_id, chunk_index, start_offset, end_offset, page_number, page_end, location_hint, content)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            file_id,
            chunk_index as i64,
            start_offset as i64,
            end_offset as i64,
            page_number.map(|p| p as i64),
            page_end.map(|p| p as i64),
            location_hint,
            content
        ],
    )?;

    let chunk_id = conn.last_insert_rowid();

    // FTS 인덱싱 (원본 content + 형태소 토큰)
    let fts_content = match fts_extra_tokens {
        Some(tokens) if !tokens.is_empty() => format!("{} {}", content, tokens),
        _ => content.to_string(),
    };
    conn.execute(
        "INSERT INTO chunks_fts (rowid, content) VALUES (?, ?)",
        params![chunk_id, fts_content],
    )?;

    Ok(chunk_id)
}

// ==================== 청크 조회 ====================

/// 여러 chunk_id로 청크 정보 일괄 조회
pub fn get_chunks_by_ids(conn: &Connection, chunk_ids: &[i64]) -> Result<Vec<ChunkInfo>> {
    if chunk_ids.is_empty() {
        return Ok(vec![]);
    }

    let placeholders: String = chunk_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT c.id, c.file_id, c.chunk_index, c.start_offset, c.end_offset, c.page_number,
                c.page_end, c.location_hint, f.path, f.name,
                COALESCE(c.content, fts.content) AS content, f.modified_at
         FROM chunks c
         JOIN files f ON f.id = c.file_id
         JOIN chunks_fts fts ON fts.rowid = c.id
         WHERE c.id IN ({})",
        placeholders
    );

    let mut stmt = conn.prepare(&sql)?;
    let params: Vec<&dyn rusqlite::ToSql> = chunk_ids
        .iter()
        .map(|id| id as &dyn rusqlite::ToSql)
        .collect();

    let results = stmt.query_map(params.as_slice(), |row| {
        Ok(ChunkInfo {
            chunk_id: row.get(0)?,
            file_id: row.get(1)?,
            chunk_index: row.get(2)?,
            start_offset: row.get(3)?,
            end_offset: row.get(4)?,
            page_number: row.get(5)?,
            page_end: row.get(6)?,
            location_hint: row.get(7)?,
            file_path: row.get(8)?,
            file_name: row.get(9)?,
            content: row.get(10)?,
            modified_at: row.get(11)?,
        })
    })?;

    results.collect()
}

/// 파일 경로 + 청크 인덱스 목록으로 청크 일괄 조회 (RAG 이웃 청크 확장용)
pub fn get_chunks_for_file_indices(
    conn: &Connection,
    file_path: &str,
    chunk_indices: &[i64],
) -> Result<Vec<ChunkInfo>> {
    if chunk_indices.is_empty() {
        return Ok(vec![]);
    }

    let placeholders: String = chunk_indices.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT c.id, c.file_id, c.chunk_index, c.start_offset, c.end_offset, c.page_number,
                c.page_end, c.location_hint, f.path, f.name,
                COALESCE(c.content, fts.content) AS content, f.modified_at
         FROM chunks c
         JOIN files f ON f.id = c.file_id
         JOIN chunks_fts fts ON fts.rowid = c.id
         WHERE f.path = ? AND c.chunk_index IN ({})",
        placeholders
    );

    let mut stmt = conn.prepare(&sql)?;
    let mut params_vec: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(chunk_indices.len() + 1);
    params_vec.push(&file_path);
    for idx in chunk_indices {
        params_vec.push(idx as &dyn rusqlite::ToSql);
    }

    let results = stmt.query_map(params_vec.as_slice(), |row| {
        Ok(ChunkInfo {
            chunk_id: row.get(0)?,
            file_id: row.get(1)?,
            chunk_index: row.get(2)?,
            start_offset: row.get(3)?,
            end_offset: row.get(4)?,
            page_number: row.get(5)?,
            page_end: row.get(6)?,
            location_hint: row.get(7)?,
            file_path: row.get(8)?,
            file_name: row.get(9)?,
            content: row.get(10)?,
            modified_at: row.get(11)?,
        })
    })?;

    results.collect()
}

/// 청크 ID → 파일 경로 경량 조회 (content 없이 경로만 — 벡터 스코프 프리필터용)
pub fn get_chunk_file_paths(conn: &Connection, chunk_ids: &[i64]) -> Result<HashMap<i64, String>> {
    if chunk_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let placeholders: String = chunk_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT c.id, f.path FROM chunks c JOIN files f ON f.id = c.file_id WHERE c.id IN ({})",
        placeholders
    );

    let mut stmt = conn.prepare(&sql)?;
    let params: Vec<&dyn rusqlite::ToSql> = chunk_ids
        .iter()
        .map(|id| id as &dyn rusqlite::ToSql)
        .collect();

    let mut map = HashMap::with_capacity(chunk_ids.len());
    let rows = stmt.query_map(params.as_slice(), |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (id, path) = row?;
        map.insert(id, path);
    }
    Ok(map)
}

/// 파일 경로로 chunk ID들 조회 (벡터 인덱스 삭제용)
pub fn get_chunk_ids_for_path(conn: &Connection, path: &str) -> Result<Vec<i64>> {
    let mut stmt = conn.prepare(
        "SELECT c.id FROM chunks c
         JOIN files f ON c.file_id = f.id
         WHERE f.path = ?",
    )?;
    let rows = stmt.query_map(params![path], |row| row.get(0))?;
    rows.collect()
}

/// 폴더 통계 정보
#[derive(Debug, Clone)]
pub struct FolderStats {
    pub file_count: usize,
    pub indexed_count: usize,
    pub last_indexed: Option<i64>,
}

/// 폴더별 인덱싱 통계 조회
pub fn get_folder_stats(conn: &Connection, folder_path: &str) -> Result<FolderStats> {
    // 폴더 경로 이스케이프 (SQL Injection 방지)
    let folder_path = folder_path.trim_end_matches(['/', '\\']);
    let escaped_unix = escape_like_pattern(&folder_path.replace('\\', "/"));
    let escaped_win = escape_like_pattern(&folder_path.replace('/', "\\"));
    let pattern_unix = format!("{}/%", escaped_unix);
    let pattern_win = format!("{}\\\\%", escaped_win);

    let result = conn.query_row(
        "SELECT COUNT(*) as file_count,
                SUM(CASE WHEN fts_indexed_at IS NOT NULL THEN 1 ELSE 0 END) as indexed_count,
                MAX(indexed_at) as last_indexed
         FROM files WHERE path LIKE ? ESCAPE '\\' OR path LIKE ? ESCAPE '\\'",
        params![pattern_unix, pattern_win],
        |row| {
            Ok(FolderStats {
                file_count: row.get::<_, i64>(0)? as usize,
                indexed_count: row.get::<_, i64>(1)? as usize,
                last_indexed: row.get(2)?,
            })
        },
    )?;

    Ok(result)
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // 구조체 필드는 데이터 모델의 일부 (일부 필드만 현재 사용)
pub struct ChunkInfo {
    pub chunk_id: i64,
    pub file_id: i64,
    pub chunk_index: i64,
    pub start_offset: i64,
    pub end_offset: i64,
    pub page_number: Option<i64>,
    pub page_end: Option<i64>,
    pub location_hint: Option<String>,
    pub file_path: String,
    pub file_name: String,
    pub content: String,
    pub modified_at: Option<i64>,
}

// ==================== 2단계 인덱싱 ====================

/// 파일 저장 (FTS만, 벡터 인덱싱 대기 상태)
pub fn upsert_file_fts_only(
    conn: &Connection,
    path: &str,
    name: &str,
    file_type: &str,
    size: i64,
    modified_at: i64,
) -> Result<i64> {
    let now = current_timestamp();

    // RETURNING으로 INSERT/UPDATE 모두에서 id를 1회 쿼리로 획득
    let file_id: i64 = conn.query_row(
        "INSERT INTO files (path, name, file_type, size, modified_at, indexed_at, fts_indexed_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(path) DO UPDATE SET
           name = excluded.name,
           file_type = excluded.file_type,
           size = excluded.size,
           modified_at = excluded.modified_at,
           indexed_at = excluded.indexed_at,
           fts_indexed_at = excluded.fts_indexed_at,
           vector_indexed_at = NULL
         RETURNING id",
        params![path, name, file_type, size, modified_at, now, now],
        |row| row.get(0),
    )?;

    // files_fts 인덱스 갱신
    conn.execute("DELETE FROM files_fts WHERE rowid = ?", params![file_id])?;
    conn.execute(
        "INSERT INTO files_fts (rowid, name) VALUES (?, ?)",
        params![file_id, name],
    )?;

    Ok(file_id)
}

/// 파일 메타데이터만 저장 (FTS 인덱싱 없이, 파일명 검색용)
/// scan_metadata_only()에서 사용
pub fn insert_file_metadata_only(
    conn: &Connection,
    path: &str,
    name: &str,
    file_type: &str,
    size: i64,
    modified_at: i64,
) -> Result<i64> {
    // fts_indexed_at = NULL, vector_indexed_at = NULL (파싱 대기 상태)
    // RETURNING으로 INSERT/UPDATE 모두에서 id를 1회 쿼리로 획득
    let file_id: i64 = conn.query_row(
        "INSERT INTO files (path, name, file_type, size, modified_at)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(path) DO UPDATE SET
           name = excluded.name,
           file_type = excluded.file_type,
           size = excluded.size,
           modified_at = excluded.modified_at
         RETURNING id",
        params![path, name, file_type, size, modified_at],
        |row| row.get(0),
    )?;

    // files_fts 인덱스 갱신 (파일명 검색용)
    conn.execute("DELETE FROM files_fts WHERE rowid = ?", params![file_id])?;
    conn.execute(
        "INSERT INTO files_fts (rowid, name) VALUES (?, ?)",
        params![file_id, name],
    )?;

    Ok(file_id)
}

/// 벡터 인덱싱 대기 중인 청크
#[derive(Debug, Clone)]
pub struct PendingChunk {
    pub chunk_id: i64,
    pub content: String,
    pub file_path: String,
}

/// 특정 파일의 pending 청크 전체 조회 (DB 레벨 필터링)
///
/// LIMIT 없이 파일의 모든 청크를 반환하여 부분 처리 방지
pub fn get_pending_vector_chunks_for_file(
    conn: &Connection,
    file_id: i64,
) -> Result<Vec<PendingChunk>> {
    let mut stmt = conn.prepare(
        "SELECT c.id, fts.content, f.path
         FROM chunks c
         JOIN files f ON f.id = c.file_id
         JOIN chunks_fts fts ON fts.rowid = c.id
         WHERE f.id = ? AND f.fts_indexed_at IS NOT NULL AND f.vector_indexed_at IS NULL
         ORDER BY c.chunk_index",
    )?;

    let results = stmt.query_map(params![file_id], |row| {
        Ok(PendingChunk {
            chunk_id: row.get(0)?,
            content: row.get(1)?,
            file_path: row.get(2)?,
        })
    })?;

    results.collect()
}

/// 파일의 벡터 인덱싱 완료 표시
pub fn mark_file_vector_indexed(conn: &Connection, file_id: i64) -> Result<()> {
    let now = current_timestamp();

    conn.execute(
        "UPDATE files SET vector_indexed_at = ? WHERE id = ?",
        params![now, file_id],
    )?;

    Ok(())
}

/// 벡터 인덱싱 통계
#[derive(Debug, Clone, serde::Serialize)]
pub struct VectorIndexingStats {
    pub total_files: usize,
    pub fts_only_files: usize,
    pub vector_indexed_files: usize,
    pub pending_chunks: usize,
    /// 이미 벡터 인덱싱 완료된 청크 수 (누적 진행률 계산용)
    pub completed_chunks: usize,
}

/// 벡터 인덱싱 통계 조회
pub fn get_vector_indexing_stats(conn: &Connection) -> Result<VectorIndexingStats> {
    let total_files: i64 = conn.query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))?;

    let fts_only_files: i64 = conn.query_row(
        "SELECT COUNT(*) FROM files WHERE fts_indexed_at IS NOT NULL AND vector_indexed_at IS NULL",
        [],
        |row| row.get(0),
    )?;

    let vector_indexed_files: i64 = conn.query_row(
        "SELECT COUNT(*) FROM files WHERE vector_indexed_at IS NOT NULL",
        [],
        |row| row.get(0),
    )?;

    let pending_chunks: i64 = conn.query_row(
        "SELECT COUNT(*) FROM chunks c
         JOIN files f ON f.id = c.file_id
         WHERE f.fts_indexed_at IS NOT NULL AND f.vector_indexed_at IS NULL",
        [],
        |row| row.get(0),
    )?;

    let completed_chunks: i64 = conn.query_row(
        "SELECT COUNT(*) FROM chunks c
         JOIN files f ON f.id = c.file_id
         WHERE f.vector_indexed_at IS NOT NULL",
        [],
        |row| row.get(0),
    )?;

    Ok(VectorIndexingStats {
        total_files: total_files as usize,
        fts_only_files: fts_only_files as usize,
        vector_indexed_files: vector_indexed_files as usize,
        pending_chunks: pending_chunks as usize,
        completed_chunks: completed_chunks as usize,
    })
}

/// 벡터 인덱싱 대기 중인 파일 ID 목록 조회
pub fn get_pending_vector_file_ids(conn: &Connection) -> Result<Vec<i64>> {
    let mut stmt = conn.prepare(
        "SELECT id FROM files WHERE fts_indexed_at IS NOT NULL AND vector_indexed_at IS NULL ORDER BY id"
    )?;

    let results = stmt.query_map([], |row| row.get(0))?;
    results.collect()
}

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
pub fn get_year_distribution(conn: &Connection) -> Result<Vec<(String, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT COALESCE(strftime('%Y', datetime(modified_at, 'unixepoch')), '미분류') as year,
                COUNT(*) as cnt
         FROM files
         GROUP BY year
         ORDER BY year DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    rows.collect()
}

/// 최근 수정된 문서 Top N
pub fn get_recent_files(conn: &Connection, limit: usize) -> Result<Vec<(String, String, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT path, name, modified_at FROM files
         WHERE modified_at IS NOT NULL
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

    result.sort_by(|a, b| b.1.cmp(&a.1));
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
