use rusqlite::{params, Connection, Result};
use std::path::Path;

use super::pool::get_connection;

// ==================== 스키마 마이그레이션 ====================

/// 현재 스키마 버전
const CURRENT_SCHEMA_VERSION: i32 = 11;

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

    // auto_vacuum은 테이블 생성 전에만 설정 가능 (새 DB에서만 효과)
    // 기존 DB에서는 무시되므로 항상 호출해도 안전
    conn.execute_batch("PRAGMA auto_vacuum = INCREMENTAL;")?;

    migrate_schema(&conn, db_path)
}

/// 기존 Connection으로 스키마 마이그레이션 (clear_all_data에서도 사용)
pub fn migrate_schema(conn: &Connection, db_path: &Path) -> Result<()> {
    // 스키마 버전 테이블 (항상 먼저 생성)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS schema_version (
            id INTEGER PRIMARY KEY,
            version INTEGER NOT NULL
        )",
        [],
    )?;

    let current_version = get_schema_version(conn);

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

        set_schema_version(conn, 1)?;
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
        set_schema_version(conn, 2)?;
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
        set_schema_version(conn, 3)?;
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

        set_schema_version(conn, 4)?;
        tracing::info!("Schema migrated to v4 (two-phase indexing)");
    }

    // === v5: page_end 컬럼 ===
    if current_version < 5 {
        if let Err(e) = conn.execute("ALTER TABLE chunks ADD COLUMN page_end INTEGER", []) {
            tracing::trace!("Migration v5: page_end already exists: {}", e);
        }
        set_schema_version(conn, 5)?;
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
        set_schema_version(conn, 6)?;
        tracing::info!("Schema migrated to v6 (last_synced_at)");
    }

    // === v7: 북마크 테이블 ===
    if current_version < 7 {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS bookmarks (
                id INTEGER PRIMARY KEY,
                file_path TEXT NOT NULL,
                file_name TEXT NOT NULL,
                content_preview TEXT NOT NULL DEFAULT '',
                page_number INTEGER,
                location_hint TEXT,
                note TEXT,
                created_at INTEGER NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bookmarks_file_path ON bookmarks(file_path)",
            [],
        )?;
        set_schema_version(conn, 7)?;
        tracing::info!("Schema migrated to v7 (bookmarks)");
    }

    // === v8: 검색어 자동완성 (fts5vocab + search_queries) ===
    if current_version < 8 {
        // fts5vocab: 인덱싱된 용어 빈도 조회용 가상 테이블
        conn.execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts_vocab USING fts5vocab(chunks_fts, 'row')",
            [],
        )?;

        // 검색어 히스토리 (빈도 추적)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS search_queries (
                id INTEGER PRIMARY KEY,
                query TEXT UNIQUE NOT NULL,
                frequency INTEGER NOT NULL DEFAULT 1,
                last_searched_at INTEGER NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_search_queries_freq ON search_queries(frequency DESC)",
            [],
        )?;

        set_schema_version(conn, 8)?;
        tracing::info!("Schema migrated to v8 (autocomplete: fts5vocab + search_queries)");
    }

    // === v9: 북마크 중복 방지 (file_path UNIQUE) ===
    if current_version < 9 {
        // 기존 중복 북마크 정리 (가장 최근 것만 유지)
        conn.execute(
            "DELETE FROM bookmarks WHERE id NOT IN (
                SELECT MAX(id) FROM bookmarks GROUP BY file_path
            )",
            [],
        )?;
        // 기존 일반 인덱스 제거 후 UNIQUE 인덱스 생성
        conn.execute("DROP INDEX IF EXISTS idx_bookmarks_file_path", [])?;
        conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_bookmarks_file_path ON bookmarks(file_path)",
            [],
        )?;
        set_schema_version(conn, 9)?;
        tracing::info!("Schema migrated to v9 (bookmark unique constraint)");
    }

    // v10: 파일 태그 시스템
    if get_schema_version(conn) < 10 {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS file_tags (
                id INTEGER PRIMARY KEY,
                file_path TEXT NOT NULL,
                tag TEXT NOT NULL,
                created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
            )",
            [],
        )?;
        conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_file_tags_path_tag ON file_tags(file_path, tag)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_file_tags_tag ON file_tags(tag)",
            [],
        )?;
        set_schema_version(conn, 10)?;
        tracing::info!("Schema migrated to v10 (file tags)");
    }

    // === v11: chunks 테이블에 원본 content 컬럼 추가 ===
    // FTS 테이블에는 형태소 토큰이 추가된 텍스트가 저장되므로
    // 미리보기용 원본 텍스트를 별도 보관
    if get_schema_version(conn) < 11 {
        if let Err(e) = conn.execute("ALTER TABLE chunks ADD COLUMN content TEXT", []) {
            tracing::trace!("Migration v11: content column already exists: {}", e);
        }
        set_schema_version(conn, 11)?;
        tracing::info!("Schema migrated to v11 (chunks.content for preview)");
    }

    tracing::info!(
        "Database initialized at {:?} (schema v{})",
        db_path,
        CURRENT_SCHEMA_VERSION
    );
    Ok(())
}
