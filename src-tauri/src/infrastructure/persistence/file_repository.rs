//! SqliteFileRepository - FileRepository trait의 SQLite 구현체

use crate::domain::entities::{File, FileType};
use crate::domain::errors::DomainError;
use crate::domain::repositories::FileRepository;
use crate::domain::value_objects::FileId;
use async_trait::async_trait;
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// SQLite 기반 파일 리포지토리
pub struct SqliteFileRepository {
    conn: Mutex<Connection>,
}

impl SqliteFileRepository {
    /// 새 리포지토리 생성
    pub fn new(db_path: &Path) -> Result<Self, DomainError> {
        let conn = crate::db::get_connection(db_path)
            .map_err(|e| DomainError::repository(format!("DB open failed: {}", e)))?
            .into_inner(); // 풀에서 분리 (Repository가 장기 보유)

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// 커넥션 참조 반환 (내부 사용)
    fn with_conn<F, T>(&self, f: F) -> Result<T, DomainError>
    where
        F: FnOnce(&Connection) -> Result<T, rusqlite::Error>,
    {
        let conn = self
            .conn
            .lock()
            .map_err(|e| DomainError::repository(format!("Lock failed: {}", e)))?;

        f(&conn).map_err(|e| DomainError::repository(format!("Query failed: {}", e)))
    }

    /// 현재 타임스탬프
    fn now() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
    }

    /// 문자열을 FileType으로 변환
    fn str_to_file_type(s: &str) -> FileType {
        match s {
            "hwpx" => FileType::Hwpx,
            "docx" => FileType::Docx,
            "xlsx" => FileType::Xlsx,
            "pdf" => FileType::Pdf,
            "txt" => FileType::Txt,
            _ => FileType::Unknown,
        }
    }

}

#[async_trait]
impl FileRepository for SqliteFileRepository {
    async fn save(&self, file: &mut File) -> Result<FileId, DomainError> {
        let now = Self::now();
        let path = file.path().to_string();
        let name = file.name().to_string();
        let file_type = file.file_type().as_str().to_string();
        let size = file.size();
        let modified_at = file.modified_at();

        let file_id = self.with_conn(|conn| {
            // FTS만 인덱싱된 상태로 저장 (벡터 인덱싱 대기)
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

            let id: i64 = conn.query_row(
                "SELECT id FROM files WHERE path = ?",
                params![path],
                |row| row.get(0),
            )?;

            // files_fts 인덱스 갱신
            conn.execute("DELETE FROM files_fts WHERE rowid = ?", params![id])?;
            conn.execute(
                "INSERT INTO files_fts (rowid, name) VALUES (?, ?)",
                params![id, name],
            )?;

            Ok(id)
        })?;

        let id = FileId::new(file_id);
        file.set_id(id);
        file.mark_fts_indexed(now);

        Ok(id)
    }

    async fn find_by_id(&self, id: FileId) -> Result<Option<File>, DomainError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, path, name, file_type, size, modified_at, fts_indexed_at, vector_indexed_at
                 FROM files WHERE id = ?"
            )?;

            let result = stmt.query_row(params![id.value()], |row| {
                let id: i64 = row.get(0)?;
                let path: String = row.get(1)?;
                let name: String = row.get(2)?;
                let file_type: String = row.get(3)?;
                let size: i64 = row.get(4)?;
                let modified_at: i64 = row.get(5)?;
                let fts_indexed_at: Option<i64> = row.get(6)?;
                let vector_indexed_at: Option<i64> = row.get(7)?;

                Ok(File::reconstitute(
                    FileId::new(id),
                    path,
                    name,
                    Self::str_to_file_type(&file_type),
                    size,
                    modified_at,
                    fts_indexed_at,
                    vector_indexed_at,
                ))
            });

            match result {
                Ok(file) => Ok(Some(file)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        })
    }

    async fn find_by_path(&self, path: &str) -> Result<Option<File>, DomainError> {
        let path = path.to_string();
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, path, name, file_type, size, modified_at, fts_indexed_at, vector_indexed_at
                 FROM files WHERE path = ?"
            )?;

            let result = stmt.query_row(params![path], |row| {
                let id: i64 = row.get(0)?;
                let path: String = row.get(1)?;
                let name: String = row.get(2)?;
                let file_type: String = row.get(3)?;
                let size: i64 = row.get(4)?;
                let modified_at: i64 = row.get(5)?;
                let fts_indexed_at: Option<i64> = row.get(6)?;
                let vector_indexed_at: Option<i64> = row.get(7)?;

                Ok(File::reconstitute(
                    FileId::new(id),
                    path,
                    name,
                    Self::str_to_file_type(&file_type),
                    size,
                    modified_at,
                    fts_indexed_at,
                    vector_indexed_at,
                ))
            });

            match result {
                Ok(file) => Ok(Some(file)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        })
    }

    async fn find_in_folder(&self, folder_path: &str) -> Result<Vec<File>, DomainError> {
        let escaped_unix = crate::db::escape_like_pattern(&folder_path.replace('\\', "/"));
        let escaped_win = crate::db::escape_like_pattern(&folder_path.replace('/', "\\"));
        let pattern_unix = format!("{}/%", escaped_unix);
        let pattern_win = format!("{}\\\\%", escaped_win);

        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, path, name, file_type, size, modified_at, fts_indexed_at, vector_indexed_at
                 FROM files WHERE path LIKE ? ESCAPE '\\' OR path LIKE ? ESCAPE '\\'"
            )?;

            let rows = stmt.query_map(params![pattern_unix, pattern_win], |row| {
                let id: i64 = row.get(0)?;
                let path: String = row.get(1)?;
                let name: String = row.get(2)?;
                let file_type: String = row.get(3)?;
                let size: i64 = row.get(4)?;
                let modified_at: i64 = row.get(5)?;
                let fts_indexed_at: Option<i64> = row.get(6)?;
                let vector_indexed_at: Option<i64> = row.get(7)?;

                Ok(File::reconstitute(
                    FileId::new(id),
                    path,
                    name,
                    Self::str_to_file_type(&file_type),
                    size,
                    modified_at,
                    fts_indexed_at,
                    vector_indexed_at,
                ))
            })?;

            rows.collect()
        })
    }

    async fn find_pending_vector_files(&self, limit: usize) -> Result<Vec<File>, DomainError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, path, name, file_type, size, modified_at, fts_indexed_at, vector_indexed_at
                 FROM files
                 WHERE fts_indexed_at IS NOT NULL AND vector_indexed_at IS NULL
                 ORDER BY id
                 LIMIT ?"
            )?;

            let rows = stmt.query_map(params![limit as i64], |row| {
                let id: i64 = row.get(0)?;
                let path: String = row.get(1)?;
                let name: String = row.get(2)?;
                let file_type: String = row.get(3)?;
                let size: i64 = row.get(4)?;
                let modified_at: i64 = row.get(5)?;
                let fts_indexed_at: Option<i64> = row.get(6)?;
                let vector_indexed_at: Option<i64> = row.get(7)?;

                Ok(File::reconstitute(
                    FileId::new(id),
                    path,
                    name,
                    Self::str_to_file_type(&file_type),
                    size,
                    modified_at,
                    fts_indexed_at,
                    vector_indexed_at,
                ))
            })?;

            rows.collect()
        })
    }

    async fn delete(&self, id: FileId) -> Result<(), DomainError> {
        self.with_conn(|conn| {
            // chunks_fts 삭제
            conn.execute(
                "DELETE FROM chunks_fts WHERE rowid IN (SELECT id FROM chunks WHERE file_id = ?)",
                params![id.value()],
            )?;

            // files_fts 삭제
            conn.execute("DELETE FROM files_fts WHERE rowid = ?", params![id.value()])?;

            // chunks 삭제 (CASCADE가 있지만 명시적으로)
            conn.execute("DELETE FROM chunks WHERE file_id = ?", params![id.value()])?;

            // files 삭제
            conn.execute("DELETE FROM files WHERE id = ?", params![id.value()])?;

            Ok(())
        })
    }

    async fn delete_by_path(&self, path: &str) -> Result<(), DomainError> {
        let path = path.to_string();
        self.with_conn(|conn| {
            // chunks_fts 삭제
            conn.execute(
                "DELETE FROM chunks_fts WHERE rowid IN (
                    SELECT c.id FROM chunks c JOIN files f ON c.file_id = f.id WHERE f.path = ?
                )",
                params![path],
            )?;

            // files_fts 삭제
            conn.execute(
                "DELETE FROM files_fts WHERE rowid IN (SELECT id FROM files WHERE path = ?)",
                params![path],
            )?;

            // chunks 삭제
            conn.execute(
                "DELETE FROM chunks WHERE file_id IN (SELECT id FROM files WHERE path = ?)",
                params![path],
            )?;

            // files 삭제
            conn.execute("DELETE FROM files WHERE path = ?", params![path])?;

            Ok(())
        })
    }

    async fn delete_in_folder(&self, folder_path: &str) -> Result<usize, DomainError> {
        let escaped_unix = crate::db::escape_like_pattern(&folder_path.replace('\\', "/"));
        let escaped_win = crate::db::escape_like_pattern(&folder_path.replace('/', "\\"));
        let pattern_unix = format!("{}/%", escaped_unix);
        let pattern_win = format!("{}\\\\%", escaped_win);

        self.with_conn(|conn| {
            // chunks_fts 삭제
            conn.execute(
                "DELETE FROM chunks_fts WHERE rowid IN (
                    SELECT c.id FROM chunks c JOIN files f ON c.file_id = f.id
                    WHERE f.path LIKE ? ESCAPE '\\' OR f.path LIKE ? ESCAPE '\\'
                )",
                params![pattern_unix, pattern_win],
            )?;

            // files_fts 삭제
            conn.execute(
                "DELETE FROM files_fts WHERE rowid IN (
                    SELECT id FROM files WHERE path LIKE ? ESCAPE '\\' OR path LIKE ? ESCAPE '\\'
                )",
                params![pattern_unix, pattern_win],
            )?;

            // files 삭제 (chunks는 CASCADE)
            let deleted = conn.execute(
                "DELETE FROM files WHERE path LIKE ? ESCAPE '\\' OR path LIKE ? ESCAPE '\\'",
                params![pattern_unix, pattern_win],
            )?;

            Ok(deleted)
        })
    }

    async fn count(&self) -> Result<usize, DomainError> {
        self.with_conn(|conn| {
            let count: i64 = conn.query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))?;
            Ok(count as usize)
        })
    }

    async fn mark_fts_indexed(&self, id: FileId, timestamp: i64) -> Result<(), DomainError> {
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE files SET fts_indexed_at = ? WHERE id = ?",
                params![timestamp, id.value()],
            )?;
            Ok(())
        })
    }

    async fn mark_vector_indexed(&self, id: FileId, timestamp: i64) -> Result<(), DomainError> {
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE files SET vector_indexed_at = ? WHERE id = ?",
                params![timestamp, id.value()],
            )?;
            Ok(())
        })
    }

    async fn exists(&self, path: &str) -> Result<bool, DomainError> {
        let path = path.to_string();
        self.with_conn(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM files WHERE path = ?",
                params![path],
                |row| row.get(0),
            )?;
            Ok(count > 0)
        })
    }
}
