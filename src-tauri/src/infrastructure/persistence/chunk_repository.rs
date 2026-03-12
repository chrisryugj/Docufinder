//! SqliteChunkRepository - ChunkRepository trait의 SQLite 구현체

use crate::domain::entities::Chunk;
use crate::domain::errors::DomainError;
use crate::domain::repositories::{ChunkRepository, FtsSearchResult};
use crate::domain::value_objects::{ChunkId, FileId};
use async_trait::async_trait;
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;

/// SQLite 기반 청크 리포지토리
pub struct SqliteChunkRepository {
    conn: Mutex<Connection>,
}

impl SqliteChunkRepository {
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

}

#[async_trait]
impl ChunkRepository for SqliteChunkRepository {
    async fn save(&self, chunk: &mut Chunk) -> Result<ChunkId, DomainError> {
        let file_id = chunk.file_id().value();
        let chunk_index = chunk.chunk_index() as i64;
        let content = chunk.content().to_string();
        let start_offset = chunk.start_offset() as i64;
        let end_offset = chunk.end_offset() as i64;
        let page_number = chunk.page_number().map(|p| p as i64);
        let location_hint = chunk.location_hint().map(|s| s.to_string());

        let chunk_id = self.with_conn(|conn| {
            // 청크 메타데이터 저장
            conn.execute(
                "INSERT INTO chunks (file_id, chunk_index, start_offset, end_offset, page_number, location_hint)
                 VALUES (?, ?, ?, ?, ?, ?)",
                params![file_id, chunk_index, start_offset, end_offset, page_number, location_hint],
            )?;

            let id = conn.last_insert_rowid();

            // FTS 인덱싱
            conn.execute(
                "INSERT INTO chunks_fts (rowid, content) VALUES (?, ?)",
                params![id, content],
            )?;

            Ok(id)
        })?;

        let id = ChunkId::new(chunk_id);
        chunk.set_id(id);

        Ok(id)
    }

    async fn save_batch(&self, chunks: &mut [Chunk]) -> Result<Vec<ChunkId>, DomainError> {
        if chunks.is_empty() {
            return Ok(vec![]);
        }

        let mut ids = Vec::with_capacity(chunks.len());

        self.with_conn(|conn| {
            conn.execute("BEGIN TRANSACTION", [])?;

            for chunk in chunks.iter_mut() {
                // 청크 메타데이터 저장
                conn.execute(
                    "INSERT INTO chunks (file_id, chunk_index, start_offset, end_offset, page_number, location_hint)
                     VALUES (?, ?, ?, ?, ?, ?)",
                    params![
                        chunk.file_id().value(),
                        chunk.chunk_index() as i64,
                        chunk.start_offset() as i64,
                        chunk.end_offset() as i64,
                        chunk.page_number().map(|p| p as i64),
                        chunk.location_hint()
                    ],
                )?;

                let id = conn.last_insert_rowid();

                // FTS 인덱싱
                conn.execute(
                    "INSERT INTO chunks_fts (rowid, content) VALUES (?, ?)",
                    params![id, chunk.content()],
                )?;

                let chunk_id = ChunkId::new(id);
                chunk.set_id(chunk_id);
                ids.push(chunk_id);
            }

            conn.execute("COMMIT", [])?;
            Ok(())
        })?;

        Ok(ids)
    }

    async fn find_by_id(&self, id: ChunkId) -> Result<Option<Chunk>, DomainError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT c.id, c.file_id, c.chunk_index, fts.content, c.start_offset, c.end_offset, c.page_number, c.location_hint
                 FROM chunks c
                 JOIN chunks_fts fts ON fts.rowid = c.id
                 WHERE c.id = ?"
            )?;

            let result = stmt.query_row(params![id.value()], |row| {
                let id: i64 = row.get(0)?;
                let file_id: i64 = row.get(1)?;
                let chunk_index: i64 = row.get(2)?;
                let content: String = row.get(3)?;
                let start_offset: i64 = row.get(4)?;
                let end_offset: i64 = row.get(5)?;
                let page_number: Option<i64> = row.get(6)?;
                let location_hint: Option<String> = row.get(7)?;

                Ok(Chunk::reconstitute(
                    ChunkId::new(id),
                    FileId::new(file_id),
                    chunk_index as usize,
                    content,
                    start_offset as usize,
                    end_offset as usize,
                    page_number.map(|p| p as usize),
                    location_hint,
                ))
            });

            match result {
                Ok(chunk) => Ok(Some(chunk)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        })
    }

    async fn find_by_ids(&self, ids: &[ChunkId]) -> Result<Vec<Chunk>, DomainError> {
        if ids.is_empty() {
            return Ok(vec![]);
        }

        let placeholders: String = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT c.id, c.file_id, c.chunk_index, fts.content, c.start_offset, c.end_offset, c.page_number, c.location_hint
             FROM chunks c
             JOIN chunks_fts fts ON fts.rowid = c.id
             WHERE c.id IN ({})",
            placeholders
        );

        self.with_conn(|conn| {
            let mut stmt = conn.prepare(&sql)?;
            let params: Vec<i64> = ids.iter().map(|id| id.value()).collect();
            let params_ref: Vec<&dyn rusqlite::ToSql> =
                params.iter().map(|p| p as &dyn rusqlite::ToSql).collect();

            let rows = stmt.query_map(params_ref.as_slice(), |row| {
                let id: i64 = row.get(0)?;
                let file_id: i64 = row.get(1)?;
                let chunk_index: i64 = row.get(2)?;
                let content: String = row.get(3)?;
                let start_offset: i64 = row.get(4)?;
                let end_offset: i64 = row.get(5)?;
                let page_number: Option<i64> = row.get(6)?;
                let location_hint: Option<String> = row.get(7)?;

                Ok(Chunk::reconstitute(
                    ChunkId::new(id),
                    FileId::new(file_id),
                    chunk_index as usize,
                    content,
                    start_offset as usize,
                    end_offset as usize,
                    page_number.map(|p| p as usize),
                    location_hint,
                ))
            })?;

            rows.collect()
        })
    }

    async fn find_by_file_id(&self, file_id: FileId) -> Result<Vec<Chunk>, DomainError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT c.id, c.file_id, c.chunk_index, fts.content, c.start_offset, c.end_offset, c.page_number, c.location_hint
                 FROM chunks c
                 JOIN chunks_fts fts ON fts.rowid = c.id
                 WHERE c.file_id = ?
                 ORDER BY c.chunk_index"
            )?;

            let rows = stmt.query_map(params![file_id.value()], |row| {
                let id: i64 = row.get(0)?;
                let file_id: i64 = row.get(1)?;
                let chunk_index: i64 = row.get(2)?;
                let content: String = row.get(3)?;
                let start_offset: i64 = row.get(4)?;
                let end_offset: i64 = row.get(5)?;
                let page_number: Option<i64> = row.get(6)?;
                let location_hint: Option<String> = row.get(7)?;

                Ok(Chunk::reconstitute(
                    ChunkId::new(id),
                    FileId::new(file_id),
                    chunk_index as usize,
                    content,
                    start_offset as usize,
                    end_offset as usize,
                    page_number.map(|p| p as usize),
                    location_hint,
                ))
            })?;

            rows.collect()
        })
    }

    async fn delete(&self, id: ChunkId) -> Result<(), DomainError> {
        self.with_conn(|conn| {
            // FTS 삭제
            conn.execute(
                "DELETE FROM chunks_fts WHERE rowid = ?",
                params![id.value()],
            )?;
            // 청크 삭제
            conn.execute("DELETE FROM chunks WHERE id = ?", params![id.value()])?;
            Ok(())
        })
    }

    async fn delete_by_file_id(&self, file_id: FileId) -> Result<usize, DomainError> {
        self.with_conn(|conn| {
            // FTS 삭제
            conn.execute(
                "DELETE FROM chunks_fts WHERE rowid IN (SELECT id FROM chunks WHERE file_id = ?)",
                params![file_id.value()],
            )?;
            // 청크 삭제
            let deleted = conn.execute(
                "DELETE FROM chunks WHERE file_id = ?",
                params![file_id.value()],
            )?;
            Ok(deleted)
        })
    }

    async fn delete_in_folder(&self, folder_path: &str) -> Result<usize, DomainError> {
        let escaped_unix = crate::db::escape_like_pattern(&folder_path.replace('\\', "/"));
        let escaped_win = crate::db::escape_like_pattern(&folder_path.replace('/', "\\"));
        let pattern_unix = format!("{}/%", escaped_unix);
        let pattern_win = format!("{}\\\\%", escaped_win);

        self.with_conn(|conn| {
            // FTS 삭제
            conn.execute(
                "DELETE FROM chunks_fts WHERE rowid IN (
                    SELECT c.id FROM chunks c
                    JOIN files f ON c.file_id = f.id
                    WHERE f.path LIKE ? ESCAPE '\\' OR f.path LIKE ? ESCAPE '\\'
                )",
                params![pattern_unix, pattern_win],
            )?;

            // 청크 삭제
            let deleted = conn.execute(
                "DELETE FROM chunks WHERE file_id IN (
                    SELECT id FROM files
                    WHERE path LIKE ? ESCAPE '\\' OR path LIKE ? ESCAPE '\\'
                )",
                params![pattern_unix, pattern_win],
            )?;

            Ok(deleted)
        })
    }

    async fn search_fts(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<FtsSearchResult>, DomainError> {
        if query.trim().is_empty() {
            return Ok(vec![]);
        }

        // FTS5 쿼리 이스케이프: 특수문자를 큰따옴표로 감싸기
        let escaped_query = format!("\"{}\"", query.replace('"', "\"\""));

        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT c.id, c.file_id, fts.content, bm25(chunks_fts) as score
                 FROM chunks_fts fts
                 JOIN chunks c ON c.id = fts.rowid
                 WHERE chunks_fts MATCH ?
                 ORDER BY score
                 LIMIT ?",
            )?;

            let rows = stmt.query_map(params![escaped_query, limit as i64], |row| {
                let chunk_id: i64 = row.get(0)?;
                let file_id: i64 = row.get(1)?;
                let content: String = row.get(2)?;
                let score: f64 = row.get(3)?;

                Ok((chunk_id, file_id, content, score))
            })?;

            let mut results = Vec::new();
            let query_lower = query.to_lowercase();

            for row in rows {
                let (chunk_id, file_id, content, score) = row?;

                // 하이라이트 범위 계산
                let content_lower = content.to_lowercase();
                let mut highlight_ranges = Vec::new();
                let mut start = 0;

                while let Some(pos) = content_lower[start..].find(&query_lower) {
                    let actual_pos = start + pos;
                    highlight_ranges.push((actual_pos, actual_pos + query.len()));
                    start = actual_pos + 1;
                }

                results.push(FtsSearchResult {
                    chunk_id: ChunkId::new(chunk_id),
                    file_id: FileId::new(file_id),
                    content,
                    score: (-score) as f32, // BM25 점수는 음수, 양수로 변환
                    highlight_ranges,
                });
            }

            Ok(results)
        })
    }

    async fn count(&self) -> Result<usize, DomainError> {
        self.with_conn(|conn| {
            let count: i64 = conn.query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))?;
            Ok(count as usize)
        })
    }

    async fn count_by_file_id(&self, file_id: FileId) -> Result<usize, DomainError> {
        self.with_conn(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM chunks WHERE file_id = ?",
                params![file_id.value()],
                |row| row.get(0),
            )?;
            Ok(count as usize)
        })
    }
}
