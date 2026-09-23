//! 파일(files) 저장·삭제·집계와 2단계(FTS 우선, 벡터 후행) 인덱싱 상태.

use super::{current_timestamp, escape_like_pattern, migration};
use rusqlite::{params, Connection, Result};

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

    Ok(file_id)
}

/// 파일의 garbled(복사 시 깨짐) 플래그 설정.
/// 인덱싱 시점에 looks_like_garbage_text 판정 결과를 files.garbled 에 기록한다.
pub fn set_file_garbled(conn: &Connection, file_id: i64, garbled: bool) -> Result<()> {
    conn.execute(
        "UPDATE files SET garbled = ?1 WHERE id = ?2",
        params![garbled, file_id],
    )?;
    Ok(())
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

        // 3. files 삭제
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

/// 모든 데이터 초기화 — DROP + re-CREATE (DELETE 대비 수백 배 빠름)
pub fn clear_all_data(conn: &Connection, db_path: &std::path::Path) -> Result<()> {
    conn.execute_batch(
        "DROP TABLE IF EXISTS chunks_fts_vocab;
         DROP TABLE IF EXISTS chunks_fts;
         DROP TABLE IF EXISTS file_tags;
         DROP TABLE IF EXISTS bookmarks;
         DROP TABLE IF EXISTS search_queries;
         DROP TABLE IF EXISTS chunks;
         DROP TABLE IF EXISTS files;
         DROP TABLE IF EXISTS watched_folders;
         DROP TABLE IF EXISTS schema_version;",
    )?;

    // 동일 Connection으로 테이블 재생성
    migration::migrate_schema(conn, db_path)?;

    // 파일 크기 회수 — DROP 은 페이지를 freelist 로 반납할 뿐 파일은 줄지 않는다.
    // 호출부(commands/index/data.rs)가 직전에 drain_pool() 을 수행해 다른 커넥션
    // 락과 경합하지 않는 시점. VACUUM 은 임시로 DB 크기만큼 여유 공간이 필요하므로
    // 실패해도 초기화 자체는 성공 처리한다.
    if let Err(e) = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); VACUUM;") {
        tracing::warn!("VACUUM after clear_all_data failed (disk space?): {}", e);
    }

    Ok(())
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
    // 핫패스: prepare_cached로 SQL 재컴파일 방지 (배치 인덱싱 시 파일마다 호출)
    let mut stmt = conn.prepare_cached(
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
    )?;
    let file_id: i64 = stmt.query_row(
        params![path, name, file_type, size, modified_at, now, now],
        |row| row.get(0),
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
    // 핫패스: prepare_cached로 SQL 재컴파일 방지 (메타데이터 스캔 시 파일마다 호출)
    let mut stmt = conn.prepare_cached(
        "INSERT INTO files (path, name, file_type, size, modified_at)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(path) DO UPDATE SET
           name = excluded.name,
           file_type = excluded.file_type,
           size = excluded.size,
           modified_at = excluded.modified_at
         RETURNING id",
    )?;
    let file_id: i64 = stmt
        .query_row(params![path, name, file_type, size, modified_at], |row| {
            row.get(0)
        })?;

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
    // 벡터 임베딩은 **원문 content** 를 사용해야 한다.
    // fts.content 에는 형태소 토큰이 덧붙어 있어 임베딩 공간이 오염된다.
    // c.content 가 비어있는 legacy 레코드(v11 이전)만 fts.content 로 fallback.
    let mut stmt = conn.prepare(
        "SELECT c.id, COALESCE(c.content, fts.content) AS content, f.path
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
