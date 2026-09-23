//! 감시 폴더(watched_folders) 등록·조회·상태 갱신.

use super::current_timestamp;
use rusqlite::{params, Connection, OptionalExtension, Result};

// ==================== 감시 폴더 ====================

/// 감시 폴더가 이미 등록되어 있는지 확인 (경로 표현 불일치 허용 — 이슈 #34)
pub fn is_folder_watched(conn: &Connection, path: &str) -> Result<bool> {
    Ok(find_watched_folder_id(conn, path)?.is_some())
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

/// 감시 폴더 삭제 (경로 표현 불일치 허용 — 이슈 #34, 과거 이슈 #22 "폴더 삭제 안 됨" 계열)
pub fn remove_watched_folder(conn: &Connection, path: &str) -> Result<usize> {
    match find_watched_folder_id(conn, path)? {
        Some(id) => conn.execute("DELETE FROM watched_folders WHERE id = ?", params![id]),
        None => Ok(0),
    }
}

/// 즐겨찾기 토글 (경로 표현 불일치 허용 — 이슈 #34)
pub fn toggle_favorite(conn: &Connection, path: &str) -> Result<bool> {
    let id = find_watched_folder_id(conn, path)?.ok_or(rusqlite::Error::QueryReturnedNoRows)?;

    let current: i32 = conn.query_row(
        "SELECT COALESCE(is_favorite, 0) FROM watched_folders WHERE id = ?",
        params![id],
        |row| row.get(0),
    )?;

    let new_value = if current == 0 { 1 } else { 0 };

    conn.execute(
        "UPDATE watched_folders SET is_favorite = ? WHERE id = ?",
        params![new_value, id],
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

/// 경로 표현이 달라도 같은 watched_folders row를 찾는다 (이슈 #34).
///
/// 호출자마다 UI(=DB 저장 표현)와 canonicalize된 표현(매핑드라이브 ↔ UNC)이 섞여
/// 들어오는데, `WHERE path = ?` exact 매치는 0 rows로 **침묵 no-op**이 되어 폴더
/// 상태가 'indexing'에 고착되고 "이어서 인덱싱" 프롬프트가 무한 재등장한다.
/// exact 매치 실패 시에만 normalize_for_compare로 전 폴더를 대조한다
/// (watched_folders는 수십 건 규모라 전량 스캔 비용 무시 가능).
fn find_watched_folder_id(conn: &Connection, path: &str) -> Result<Option<i64>> {
    let exact: Option<i64> = conn
        .query_row(
            "SELECT id FROM watched_folders WHERE path = ?",
            params![path],
            |row| row.get(0),
        )
        .optional()?;
    if exact.is_some() {
        return Ok(exact);
    }

    use crate::utils::network_path;
    let drive_map = network_path::network_drive_map();
    let target = network_path::normalize_for_compare(std::path::Path::new(path), &drive_map);

    let mut stmt = conn.prepare("SELECT id, path FROM watched_folders")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (id, folder_path) = row?;
        let key =
            network_path::normalize_for_compare(std::path::Path::new(&folder_path), &drive_map);
        if key == target {
            return Ok(Some(id));
        }
    }
    Ok(None)
}

/// 폴더 인덱싱 상태 업데이트 (경로 표현 불일치 허용 — 이슈 #34)
pub fn set_folder_indexing_status(conn: &Connection, path: &str, status: &str) -> Result<usize> {
    match find_watched_folder_id(conn, path)? {
        Some(id) => conn.execute(
            "UPDATE watched_folders SET indexing_status = ? WHERE id = ?",
            params![status, id],
        ),
        None => Ok(0),
    }
}

/// 폴더 마지막 동기화 시각 업데이트 (경로 표현 불일치 허용 — 이슈 #34)
pub fn update_last_synced_at(conn: &Connection, path: &str) -> Result<usize> {
    let now = current_timestamp();
    match find_watched_folder_id(conn, path)? {
        Some(id) => conn.execute(
            "UPDATE watched_folders SET last_synced_at = ? WHERE id = ?",
            params![now, id],
        ),
        None => Ok(0),
    }
}

/// FTS 인덱싱이 완료된(`fts_indexed_at IS NOT NULL`) 파일 경로를 행 단위로 순회.
///
/// 폴더 prefix `LIKE` 매칭은 네트워크 경로 표현(매핑드라이브 ↔ UNC ↔ `\\?\`)이 흔들리면
/// 0건을 반환해 resume 가 전체 재인덱싱으로 빠진다(이슈 #34). 그 한계를 피하기 위해
/// SQL 필터 없이 전량을 순회시키되, 전체 경로를 `Vec` 으로 물질화하지 않고 호출부가
/// 행마다 `network_path::normalize_for_compare` 로 표현을 통일해 폴더 소속을 판정하고
/// 필요한 경로만 남기게 한다 (대형 DB resume 피크 메모리 절감).
pub fn for_each_fts_indexed_path(conn: &Connection, mut f: impl FnMut(&str)) -> Result<()> {
    let mut stmt = conn.prepare("SELECT path FROM files WHERE fts_indexed_at IS NOT NULL")?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        f(row.get_ref(0)?.as_str()?);
    }
    Ok(())
}
