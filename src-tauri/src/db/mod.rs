use rusqlite::{params, Connection, Result};
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// LIKE 패턴 특수문자 이스케이프 (SQL Injection 방지)
/// %, _, \ 문자를 이스케이프하여 리터럴로 처리
pub fn escape_like_pattern(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

// ==================== 커넥션 풀 ====================

/// 커넥션 풀 (최대 4개, Drop 시 자동 반환)
/// 매 쿼리마다 Connection::open + PRAGMA 8개 실행하던 오버헤드를 제거.
/// HDD 환경에서 쿼리당 10-30ms 절감.
/// i3-12100 (4C) 기준 동시 DB 접근은 3-4개면 충분.
static CONN_POOL: Mutex<Vec<Connection>> = Mutex::new(Vec::new());
const MAX_POOL_SIZE: usize = 4;

/// 풀에서 관리되는 DB 커넥션 래퍼
/// Deref<Target=Connection>으로 기존 &Connection API 호환.
/// Drop 시 트랜잭션이 없으면 풀에 자동 반환.
pub struct PooledConnection {
    inner: Option<Connection>,
}

impl Drop for PooledConnection {
    fn drop(&mut self) {
        if let Some(conn) = self.inner.take() {
            // 열린 트랜잭션이 있으면 반환하지 않음 (안전)
            if conn.is_autocommit() {
                if let Ok(mut pool) = CONN_POOL.lock() {
                    if pool.len() < MAX_POOL_SIZE {
                        pool.push(conn);
                    }
                }
            }
            // 풀이 가득 차거나 트랜잭션 중이면 그냥 drop
        }
    }
}

impl PooledConnection {
    /// 커넥션을 풀에서 분리하여 반환 (Drop 시 풀로 반환하지 않음)
    /// 장기 보유하는 Repository 등에서 사용
    pub fn into_inner(mut self) -> Connection {
        self.inner.take().expect("PooledConnection already taken")
    }
}

impl std::ops::Deref for PooledConnection {
    type Target = Connection;
    fn deref(&self) -> &Connection {
        self.inner
            .as_ref()
            .expect("PooledConnection used after take")
    }
}

/// DB 연결 획득 (풀 우선, 없으면 새 연결 + PRAGMA 설정)
///
/// 풀에 유휴 커넥션이 있으면 PRAGMA 없이 즉시 반환 (~0ms).
/// HDD에서는 mmap_size=0으로 설정하여 랜덤 I/O 방지.
pub fn get_connection(db_path: &Path) -> Result<PooledConnection> {
    // 풀에서 재사용 시도 (PRAGMA 스킵)
    if let Ok(mut pool) = CONN_POOL.lock() {
        if let Some(conn) = pool.pop() {
            return Ok(PooledConnection { inner: Some(conn) });
        }
    }

    // 새 커넥션 생성 + PRAGMA 설정
    let conn = Connection::open(db_path)?;

    // HDD 감지: mmap은 HDD에서 랜덤 I/O → 디스크 헤드 thrashing
    let is_hdd = crate::utils::disk_info::detect_disk_type(db_path).is_hdd();
    let mmap_size = if is_hdd { 0 } else { 67108864 }; // SSD: 64MB, HDD: 0

    // 모든 PRAGMA를 단일 배치로 실행 (개별 호출 대비 ~50% 오버헤드 절감)
    conn.execute_batch(&format!(
        "PRAGMA foreign_keys = ON;
         PRAGMA journal_mode = WAL;
         PRAGMA busy_timeout = 5000;
         PRAGMA journal_size_limit = 67108864;
         PRAGMA synchronous = NORMAL;
         PRAGMA cache_size = -16384;
         PRAGMA mmap_size = {};
         PRAGMA temp_store = MEMORY;",
        mmap_size
    ))?;

    Ok(PooledConnection { inner: Some(conn) })
}

/// 현재 스키마 버전
const CURRENT_SCHEMA_VERSION: i32 = 6;

/// 스키마 버전 조회
fn get_schema_version(conn: &Connection) -> i32 {
    conn.query_row(
        "SELECT version FROM schema_version WHERE id = 1",
        [],
        |row| row.get(0),
    )
    .unwrap_or(0)
}

/// 스키마 버전 저장
fn set_schema_version(conn: &Connection, version: i32) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO schema_version (id, version) VALUES (1, ?1)",
        params![version],
    )?;
    Ok(())
}

/// 데이터베이스 초기화
pub fn init_database(db_path: &Path) -> Result<()> {
    let conn = get_connection(db_path)?;

    // 스키마 버전 테이블 (항상 먼저 생성)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS schema_version (
            id INTEGER PRIMARY KEY,
            version INTEGER NOT NULL
        )",
        [],
    )?;

    let current_version = get_schema_version(&conn);

    // === v1: 기본 테이블 ===
    if current_version < 1 {
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

        conn.execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
                content,
                content_rowid='id',
                tokenize='unicode61'
            )",
            [],
        )?;

        conn.execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS files_fts USING fts5(
                name,
                content_rowid='id',
                tokenize='unicode61'
            )",
            [],
        )?;

        conn.execute(
            "INSERT OR IGNORE INTO files_fts (rowid, name) SELECT id, name FROM files",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS watched_folders (
                id INTEGER PRIMARY KEY,
                path TEXT UNIQUE NOT NULL,
                added_at INTEGER,
                is_favorite INTEGER DEFAULT 0
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_files_path ON files(path)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_chunks_file_id ON chunks(file_id)",
            [],
        )?;

        set_schema_version(&conn, 1)?;
        tracing::info!("Schema migrated to v1 (base tables)");
    }

    // === v2: is_favorite 컬럼 ===
    if current_version < 2 {
        if let Err(e) = conn.execute(
            "ALTER TABLE watched_folders ADD COLUMN is_favorite INTEGER DEFAULT 0",
            [],
        ) {
            tracing::trace!("Migration v2: is_favorite already exists: {}", e);
        }
        set_schema_version(&conn, 2)?;
        tracing::info!("Schema migrated to v2 (is_favorite)");
    }

    // === v3: indexing_status 컬럼 ===
    if current_version < 3 {
        if let Err(e) = conn.execute(
            "ALTER TABLE watched_folders ADD COLUMN indexing_status TEXT DEFAULT 'completed'",
            [],
        ) {
            tracing::trace!("Migration v3: indexing_status already exists: {}", e);
        }
        set_schema_version(&conn, 3)?;
        tracing::info!("Schema migrated to v3 (indexing_status)");
    }

    // === v4: 2단계 인덱싱 (fts_indexed_at, vector_indexed_at) ===
    if current_version < 4 {
        if let Err(e) = conn.execute("ALTER TABLE files ADD COLUMN fts_indexed_at INTEGER", []) {
            tracing::trace!("Migration v4: fts_indexed_at already exists: {}", e);
        }
        if let Err(e) = conn.execute("ALTER TABLE files ADD COLUMN vector_indexed_at INTEGER", []) {
            tracing::trace!("Migration v4: vector_indexed_at already exists: {}", e);
        }
        // 기존 데이터 마이그레이션
        let _ = conn.execute(
            "UPDATE files SET fts_indexed_at = indexed_at WHERE fts_indexed_at IS NULL AND indexed_at IS NOT NULL",
            [],
        );
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_files_fts_indexed ON files(fts_indexed_at)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_files_vector_indexed ON files(vector_indexed_at)",
            [],
        )?;

        set_schema_version(&conn, 4)?;
        tracing::info!("Schema migrated to v4 (two-phase indexing)");
    }

    // === v5: page_end 컬럼 ===
    if current_version < 5 {
        if let Err(e) = conn.execute("ALTER TABLE chunks ADD COLUMN page_end INTEGER", []) {
            tracing::trace!("Migration v5: page_end already exists: {}", e);
        }
        set_schema_version(&conn, 5)?;
        tracing::info!("Schema migrated to v5 (page_end)");
    }

    // === v6: last_synced_at 컬럼 (시작 sync 스킵용) ===
    if current_version < 6 {
        if let Err(e) = conn.execute(
            "ALTER TABLE watched_folders ADD COLUMN last_synced_at INTEGER",
            [],
        ) {
            tracing::trace!("Migration v6: last_synced_at already exists: {}", e);
        }
        set_schema_version(&conn, 6)?;
        tracing::info!("Schema migrated to v6 (last_synced_at)");
    }

    tracing::info!(
        "Database initialized at {:?} (schema v{})",
        db_path,
        CURRENT_SCHEMA_VERSION
    );
    Ok(())
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

/// 폴더 내 파일 메타데이터 조회 (sync diff용)
pub fn get_file_metadata_in_folder(
    conn: &Connection,
    folder_path: &str,
) -> Result<std::collections::HashMap<String, (i64, Option<i64>)>> {
    let escaped_unix = escape_like_pattern(&folder_path.replace('\\', "/"));
    let escaped_win = escape_like_pattern(&folder_path.replace('/', "\\"));
    let pattern_unix = format!("{}/%", escaped_unix);
    let pattern_win = format!("{}\\\\%", escaped_win);

    let mut stmt = conn.prepare(
        "SELECT path, modified_at, size FROM files WHERE path LIKE ? ESCAPE '\\' OR path LIKE ? ESCAPE '\\'"
    )?;

    let rows = stmt.query_map(params![pattern_unix, pattern_win], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, Option<i64>>(2)?,
        ))
    })?;

    let mut map = std::collections::HashMap::new();
    for (path, modified_at, size) in rows.flatten() {
        map.insert(path, (modified_at, size));
    }
    Ok(map)
}

/// 폴더 내 이미 FTS 인덱싱 완료된 파일 경로 조회 (resume 시 스킵용)
pub fn get_fts_indexed_paths_in_folder(
    conn: &Connection,
    folder_path: &str,
) -> Result<std::collections::HashSet<String>> {
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
    let escaped_unix = escape_like_pattern(&folder_path.replace('\\', "/"));
    let escaped_win = escape_like_pattern(&folder_path.replace('/', "\\"));

    let mut stmt = conn
        .prepare("SELECT id FROM files WHERE path LIKE ? ESCAPE '\\' OR path LIKE ? ESCAPE '\\'")?;

    // Windows/Unix 경로 모두 지원
    let pattern_unix = format!("{}/%", escaped_unix);
    let pattern_win = format!("{}\\\\%", escaped_win); // \\ → \\\\ (escaped backslash)

    let file_ids: Vec<i64> = stmt
        .query_map(params![pattern_unix, pattern_win], |row| row.get(0))?
        .filter_map(|r| match r {
            Ok(id) => Some(id),
            Err(e) => {
                tracing::trace!("Skipping row in folder query: {}", e);
                None
            }
        })
        .collect();

    let mut results = Vec::new();
    for file_id in file_ids {
        let chunk_ids = get_chunk_ids_for_file(conn, file_id)?;
        results.push((file_id, chunk_ids));
    }

    Ok(results)
}

/// 폴더 내 모든 파일 삭제 (FTS + 파일) - 트랜잭션 보장
pub fn delete_files_in_folder(conn: &Connection, folder_path: &str) -> Result<usize> {
    // 폴더 경로 이스케이프 (SQL Injection 방지)
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
    // 청크 메타데이터 저장
    conn.execute(
        "INSERT INTO chunks (file_id, chunk_index, start_offset, end_offset, page_number, page_end, location_hint)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
        params![
            file_id,
            chunk_index as i64,
            start_offset as i64,
            end_offset as i64,
            page_number.map(|p| p as i64),
            page_end.map(|p| p as i64),
            location_hint
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
                c.page_end, c.location_hint, f.path, f.name, fts.content, f.modified_at
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

    conn.execute(
        "INSERT INTO files (path, name, file_type, size, modified_at, indexed_at, fts_indexed_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(path) DO UPDATE SET
           name = excluded.name,
           file_type = excluded.file_type,
           size = excluded.size,
           modified_at = excluded.modified_at,
           indexed_at = excluded.indexed_at,
           fts_indexed_at = excluded.fts_indexed_at,
           vector_indexed_at = NULL",
        params![path, name, file_type, size, modified_at, now, now],
    )?;

    // 파일 ID 조회
    let file_id: i64 = conn.query_row(
        "SELECT id FROM files WHERE path = ?",
        params![path],
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
    conn.execute(
        "INSERT INTO files (path, name, file_type, size, modified_at)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(path) DO UPDATE SET
           name = excluded.name,
           file_type = excluded.file_type,
           size = excluded.size,
           modified_at = excluded.modified_at",
        params![path, name, file_type, size, modified_at],
    )?;

    let file_id: i64 = conn.query_row(
        "SELECT id FROM files WHERE path = ?",
        params![path],
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
