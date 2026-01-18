use rusqlite::{Connection, Result, params};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// LIKE 패턴 특수문자 이스케이프 (SQL Injection 방지)
/// %, _, \ 문자를 이스케이프하여 리터럴로 처리
fn escape_like_pattern(s: &str) -> String {
    s.replace('\\', "\\\\")
     .replace('%', "\\%")
     .replace('_', "\\_")
}

/// DB 연결 생성 (WAL 모드 + 동시성 최적화)
pub fn get_connection(db_path: &Path) -> Result<Connection> {
    let conn = Connection::open(db_path)?;

    // foreign_keys 활성화: ON DELETE CASCADE 작동 필수
    // 주의: 다른 PRAGMA보다 먼저 설정해야 함
    conn.pragma_update(None, "foreign_keys", "ON")?;

    // WAL 모드: 읽기/쓰기 동시 허용, 인덱싱 중 검색 가능
    conn.pragma_update(None, "journal_mode", "WAL")?;

    // busy_timeout: 잠금 충돌 시 5초 대기 (race condition 방지)
    conn.pragma_update(None, "busy_timeout", 5000)?;

    // synchronous=NORMAL: WAL에서 성능/안정성 균형
    conn.pragma_update(None, "synchronous", "NORMAL")?;

    Ok(conn)
}

/// 데이터베이스 초기화
pub fn init_database(db_path: &Path) -> Result<()> {
    let conn = get_connection(db_path)?;

    // 파일 메타데이터 테이블
    conn.execute(
        "CREATE TABLE IF NOT EXISTS files (
            id INTEGER PRIMARY KEY,
            path TEXT UNIQUE NOT NULL,
            name TEXT NOT NULL,
            file_type TEXT NOT NULL,
            size INTEGER,
            modified_at INTEGER,
            hash TEXT,
            indexed_at INTEGER
        )",
        [],
    )?;

    // 청크 메타데이터 테이블
    conn.execute(
        "CREATE TABLE IF NOT EXISTS chunks (
            id INTEGER PRIMARY KEY,
            file_id INTEGER REFERENCES files(id) ON DELETE CASCADE,
            chunk_index INTEGER,
            start_offset INTEGER,
            end_offset INTEGER,
            page_number INTEGER,
            paragraph_number INTEGER,
            location_hint TEXT
        )",
        [],
    )?;

    // FTS5 전문 검색 인덱스 (청크 내용)
    conn.execute(
        "CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
            content,
            content_rowid='id',
            tokenize='unicode61'
        )",
        [],
    )?;

    // FTS5 파일명 검색 인덱스
    conn.execute(
        "CREATE VIRTUAL TABLE IF NOT EXISTS files_fts USING fts5(
            name,
            content_rowid='id',
            tokenize='unicode61'
        )",
        [],
    )?;

    // 기존 파일 → files_fts 마이그레이션 (최초 실행 시)
    conn.execute(
        "INSERT OR IGNORE INTO files_fts (rowid, name) SELECT id, name FROM files",
        [],
    )?;

    // 감시 폴더 테이블
    conn.execute(
        "CREATE TABLE IF NOT EXISTS watched_folders (
            id INTEGER PRIMARY KEY,
            path TEXT UNIQUE NOT NULL,
            added_at INTEGER,
            is_favorite INTEGER DEFAULT 0
        )",
        [],
    )?;

    // 기존 테이블에 is_favorite 컬럼 추가 (마이그레이션)
    let _ = conn.execute(
        "ALTER TABLE watched_folders ADD COLUMN is_favorite INTEGER DEFAULT 0",
        [],
    );

    // 인덱스 생성
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_files_path ON files(path)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_chunks_file_id ON chunks(file_id)",
        [],
    )?;

    tracing::info!("Database initialized at {:?}", db_path);
    Ok(())
}

// ==================== 감시 폴더 ====================

/// 감시 폴더 추가
pub fn add_watched_folder(conn: &Connection, path: &str) -> Result<i64> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

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
}

/// 감시 폴더 목록 조회 (상세 정보 포함)
pub fn get_watched_folders_with_info(conn: &Connection) -> Result<Vec<WatchedFolderInfo>> {
    let mut stmt = conn.prepare(
        "SELECT path, COALESCE(is_favorite, 0), added_at FROM watched_folders ORDER BY is_favorite DESC, added_at DESC"
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(WatchedFolderInfo {
            path: row.get(0)?,
            is_favorite: row.get::<_, i32>(1)? == 1,
            added_at: row.get(2)?,
        })
    })?;

    rows.collect()
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
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    conn.execute(
        "INSERT INTO files (path, name, file_type, size, modified_at, indexed_at)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(path) DO UPDATE SET
           name = excluded.name,
           file_type = excluded.file_type,
           size = excluded.size,
           modified_at = excluded.modified_at,
           indexed_at = excluded.indexed_at",
        params![path, name, file_type, size, modified_at, now],
    )?;

    // 파일 ID 조회
    let file_id: i64 = conn.query_row(
        "SELECT id FROM files WHERE path = ?",
        params![path],
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

/// 파일 삭제 (청크 + FTS 인덱스 포함)
pub fn delete_file(conn: &Connection, path: &str) -> Result<usize> {
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
}

/// 파일 개수 조회
pub fn get_file_count(conn: &Connection) -> Result<usize> {
    conn.query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))
}

/// 폴더 내 파일 ID와 청크 ID 조회 (벡터 삭제용)
pub fn get_file_and_chunk_ids_in_folder(conn: &Connection, folder_path: &str) -> Result<Vec<(i64, Vec<i64>)>> {
    // 폴더 경로 이스케이프 (SQL Injection 방지)
    let escaped_unix = escape_like_pattern(&folder_path.replace('\\', "/"));
    let escaped_win = escape_like_pattern(&folder_path.replace('/', "\\"));

    let mut stmt = conn.prepare(
        "SELECT id FROM files WHERE path LIKE ? ESCAPE '\\' OR path LIKE ? ESCAPE '\\'"
    )?;

    // Windows/Unix 경로 모두 지원
    let pattern_unix = format!("{}/%", escaped_unix);
    let pattern_win = format!("{}\\\\%", escaped_win); // \\ → \\\\ (escaped backslash)

    let file_ids: Vec<i64> = stmt
        .query_map(params![pattern_unix, pattern_win], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    let mut results = Vec::new();
    for file_id in file_ids {
        let chunk_ids = get_chunk_ids_for_file(conn, file_id)?;
        results.push((file_id, chunk_ids));
    }

    Ok(results)
}

/// 폴더 내 모든 파일 삭제 (FTS + 파일)
pub fn delete_files_in_folder(conn: &Connection, folder_path: &str) -> Result<usize> {
    // 폴더 경로 이스케이프 (SQL Injection 방지)
    let escaped_unix = escape_like_pattern(&folder_path.replace('\\', "/"));
    let escaped_win = escape_like_pattern(&folder_path.replace('/', "\\"));
    let pattern_unix = format!("{}/%", escaped_unix);
    let pattern_win = format!("{}\\\\%", escaped_win);

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
}

// ==================== 청크 ====================

/// 파일의 기존 청크 삭제
pub fn delete_chunks_for_file(conn: &Connection, file_id: i64) -> Result<()> {
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
pub fn insert_chunk(
    conn: &Connection,
    file_id: i64,
    chunk_index: usize,
    content: &str,
    start_offset: usize,
    end_offset: usize,
    page_number: Option<usize>,
    location_hint: Option<&str>,
) -> Result<i64> {
    // 청크 메타데이터 저장
    conn.execute(
        "INSERT INTO chunks (file_id, chunk_index, start_offset, end_offset, page_number, location_hint)
         VALUES (?, ?, ?, ?, ?, ?)",
        params![
            file_id,
            chunk_index as i64,
            start_offset as i64,
            end_offset as i64,
            page_number.map(|p| p as i64),
            location_hint
        ],
    )?;

    let chunk_id = conn.last_insert_rowid();

    // FTS 인덱싱
    conn.execute(
        "INSERT INTO chunks_fts (rowid, content) VALUES (?, ?)",
        params![chunk_id, content],
    )?;

    Ok(chunk_id)
}

// ==================== 청크 조회 ====================

/// chunk_id로 청크 정보 조회
pub fn get_chunk_by_id(conn: &Connection, chunk_id: i64) -> Result<Option<ChunkInfo>> {
    let mut stmt = conn.prepare(
        "SELECT c.id, c.file_id, c.chunk_index, c.start_offset, c.end_offset, c.page_number,
                c.location_hint, f.path, f.name, fts.content
         FROM chunks c
         JOIN files f ON f.id = c.file_id
         JOIN chunks_fts fts ON fts.rowid = c.id
         WHERE c.id = ?"
    )?;

    let result = stmt.query_row(params![chunk_id], |row| {
        Ok(ChunkInfo {
            chunk_id: row.get(0)?,
            file_id: row.get(1)?,
            chunk_index: row.get(2)?,
            start_offset: row.get(3)?,
            end_offset: row.get(4)?,
            page_number: row.get(5)?,
            location_hint: row.get(6)?,
            file_path: row.get(7)?,
            file_name: row.get(8)?,
            content: row.get(9)?,
        })
    });

    match result {
        Ok(info) => Ok(Some(info)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

/// 여러 chunk_id로 청크 정보 일괄 조회
pub fn get_chunks_by_ids(conn: &Connection, chunk_ids: &[i64]) -> Result<Vec<ChunkInfo>> {
    if chunk_ids.is_empty() {
        return Ok(vec![]);
    }

    let placeholders: String = chunk_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT c.id, c.file_id, c.chunk_index, c.start_offset, c.end_offset, c.page_number,
                c.location_hint, f.path, f.name, fts.content
         FROM chunks c
         JOIN files f ON f.id = c.file_id
         JOIN chunks_fts fts ON fts.rowid = c.id
         WHERE c.id IN ({})",
        placeholders
    );

    let mut stmt = conn.prepare(&sql)?;
    let params: Vec<&dyn rusqlite::ToSql> = chunk_ids.iter().map(|id| id as &dyn rusqlite::ToSql).collect();

    let results = stmt.query_map(params.as_slice(), |row| {
        Ok(ChunkInfo {
            chunk_id: row.get(0)?,
            file_id: row.get(1)?,
            chunk_index: row.get(2)?,
            start_offset: row.get(3)?,
            end_offset: row.get(4)?,
            page_number: row.get(5)?,
            location_hint: row.get(6)?,
            file_path: row.get(7)?,
            file_name: row.get(8)?,
            content: row.get(9)?,
        })
    })?;

    results.collect()
}

/// 파일의 모든 청크 ID 조회
pub fn get_chunk_ids_for_file(conn: &Connection, file_id: i64) -> Result<Vec<i64>> {
    let mut stmt = conn.prepare("SELECT id FROM chunks WHERE file_id = ?")?;
    let rows = stmt.query_map(params![file_id], |row| row.get(0))?;
    rows.collect()
}

/// 파일 경로로 chunk ID들 조회 (벡터 인덱스 삭제용)
pub fn get_chunk_ids_for_path(conn: &Connection, path: &str) -> Result<Vec<i64>> {
    let mut stmt = conn.prepare(
        "SELECT c.id FROM chunks c
         JOIN files f ON c.file_id = f.id
         WHERE f.path = ?"
    )?;
    let rows = stmt.query_map(params![path], |row| row.get(0))?;
    rows.collect()
}

/// 폴더 통계 정보
#[derive(Debug, Clone)]
pub struct FolderStats {
    pub file_count: usize,
    pub last_indexed: Option<i64>,
}

/// 폴더별 인덱싱 통계 조회
pub fn get_folder_stats(conn: &Connection, folder_path: &str) -> Result<FolderStats> {
    // 폴더 경로 이스케이프 (SQL Injection 방지)
    let escaped_unix = escape_like_pattern(&folder_path.replace('\\', "/"));
    let escaped_win = escape_like_pattern(&folder_path.replace('/', "\\"));
    let pattern_unix = format!("{}/%", escaped_unix);
    let pattern_win = format!("{}\\\\%", escaped_win);

    let result = conn.query_row(
        "SELECT COUNT(*) as file_count, MAX(indexed_at) as last_indexed
         FROM files WHERE path LIKE ? ESCAPE '\\' OR path LIKE ? ESCAPE '\\'",
        params![pattern_unix, pattern_win],
        |row| {
            Ok(FolderStats {
                file_count: row.get::<_, i64>(0)? as usize,
                last_indexed: row.get(1)?,
            })
        },
    )?;

    Ok(result)
}

#[derive(Debug, Clone)]
pub struct ChunkInfo {
    pub chunk_id: i64,
    pub file_id: i64,
    pub chunk_index: i64,
    pub start_offset: i64,
    pub end_offset: i64,
    pub page_number: Option<i64>,
    pub location_hint: Option<String>,
    pub file_path: String,
    pub file_name: String,
    pub content: String,
}
