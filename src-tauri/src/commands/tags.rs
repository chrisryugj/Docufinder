use crate::db;
use crate::error::{ApiError, ApiResult};
use crate::AppContainer;
use serde::Serialize;
use std::sync::RwLock;
use tauri::State;

#[derive(Debug, Serialize)]
pub struct TagInfo {
    pub tag: String,
    pub count: usize,
}

/// 파일에 태그 추가
#[tauri::command]
pub async fn add_file_tag(
    file_path: String,
    tag: String,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<()> {
    let tag = tag.trim().to_string();
    if tag.is_empty() || tag.len() > 50 {
        return Err(ApiError::Validation("태그는 1~50자여야 합니다".to_string()));
    }

    let db_path = {
        let container = state.read()?;
        container.db_path.to_string_lossy().to_string()
    };

    tokio::task::spawn_blocking(move || -> ApiResult<()> {
        let conn = db::get_connection(std::path::Path::new(&db_path))?;
        conn.execute(
            "INSERT OR IGNORE INTO file_tags (file_path, tag) VALUES (?1, ?2)",
            rusqlite::params![file_path, tag],
        )
        .map_err(|e| ApiError::DatabaseQuery(e.to_string()))?;
        Ok(())
    })
    .await??;

    Ok(())
}

/// 파일에서 태그 제거
#[tauri::command]
pub async fn remove_file_tag(
    file_path: String,
    tag: String,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<()> {
    let db_path = {
        let container = state.read()?;
        container.db_path.to_string_lossy().to_string()
    };

    tokio::task::spawn_blocking(move || -> ApiResult<()> {
        let conn = db::get_connection(std::path::Path::new(&db_path))?;
        conn.execute(
            "DELETE FROM file_tags WHERE file_path = ?1 AND tag = ?2",
            rusqlite::params![file_path, tag],
        )
        .map_err(|e| ApiError::DatabaseQuery(e.to_string()))?;
        Ok(())
    })
    .await??;

    Ok(())
}

/// 특정 파일의 태그 조회
#[tauri::command]
pub async fn get_file_tags(
    file_path: String,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<Vec<String>> {
    let db_path = {
        let container = state.read()?;
        container.db_path.to_string_lossy().to_string()
    };

    tokio::task::spawn_blocking(move || -> ApiResult<Vec<String>> {
        let conn = db::get_connection(std::path::Path::new(&db_path))?;
        let mut stmt = conn
            .prepare("SELECT tag FROM file_tags WHERE file_path = ?1 ORDER BY tag")
            .map_err(|e| ApiError::DatabaseQuery(e.to_string()))?;
        let tags: Vec<String> = stmt
            .query_map(rusqlite::params![file_path], |row| row.get::<_, String>(0))
            .map_err(|e| ApiError::DatabaseQuery(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(tags)
    })
    .await?
}

/// 전체 태그 목록 (사용 횟수 포함)
#[tauri::command]
pub async fn get_all_tags(state: State<'_, RwLock<AppContainer>>) -> ApiResult<Vec<TagInfo>> {
    let db_path = {
        let container = state.read()?;
        container.db_path.to_string_lossy().to_string()
    };

    tokio::task::spawn_blocking(move || -> ApiResult<Vec<TagInfo>> {
        let conn = db::get_connection(std::path::Path::new(&db_path))?;
        let mut stmt = conn
            .prepare(
                "SELECT tag, COUNT(*) as cnt FROM file_tags GROUP BY tag ORDER BY cnt DESC, tag",
            )
            .map_err(|e| ApiError::DatabaseQuery(e.to_string()))?;
        let tags: Vec<TagInfo> = stmt
            .query_map([], |row| {
                Ok(TagInfo {
                    tag: row.get(0)?,
                    count: row.get(1)?,
                })
            })
            .map_err(|e| ApiError::DatabaseQuery(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(tags)
    })
    .await?
}

/// 특정 태그가 붙은 파일 경로 목록
#[tauri::command]
pub async fn get_files_by_tag(
    tag: String,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<Vec<String>> {
    let db_path = {
        let container = state.read()?;
        container.db_path.to_string_lossy().to_string()
    };

    tokio::task::spawn_blocking(move || -> ApiResult<Vec<String>> {
        let conn = db::get_connection(std::path::Path::new(&db_path))?;
        let mut stmt = conn
            .prepare("SELECT file_path FROM file_tags WHERE tag = ?1 ORDER BY file_path")
            .map_err(|e| ApiError::DatabaseQuery(e.to_string()))?;
        let paths: Vec<String> = stmt
            .query_map(rusqlite::params![tag], |row| row.get::<_, String>(0))
            .map_err(|e| ApiError::DatabaseQuery(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(paths)
    })
    .await?
}
