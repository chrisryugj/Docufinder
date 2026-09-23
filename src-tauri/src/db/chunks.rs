//! 청크(chunks) 저장·FTS 인덱싱과 청크 단위 조회.

use super::escape_like_pattern;
use rusqlite::{params, Connection, Result};
use std::collections::HashMap;

// ==================== 청크 ====================

/// 파일의 기존 청크 삭제 - 트랜잭션 없음 (배치 파이프라인용)
///
/// 호출자가 이미 트랜잭션을 관리하는 경우 사용.
/// 중첩 BEGIN 방지로 배치 인덱싱 시 에러 해소.
pub fn delete_chunks_for_file_no_tx(conn: &Connection, file_id: i64) -> Result<()> {
    // 핫패스: prepare_cached로 SQL 재컴파일 방지 (배치 인덱싱 시 파일마다 호출)
    // FTS에서 먼저 삭제
    conn.prepare_cached(
        "DELETE FROM chunks_fts WHERE rowid IN (
            SELECT id FROM chunks WHERE file_id = ?
        )",
    )?
    .execute(params![file_id])?;

    conn.prepare_cached("DELETE FROM chunks WHERE file_id = ?")?
        .execute(params![file_id])?;
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
    // 핫패스: prepare_cached로 SQL 재컴파일 방지 (청크마다 호출)
    conn.prepare_cached(
        "INSERT INTO chunks (file_id, chunk_index, start_offset, end_offset, page_number, page_end, location_hint, content)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )?
    .execute(params![
        file_id,
        chunk_index as i64,
        start_offset as i64,
        end_offset as i64,
        page_number.map(|p| p as i64),
        page_end.map(|p| p as i64),
        location_hint,
        content
    ])?;

    let chunk_id = conn.last_insert_rowid();

    // FTS 인덱싱 (원본 content + 형태소 토큰)
    let fts_content = match fts_extra_tokens {
        Some(tokens) if !tokens.is_empty() => format!("{} {}", content, tokens),
        _ => content.to_string(),
    };
    conn.prepare_cached("INSERT INTO chunks_fts (rowid, content) VALUES (?, ?)")?
        .execute(params![chunk_id, fts_content])?;

    Ok(chunk_id)
}

// ==================== 청크 조회 ====================

/// SQLite SQLITE_MAX_VARIABLE_NUMBER 한계(32766) 방지를 위한 청크 크기
const SQL_BATCH_SIZE: usize = 500;

/// 여러 chunk_id로 청크 정보 일괄 조회
/// 대용량(500개 초과) 시 자동 분할하여 SQLITE_MAX_VARIABLE_NUMBER 한계를 방지한다.
pub fn get_chunks_by_ids(conn: &Connection, chunk_ids: &[i64]) -> Result<Vec<ChunkInfo>> {
    if chunk_ids.is_empty() {
        return Ok(vec![]);
    }

    let mut all_results = Vec::with_capacity(chunk_ids.len());
    for batch in chunk_ids.chunks(SQL_BATCH_SIZE) {
        let placeholders: String = batch.iter().map(|_| "?").collect::<Vec<_>>().join(",");
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
        let params: Vec<&dyn rusqlite::ToSql> =
            batch.iter().map(|id| id as &dyn rusqlite::ToSql).collect();

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

        for row in results {
            all_results.push(row?);
        }
    }
    Ok(all_results)
}

/// 파일 경로 + 청크 인덱스 목록으로 청크 일괄 조회 (RAG 이웃 청크 확장용)
/// 대용량 시 자동 분할.
///
/// 유일한 호출자가 `commands::ai` 라 lite(내부망) 빌드에는 컴파일하지 않는다.
#[cfg(feature = "online")]
pub fn get_chunks_for_file_indices(
    conn: &Connection,
    file_path: &str,
    chunk_indices: &[i64],
) -> Result<Vec<ChunkInfo>> {
    if chunk_indices.is_empty() {
        return Ok(vec![]);
    }

    // file_path가 ?1을 차지하므로 배치 크기를 1 줄임
    let batch_size = SQL_BATCH_SIZE - 1;
    let mut all_results = Vec::with_capacity(chunk_indices.len());
    for batch in chunk_indices.chunks(batch_size) {
        let placeholders: String = batch.iter().map(|_| "?").collect::<Vec<_>>().join(",");
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
        let mut params_vec: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(batch.len() + 1);
        params_vec.push(&file_path);
        for idx in batch {
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

        for row in results {
            all_results.push(row?);
        }
    }
    Ok(all_results)
}

/// 청크 ID → 파일 경로 경량 조회 (content 없이 경로만 — 벡터 스코프 프리필터용)
/// 대용량 시 자동 분할.
pub fn get_chunk_file_paths(conn: &Connection, chunk_ids: &[i64]) -> Result<HashMap<i64, String>> {
    if chunk_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let mut map = HashMap::with_capacity(chunk_ids.len());
    for batch in chunk_ids.chunks(SQL_BATCH_SIZE) {
        let placeholders: String = batch.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT c.id, f.path FROM chunks c JOIN files f ON f.id = c.file_id WHERE c.id IN ({})",
            placeholders
        );

        let mut stmt = conn.prepare(&sql)?;
        let params: Vec<&dyn rusqlite::ToSql> =
            batch.iter().map(|id| id as &dyn rusqlite::ToSql).collect();

        let rows = stmt.query_map(params.as_slice(), |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (id, path) = row?;
            map.insert(id, path);
        }
    }
    Ok(map)
}

/// 파일 경로 목록 → 파일별 전체 청크 수 batch 조회 (히트맵 절대 스케일용)
/// 대용량 시 자동 분할.
pub fn get_chunk_counts_by_file_paths(
    conn: &Connection,
    file_paths: &[String],
) -> Result<HashMap<String, i64>> {
    if file_paths.is_empty() {
        return Ok(HashMap::new());
    }

    let mut map = HashMap::with_capacity(file_paths.len());
    for batch in file_paths.chunks(SQL_BATCH_SIZE) {
        let placeholders: String = batch.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT f.path, COUNT(c.id)
             FROM files f
             JOIN chunks c ON c.file_id = f.id
             WHERE f.path IN ({})
             GROUP BY f.id",
            placeholders
        );

        let mut stmt = conn.prepare(&sql)?;
        let params: Vec<&dyn rusqlite::ToSql> =
            batch.iter().map(|p| p as &dyn rusqlite::ToSql).collect();

        let rows = stmt.query_map(params.as_slice(), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        for row in rows {
            let (path, count) = row?;
            map.insert(path, count);
        }
    }
    Ok(map)
}

/// 파일 경로 목록 → 파일별 garbled(복사 시 깨짐) 플래그 batch 조회 (검색 결과 배지용).
/// 대용량 시 자동 분할. 결과에 없는 경로는 호출부에서 기본값(false)으로 남긴다.
pub fn get_garbled_flags(
    conn: &Connection,
    file_paths: &[String],
) -> Result<HashMap<String, bool>> {
    if file_paths.is_empty() {
        return Ok(HashMap::new());
    }

    let mut map = HashMap::with_capacity(file_paths.len());
    for batch in file_paths.chunks(SQL_BATCH_SIZE) {
        let placeholders: String = batch.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT path, garbled FROM files WHERE path IN ({})",
            placeholders
        );

        let mut stmt = conn.prepare(&sql)?;
        let params: Vec<&dyn rusqlite::ToSql> =
            batch.iter().map(|p| p as &dyn rusqlite::ToSql).collect();

        let rows = stmt.query_map(params.as_slice(), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? != 0))
        })?;
        for row in rows {
            let (path, garbled) = row?;
            map.insert(path, garbled);
        }
    }
    Ok(map)
}

/// Document Lineage 정보 — 검색 결과 enrichment용.
#[derive(Debug, Clone, Default)]
pub struct LineageInfo {
    pub lineage_id: Option<String>,
    pub lineage_role: Option<String>,
    pub version_label: Option<String>,
    pub version_count: i64,
}

/// 주어진 파일 경로들의 lineage 정보를 한 번에 조회한다.
/// 각 파일의 (lineage_id, role, version_label) + 그 lineage의 전체 멤버 수를 반환.
pub fn get_lineage_info_by_file_paths(
    conn: &Connection,
    file_paths: &[String],
) -> Result<HashMap<String, LineageInfo>> {
    if file_paths.is_empty() {
        return Ok(HashMap::new());
    }

    // 1단계: path별 기본 lineage 정보
    type LineageTriple = (Option<String>, Option<String>, Option<String>);
    let mut raw: HashMap<String, LineageTriple> = HashMap::with_capacity(file_paths.len());
    for batch in file_paths.chunks(SQL_BATCH_SIZE) {
        let ph: String = batch.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT path, lineage_id, lineage_role, version_label FROM files WHERE path IN ({})",
            ph
        );
        let mut stmt = conn.prepare(&sql)?;
        let params: Vec<&dyn rusqlite::ToSql> =
            batch.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
        let rows = stmt.query_map(params.as_slice(), |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        })?;
        for row in rows {
            let (path, lid, role, label) = row?;
            raw.insert(path, (lid, role, label));
        }
    }

    // 2단계: 등장한 lineage_id들의 멤버 수
    let lineage_ids: std::collections::HashSet<String> =
        raw.values().filter_map(|(lid, _, _)| lid.clone()).collect();
    let ids_vec: Vec<String> = lineage_ids.into_iter().collect();

    let mut counts: HashMap<String, i64> = HashMap::with_capacity(ids_vec.len());
    for batch in ids_vec.chunks(SQL_BATCH_SIZE) {
        let ph: String = batch.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT lineage_id, COUNT(*) FROM files WHERE lineage_id IN ({}) GROUP BY lineage_id",
            ph
        );
        let mut stmt = conn.prepare(&sql)?;
        let params: Vec<&dyn rusqlite::ToSql> =
            batch.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
        let rows = stmt.query_map(params.as_slice(), |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })?;
        for row in rows {
            let (lid, c) = row?;
            counts.insert(lid, c);
        }
    }

    // 3단계: 조립
    let mut out: HashMap<String, LineageInfo> = HashMap::with_capacity(raw.len());
    for (path, (lid, role, label)) in raw {
        let count = lid
            .as_ref()
            .and_then(|l| counts.get(l))
            .copied()
            .unwrap_or(0);
        out.insert(
            path,
            LineageInfo {
                lineage_id: lid,
                lineage_role: role,
                version_label: label,
                version_count: count,
            },
        );
    }
    Ok(out)
}

/// 파일 ID로 chunk ID들 조회 (벡터 인덱스 삭제용 — 재인덱싱 경로)
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
