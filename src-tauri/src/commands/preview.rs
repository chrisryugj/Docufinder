//! 문서 미리보기 + 북마크 커맨드

use crate::db;
use crate::error::{ApiError, ApiResult};
use crate::AppContainer;
use serde::Serialize;
use std::sync::RwLock;
use tauri::State;

// ======================== 미리보기 ========================

/// 미리보기 청크 (프론트엔드용)
#[derive(Debug, Serialize)]
pub struct PreviewChunk {
    pub chunk_id: i64,
    pub chunk_index: i64,
    pub content: String,
    pub page_number: Option<i64>,
    pub location_hint: Option<String>,
}

/// 미리보기 응답
#[derive(Debug, Serialize)]
pub struct PreviewResponse {
    pub file_path: String,
    pub file_name: String,
    pub chunks: Vec<PreviewChunk>,
    pub total_chars: usize,
}

/// 파일 경로로 문서 전체 텍스트 로드 (미리보기용)
#[tauri::command]
pub async fn load_document_preview(
    file_path: String,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<PreviewResponse> {
    if file_path.trim().is_empty() {
        return Err(ApiError::Validation("파일 경로가 비어있습니다".to_string()));
    }

    let db_path = {
        let container = state.read()?;
        container.db_path.to_string_lossy().to_string()
    };

    let result = tokio::task::spawn_blocking(move || -> ApiResult<PreviewResponse> {
        let conn = db::get_connection(std::path::Path::new(&db_path))?;

        // 1. 파일 경로로 청크 ID 조회
        let chunk_ids = db::get_chunk_ids_for_path(&conn, &file_path)
            .map_err(|e| ApiError::DatabaseQuery(e.to_string()))?;

        if chunk_ids.is_empty() {
            return Ok(PreviewResponse {
                file_path: file_path.clone(),
                file_name: std::path::Path::new(&file_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string(),
                chunks: vec![],
                total_chars: 0,
            });
        }

        // 2. 청크 데이터 조회
        let chunk_infos = db::get_chunks_by_ids(&conn, &chunk_ids)
            .map_err(|e| ApiError::DatabaseQuery(e.to_string()))?;

        // 3. chunk_index 순 정렬
        let mut sorted = chunk_infos;
        sorted.sort_by_key(|c| c.chunk_index);

        let file_name = sorted
            .first()
            .map(|c| c.file_name.clone())
            .unwrap_or_default();

        let total_chars: usize = sorted.iter().map(|c| c.content.len()).sum();

        let chunks: Vec<PreviewChunk> = sorted
            .into_iter()
            .map(|c| PreviewChunk {
                chunk_id: c.chunk_id,
                chunk_index: c.chunk_index,
                content: c.content,
                page_number: c.page_number,
                location_hint: c.location_hint,
            })
            .collect();

        Ok(PreviewResponse {
            file_path,
            file_name,
            chunks,
            total_chars,
        })
    })
    .await??;

    Ok(result)
}

// ======================== 북마크 ========================

/// 북마크 정보 (프론트엔드용)
#[derive(Debug, Serialize)]
pub struct BookmarkInfo {
    pub id: i64,
    pub file_path: String,
    pub file_name: String,
    pub content_preview: String,
    pub page_number: Option<i64>,
    pub location_hint: Option<String>,
    pub note: Option<String>,
    pub created_at: i64,
}

/// 북마크 추가
#[tauri::command]
pub async fn add_bookmark(
    file_path: String,
    content_preview: String,
    page_number: Option<i64>,
    location_hint: Option<String>,
    note: Option<String>,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<i64> {
    if file_path.trim().is_empty() {
        return Err(ApiError::Validation("파일 경로가 비어있습니다".to_string()));
    }

    let db_path = {
        let container = state.read()?;
        container.db_path.to_string_lossy().to_string()
    };

    let result = tokio::task::spawn_blocking(move || -> ApiResult<i64> {
        let conn = db::get_connection(std::path::Path::new(&db_path))?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let file_name = std::path::Path::new(&file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        conn.execute(
            "INSERT OR REPLACE INTO bookmarks (file_path, file_name, content_preview, page_number, location_hint, note, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![file_path, file_name, content_preview, page_number, location_hint, note, now],
        )
        .map_err(|e| ApiError::DatabaseQuery(e.to_string()))?;

        let id = conn.last_insert_rowid();
        Ok(id)
    })
    .await??;

    Ok(result)
}

/// 북마크 삭제
#[tauri::command]
pub async fn remove_bookmark(
    id: i64,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<()> {
    let db_path = {
        let container = state.read()?;
        container.db_path.to_string_lossy().to_string()
    };

    tokio::task::spawn_blocking(move || -> ApiResult<()> {
        let conn = db::get_connection(std::path::Path::new(&db_path))?;
        conn.execute("DELETE FROM bookmarks WHERE id = ?", rusqlite::params![id])
            .map_err(|e| ApiError::DatabaseQuery(e.to_string()))?;
        Ok(())
    })
    .await??;

    Ok(())
}

/// 북마크 메모 수정
#[tauri::command]
pub async fn update_bookmark_note(
    id: i64,
    note: Option<String>,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<()> {
    let db_path = {
        let container = state.read()?;
        container.db_path.to_string_lossy().to_string()
    };

    tokio::task::spawn_blocking(move || -> ApiResult<()> {
        let conn = db::get_connection(std::path::Path::new(&db_path))?;
        conn.execute(
            "UPDATE bookmarks SET note = ? WHERE id = ?",
            rusqlite::params![note, id],
        )
        .map_err(|e| ApiError::DatabaseQuery(e.to_string()))?;
        Ok(())
    })
    .await??;

    Ok(())
}

/// 모든 북마크 조회
#[tauri::command]
pub async fn get_bookmarks(
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<Vec<BookmarkInfo>> {
    let db_path = {
        let container = state.read()?;
        container.db_path.to_string_lossy().to_string()
    };

    let result = tokio::task::spawn_blocking(move || -> ApiResult<Vec<BookmarkInfo>> {
        let conn = db::get_connection(std::path::Path::new(&db_path))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, file_path, file_name, content_preview, page_number, location_hint, note, created_at
                 FROM bookmarks ORDER BY created_at DESC",
            )
            .map_err(|e| ApiError::DatabaseQuery(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                Ok(BookmarkInfo {
                    id: row.get(0)?,
                    file_path: row.get(1)?,
                    file_name: row.get(2)?,
                    content_preview: row.get(3)?,
                    page_number: row.get(4)?,
                    location_hint: row.get(5)?,
                    note: row.get(6)?,
                    created_at: row.get(7)?,
                })
            })
            .map_err(|e| ApiError::DatabaseQuery(e.to_string()))?;

        let bookmarks: Vec<BookmarkInfo> = rows
            .filter_map(|r| r.ok())
            .collect();

        Ok(bookmarks)
    })
    .await??;

    Ok(result)
}
