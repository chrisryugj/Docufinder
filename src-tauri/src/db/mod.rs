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

    // === 성능 최적화 PRAGMA (8GB RAM / HDD 환경) ===
    // cache_size: 16MB (메모리 여유 확보)
    conn.pragma_update(None, "cache_size", -16384)?;

    // mmap_size: 64MB (HDD에서 과도한 mmap 방지)
    conn.pragma_update(None, "mmap_size", 67108864)?;

    // temp_store: 임시 테이블 메모리 사용 (I/O 감소)
    conn.pragma_update(None, "temp_store", "MEMORY")?;

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

    // 기존 테이블에 is_favorite 컬럼 추가 (마이그레이션 - 이미 존재하면 무시)
    if let Err(e) = conn.execute(
        "ALTER TABLE watched_folders ADD COLUMN is_favorite INTEGER DEFAULT 0",
        [],
    ) {
        tracing::trace!("Migration: is_favorite column already exists or failed: {}", e);
    }

    // 2단계 인덱싱 지원: fts_indexed_at, vector_indexed_at 컬럼 추가
    if let Err(e) = conn.execute(
        "ALTER TABLE files ADD COLUMN fts_indexed_at INTEGER",
        [],
    ) {
        tracing::trace!("Migration: fts_indexed_at column already exists or failed: {}", e);
    }
    if let Err(e) = conn.execute(
        "ALTER TABLE files ADD COLUMN vector_indexed_at INTEGER",
        [],
    ) {
        tracing::trace!("Migration: vector_indexed_at column already exists or failed: {}", e);
    }

    // 기존 데이터 마이그레이션: indexed_at 값을 fts_indexed_at으로 복사
    if let Err(e) = conn.execute(
        "UPDATE files SET fts_indexed_at = indexed_at WHERE fts_indexed_at IS NULL AND indexed_at IS NOT NULL",
        [],
    ) {
        tracing::warn!("Migration: failed to copy indexed_at to fts_indexed_at: {}", e);
    }

    // 인덱스 생성
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_files_path ON files(path)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_chunks_file_id ON chunks(file_id)",
        [],
    )?;

    // === 2단계 인덱싱 성능 최적화 인덱스 ===
    // 벡터 대기 파일 조회 최적화 (10배 빠름)
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_files_fts_indexed ON files(fts_indexed_at)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_files_vector_indexed ON files(vector_indexed_at)",
        [],
    )?;

    tracing::info!("Database initialized at {:?}", db_path);
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

/// 파일의 기존 청크 삭제 - 트랜잭션 보장
pub fn delete_chunks_for_file(conn: &Connection, file_id: i64) -> Result<()> {
    // 트랜잭션 시작 (원자성 보장)
    conn.execute("BEGIN IMMEDIATE", [])?;

    let result = (|| -> Result<()> {
        // FTS에서 먼저 삭제
        conn.execute(
            "DELETE FROM chunks_fts WHERE rowid IN (
                SELECT id FROM chunks WHERE file_id = ?
            )",
            params![file_id],
        )?;

        conn.execute("DELETE FROM chunks WHERE file_id = ?", params![file_id])?;
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

/// 여러 chunk_id로 청크 정보 일괄 조회
pub fn get_chunks_by_ids(conn: &Connection, chunk_ids: &[i64]) -> Result<Vec<ChunkInfo>> {
    if chunk_ids.is_empty() {
        return Ok(vec![]);
    }

    let placeholders: String = chunk_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT c.id, c.file_id, c.chunk_index, c.start_offset, c.end_offset, c.page_number,
                c.location_hint, f.path, f.name, fts.content, f.modified_at
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
            modified_at: row.get(10)?,
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
#[allow(dead_code)]
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
/// NOTE: 현재 미사용 (scan_metadata_only가 미사용)
#[allow(dead_code)]
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

/// 특정 파일의 pending 청크 조회 (DB 레벨 필터링)
pub fn get_pending_vector_chunks_for_file(
    conn: &Connection,
    file_id: i64,
    limit: usize,
) -> Result<Vec<PendingChunk>> {
    let mut stmt = conn.prepare(
        "SELECT c.id, fts.content, f.path
         FROM chunks c
         JOIN files f ON f.id = c.file_id
         JOIN chunks_fts fts ON fts.rowid = c.id
         WHERE f.id = ? AND f.fts_indexed_at IS NOT NULL AND f.vector_indexed_at IS NULL
         ORDER BY c.chunk_index
         LIMIT ?"
    )?;

    let results = stmt.query_map(params![file_id, limit as i64], |row| {
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
}

/// 벡터 인덱싱 통계 조회
pub fn get_vector_indexing_stats(conn: &Connection) -> Result<VectorIndexingStats> {
    let total_files: i64 = conn.query_row(
        "SELECT COUNT(*) FROM files",
        [],
        |row| row.get(0),
    )?;

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

    Ok(VectorIndexingStats {
        total_files: total_files as usize,
        fts_only_files: fts_only_files as usize,
        vector_indexed_files: vector_indexed_files as usize,
        pending_chunks: pending_chunks as usize,
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
