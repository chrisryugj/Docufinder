pub mod migration;
pub mod pool;

mod chunks;
mod files;
mod folders;
mod stats;

#[cfg(test)]
mod pool_race_tests;

pub use migration::*;
pub use pool::*;

pub use chunks::*;
pub use files::*;
pub use folders::*;
pub use stats::*;

use rusqlite::Result;
#[cfg(windows)]
use rusqlite::{params, Connection};
use std::time::{SystemTime, UNIX_EPOCH};

/// SQLITE_BUSY 시 재시도하는 래퍼 (busy_timeout으로 부족한 경우를 위한 application-level retry)
/// 최대 3회, 지수 백오프 100→200→400ms.
/// ⚠️ sync-only: std::thread::sleep 사용. async 컨텍스트에서는 spawn_blocking 내에서 호출할 것.
pub fn retry_on_busy<F, T>(f: F) -> Result<T>
where
    F: Fn() -> Result<T>,
{
    const MAX_RETRIES: u32 = 3;
    const RETRY_BASE_MS: u64 = 100; // 지수 백오프: 100ms → 200ms → 400ms

    for attempt in 0..MAX_RETRIES {
        match f() {
            Ok(v) => return Ok(v),
            Err(e) => {
                let is_busy = matches!(
                    e,
                    rusqlite::Error::SqliteFailure(
                        rusqlite::ffi::Error {
                            code: rusqlite::ffi::ErrorCode::DatabaseBusy,
                            ..
                        },
                        _,
                    )
                );
                if is_busy && attempt < MAX_RETRIES - 1 {
                    let delay_ms = RETRY_BASE_MS << attempt; // 100, 200, 400
                    tracing::warn!(
                        "[DB retry] SQLITE_BUSY on attempt {}/{}, retrying in {}ms...",
                        attempt + 1,
                        MAX_RETRIES,
                        delay_ms
                    );
                    std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                    continue;
                }
                return Err(e);
            }
        }
    }
    unreachable!()
}

/// LIKE 패턴 특수문자 이스케이프 (SQL Injection 방지)
/// %, _, \ 문자를 이스케이프하여 리터럴로 처리
pub fn escape_like_pattern(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// 현재 시간을 Unix timestamp로 반환 (패닉 방지)
fn current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// 이슈 #29: 매핑 네트워크 드라이브 prefix(`Y:\`)를 UNC base(`\\srv\share`)로 일괄 치환.
///
/// UAC elevated 실행 시 매핑 드라이브가 안 보여 sync/재인덱싱이 막히던 기존 등록
/// 경로를 자동 치유한다. `files`·`watched_folders` 의 `Y:\…` → `\\srv\share\…`.
/// 멱등 — 이미 UNC 면 매칭 0건. `(files_updated, folders_updated)` 반환.
/// 호출부(`lib.rs` 시작 시 마이그레이션)가 windows 전용이라 함수도 cfg 게이트.
#[cfg(windows)]
pub fn remap_drive_prefix(
    conn: &Connection,
    letter: char,
    unc_base: &str,
) -> Result<(usize, usize)> {
    let unc_base = unc_base.trim_end_matches(['\\', '/']);
    // `Y:\` 로 시작하는 경로만. 백슬래시는 ESCAPE 로 리터럴화 → `Y:\\%`.
    let like = format!("{}%", escape_like_pattern(&format!("{letter}:\\")));
    // 드라이브 prefix(`Y:`)는 항상 2 ASCII 글자라 substr(path, 3) 은 그 뒤 나머지(`\…`).
    // 한글 등 멀티바이트 경로에서도 SQLite substr 는 문자(코드포인트) 단위라 안전.
    let tx = conn.unchecked_transaction()?;
    let files = tx.execute(
        r"UPDATE files SET path = ?1 || substr(path, 3) WHERE path LIKE ?2 ESCAPE '\'",
        params![unc_base, like],
    )?;
    let folders = tx.execute(
        r"UPDATE watched_folders SET path = ?1 || substr(path, 3) WHERE path LIKE ?2 ESCAPE '\'",
        params![unc_base, like],
    )?;
    tx.commit()?;
    Ok((files, folders))
}

/// 이슈 #46: `\\?\UNC\srv\share\…`(Windows `canonicalize` 의 verbatim UNC)로 저장된 경로를
/// `\\srv\share\…` 로 일괄 복원.
///
/// `dunce` 가 verbatim UNC 는 벗기지 않아 v3.8.5 까지 DB 에 그대로 들어갔고, 프론트가 `\\?\` 만
/// 떼면 `UNC\srv\…` 가 되어 파일 열기·위치 열기·경로 복사가 전부 깨졌다. 같은 파일이 두 표현으로
/// 공존하면(정규화 실패 폴백 → 성공 세션 순으로 인덱싱된 경우) UNIQUE 충돌이 나므로 verbatim
/// 쪽 행(청크·FTS 포함)을 먼저 지운 뒤 나머지를 치환한다. 북마크·태그는 `file_path` 문자열로
/// 파일을 가리키므로 함께 옮긴다(충돌 행은 유지). 멱등 — 이미 `\\srv` 면 매칭 0건.
/// `(files_updated, folders_updated)` 반환. 호출부(`lib.rs`)가 windows 전용이라 함수도 cfg 게이트.
#[cfg(windows)]
pub fn remap_unc_verbatim_prefix(conn: &Connection) -> Result<(usize, usize)> {
    // `\\?\UNC\` 는 8글자 고정 → substr(path, 9) 가 `srv\share\…`, 앞에 `\\` 를 붙인다.
    // SQLite 문자열 리터럴은 이스케이프가 없어 '\\' 가 백슬래시 두 글자 그대로다.
    let like = format!("{}%", escape_like_pattern(r"\\?\UNC\"));
    let tx = conn.unchecked_transaction()?;
    // 1) `\\srv\…` 로 이미 있는 파일의 verbatim 중복 행 제거 (chunks_fts → chunks → files 순, delete_file 과 동일).
    tx.execute(
        r"DELETE FROM chunks_fts WHERE rowid IN (
            SELECT c.id FROM chunks c JOIN files f ON c.file_id = f.id
            WHERE f.path LIKE ?1 ESCAPE '\' AND ('\\' || substr(f.path, 9)) IN (SELECT path FROM files))",
        params![like],
    )?;
    tx.execute(
        r"DELETE FROM chunks WHERE file_id IN (
            SELECT id FROM files
            WHERE path LIKE ?1 ESCAPE '\' AND ('\\' || substr(path, 9)) IN (SELECT path FROM files))",
        params![like],
    )?;
    tx.execute(
        r"DELETE FROM files WHERE path LIKE ?1 ESCAPE '\' AND ('\\' || substr(path, 9)) IN (SELECT path FROM files)",
        params![like],
    )?;
    tx.execute(
        r"DELETE FROM watched_folders WHERE path LIKE ?1 ESCAPE '\' AND ('\\' || substr(path, 9)) IN (SELECT path FROM watched_folders)",
        params![like],
    )?;
    // 2) 나머지 치환.
    let files = tx.execute(
        r"UPDATE files SET path = '\\' || substr(path, 9) WHERE path LIKE ?1 ESCAPE '\'",
        params![like],
    )?;
    let folders = tx.execute(
        r"UPDATE watched_folders SET path = '\\' || substr(path, 9) WHERE path LIKE ?1 ESCAPE '\'",
        params![like],
    )?;
    tx.execute(
        r"UPDATE OR IGNORE bookmarks SET file_path = '\\' || substr(file_path, 9) WHERE file_path LIKE ?1 ESCAPE '\'",
        params![like],
    )?;
    tx.execute(
        r"UPDATE OR IGNORE file_tags SET file_path = '\\' || substr(file_path, 9) WHERE file_path LIKE ?1 ESCAPE '\'",
        params![like],
    )?;
    tx.commit()?;
    Ok((files, folders))
}

#[cfg(test)]
mod folder_status_tests {
    use super::*;
    use rusqlite::{params, Connection};

    fn test_db() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("t.db");
        let conn = Connection::open(&db_path).unwrap();
        migrate_schema(&conn, &db_path).unwrap();
        (dir, conn)
    }

    /// 이슈 #34: UI가 넘긴 경로 표현(구분자/대소문자/trailing)이 DB 저장 표현과
    /// 달라도 폴더 상태 UPDATE가 no-op으로 새지 않아야 한다
    #[test]
    fn status_update_matches_across_representations() {
        let (_dir, conn) = test_db();
        conn.execute(
            "INSERT INTO watched_folders (path, indexing_status) VALUES (?, 'indexing')",
            params![r"C:\Docs\업무"],
        )
        .unwrap();

        // 슬래시 방향 + 대소문자 + trailing 이 다른 표현으로 완료 마킹
        let n = set_folder_indexing_status(&conn, r"c:/docs/업무/", "completed").unwrap();
        assert_eq!(n, 1, "표현이 달라도 같은 폴더 row가 갱신돼야 함");

        let status: String = conn
            .query_row("SELECT indexing_status FROM watched_folders", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(status, "completed");

        // 존재하지 않는 폴더는 0 rows (에러 아님)
        let n = set_folder_indexing_status(&conn, r"D:\없는폴더", "failed").unwrap();
        assert_eq!(n, 0);
    }

    /// resume 스트리밍(T2-3): fts_indexed_at 이 있는 경로만 행 단위로 방문해야 한다
    #[test]
    fn for_each_fts_indexed_path_visits_only_indexed() {
        let (_dir, conn) = test_db();
        let a = upsert_file(&conn, r"C:\Docs\a.txt", "a.txt", "txt", 1, 1).unwrap();
        upsert_file(&conn, r"C:\Docs\b.txt", "b.txt", "txt", 1, 1).unwrap();
        conn.execute(
            "UPDATE files SET fts_indexed_at = 1 WHERE id = ?",
            params![a],
        )
        .unwrap();

        let mut seen = Vec::new();
        for_each_fts_indexed_path(&conn, |p| seen.push(p.to_string())).unwrap();
        assert_eq!(seen, vec![r"C:\Docs\a.txt".to_string()]);
    }
}
