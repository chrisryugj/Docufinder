//! 폴더 경로 표현 수렴 (이슈 #34 완결편).
//!
//! 네트워크 폴더는 시점/세션에 따라 매핑드라이브(`Z:\docs`)와 UNC(`\\srv\share\docs`)
//! 표현이 흔들린다. v3.0.7이 resume "스킵 판정"은 normalize_for_compare로 통일했지만
//! **저장 표현**은 그대로라 두 가지가 남았다:
//!
//! 1. sync(startup/periodic)의 diff가 `t.path = f.path` exact 비교 → 표현이 다른
//!    기존 rows를 전부 "신규"로 오판 → 전체 재파싱 + 파일당 files row 2개(중복 검색결과)
//! 2. 옛 표현으로 저장된 rows는 삭제 감지 LIKE에도 안 걸려 영구 잔존
//!
//! 이 모듈은 인덱싱/sync 진입 시 폴더의 canonical 표현을 확정하고, DB에 남은
//! 다른 표현(변형) rows를 canonical로 **프리픽스 재작성**해 수렴시킨다.
//! chunks는 file_id로 연결되므로 UPDATE만으로 재인덱싱·재임베딩 없이 이관된다.
//! 같은 파일이 양쪽 표현으로 이미 중복 인덱싱된 경우(충돌)는 fts_indexed_at이
//! 더 최신인 쪽을 남기고 패자의 벡터·청크·row를 정리한다.

use crate::db;
use crate::search::vector::VectorIndex;
use crate::utils::network_path;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};

/// 인덱싱/sync 진입점용 래퍼: canonical 계산 + 수렴 실행.
/// 실패해도 인덱싱 자체는 막지 않는다(원본 경로 반환).
pub fn reconcile_folder_representation(
    conn: &Connection,
    folder_path: &Path,
    vector_index: Option<&VectorIndex>,
) -> PathBuf {
    let canonical = network_path::canonicalize_best_effort(folder_path);
    match reconcile_with_canonical(conn, folder_path, &canonical, vector_index) {
        Ok(()) => canonical,
        Err(e) => {
            tracing::warn!(
                "경로 표현 수렴 실패 (인덱싱은 계속): {} → {}: {}",
                folder_path.display(),
                canonical.display(),
                e
            );
            canonical
        }
    }
}

/// 수렴 코어 (canonical 주입 — 테스트 가능).
///
/// `folder_path`(호출자가 넘긴 표현)와 watched_folders에 등록된 모든 "같은 폴더의
/// 다른 표현"을 변형(variant)으로 수집해, files/bookmarks/file_tags/watched_folders를
/// canonical 표현으로 재작성한다. 변형이 없으면 no-op.
pub fn reconcile_with_canonical(
    conn: &Connection,
    folder_path: &Path,
    canonical: &Path,
    vector_index: Option<&VectorIndex>,
) -> rusqlite::Result<()> {
    // canonical/변형은 **원문 그대로** 보관한다 — unify(`/`→`\`)한 문자열을 DB에
    // 기록하면 macOS의 POSIX 경로가 백슬래시로 오염된다. 구분자 통일은 비교
    // 연산에만 쓴다.
    let canonical_str = canonical.to_string_lossy().to_string();
    let canonical_cmp = unify_separators(&canonical_str);
    let drive_map = network_path::network_drive_map();
    let target_key = network_path::normalize_for_compare(canonical, &drive_map);

    // ── 변형 수집: 호출 인자 + watched_folders에서 같은 폴더의 다른 표현 ──
    let mut variants: Vec<String> = Vec::new();
    let folder_str = folder_path.to_string_lossy().to_string();
    // 호출 인자 표현은 normalize 키가 canonical과 달라도 수렴 대상이다:
    // canonical은 "그 인자 자체"를 OS가 해소한 결과라 같은 폴더임이 구성상
    // 보장된다. 키 일치를 요구하면 drive_map 밖 divergence(심볼릭링크·junction·
    // subst·DFS)에서 no-op이 되는데, sync 걷기 루트는 canonical로 바뀌므로
    // 구표현 rows가 diff에서 전부 "신규" 오판 + 영구 잔존한다.
    if unify_separators(&folder_str) != canonical_cmp {
        variants.push(folder_str);
    }

    let mut stmt = conn.prepare("SELECT path FROM watched_folders")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    for row in rows {
        let stored = row?;
        let stored_cmp = unify_separators(&stored);
        if stored_cmp == canonical_cmp || variants.iter().any(|v| unify_separators(v) == stored_cmp)
        {
            continue;
        }
        // 등록된 다른 폴더 row는 유도 관계가 없으므로 normalize 키 일치(구분자·
        // 대소문자·매핑드라이브↔UNC)로만 같은 폴더 판정 — 오매칭 방지.
        if network_path::normalize_for_compare(Path::new(&stored), &drive_map) == target_key {
            variants.push(stored);
        }
    }
    drop(stmt);

    if variants.is_empty() {
        return Ok(()); // 이미 수렴됨 — 대부분의 실행에서 여기로 즉시 귀결
    }

    tracing::info!(
        "경로 표현 수렴 시작: {:?} → {} (변형 {}개)",
        variants,
        canonical_str,
        variants.len()
    );

    conn.execute_batch("BEGIN IMMEDIATE")?;
    let result = (|| -> rusqlite::Result<(usize, usize)> {
        let mut moved = 0usize;
        let mut merged = 0usize;

        // 조인 경계용 stem: 후행 구분자 제거. 변형에 후행 구분자가 있거나(`Z:\docs\`)
        // 드라이브 루트(`C:\`)일 때 suffix가 항상 구분자로 시작하도록 보장해
        // `Z:\docsa.txt`/`C:\\a.txt` 류 조인 오염을 막는다.
        let canonical_stem = canonical_str.trim_end_matches(['\\', '/']);

        for variant in &variants {
            // 1. files: 변형 프리픽스 rows 수집. 변형은 원문이라 구분자가 혼합될 수
            //    있으므로 win/unix 양 형태를 만들어 exact + LIKE 네 arm으로 건다.
            //    (두 형태는 바이트 길이가 같아 suffix 슬라이스에 안전)
            let variant_stem = variant.trim_end_matches(['\\', '/']);
            if variant_stem.is_empty() {
                // 루트("/" 등)가 변형으로 들어오면 LIKE가 전 rows를 잡는다 — 스킵
                continue;
            }
            let stem_win = variant_stem.replace('/', "\\");
            let stem_unix = variant_stem.replace('\\', "/");
            let variant_win = variant.replace('/', "\\");
            let variant_unix = variant.replace('\\', "/");
            let escaped_win = db::escape_like_pattern(&stem_win);
            let escaped_unix = db::escape_like_pattern(&stem_unix);
            let files: Vec<(i64, String, Option<i64>)> = {
                let mut stmt = conn.prepare(
                    "SELECT id, path, fts_indexed_at FROM files \
                     WHERE path = ?1 OR path = ?2 \
                        OR path LIKE ?3 ESCAPE '\\' OR path LIKE ?4 ESCAPE '\\'",
                )?;
                let rows = stmt.query_map(
                    params![
                        stem_win,
                        stem_unix,
                        // 구분자 `\`는 ESCAPE 문자 자체라 `\\`로 이스케이프해야
                        // `%`가 와일드카드로 동작한다 (get_file_and_chunk_ids_in_folder와 동일)
                        format!("{}\\\\%", escaped_win),
                        format!("{}/%", escaped_unix),
                    ],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )?;
                rows.collect::<rusqlite::Result<_>>()?
            };

            for (file_id, old_path, old_fts_at) in files {
                // 양 구분자 형태는 바이트 길이가 같아(구분자 1바이트) 어느 arm이
                // 매치됐든 raw old_path의 stem 길이 지점이 suffix 경계다 (stem 뒤는
                // 항상 구분자 또는 문자열 끝). suffix는 원문에서 취해 구분자를 보존한다.
                let suffix_raw = &old_path[variant_stem.len()..];
                // Windows 표현(canonical에 `\` 포함)이면 suffix의 `/`도 `\`로 통일
                // (Windows 파일명엔 `/`가 올 수 없어 안전). POSIX면 원문 유지 —
                // `\`는 macOS에서 파일명 문자일 수 있어 치환 금지.
                let suffix = if canonical_str.contains('\\') {
                    suffix_raw.replace('/', "\\")
                } else {
                    suffix_raw.to_string()
                };
                let new_path = format!("{}{}", canonical_stem, suffix);

                // SQLite LIKE는 ASCII 대소문자 무시라 이미 canonical인 row도 매치된다
                // — 재작성 결과가 원본과 같으면 스킵 (자기 자신과의 충돌 병합 방지)
                if old_path == new_path {
                    continue;
                }

                // 충돌: canonical 표현으로 이미 인덱싱된 같은 파일이 있는 경우
                let existing: Option<(i64, Option<i64>)> = conn
                    .query_row(
                        "SELECT id, fts_indexed_at FROM files WHERE path = ?",
                        params![new_path],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .optional()?;

                if let Some((existing_id, existing_fts_at)) = existing {
                    // fts_indexed_at 최신 쪽 유지 (NULL=0). 동률이면 기존 canonical 행 유지.
                    let keep_old = old_fts_at.unwrap_or(0) > existing_fts_at.unwrap_or(0);
                    let loser_id = if keep_old { existing_id } else { file_id };
                    delete_file_by_id_no_tx(conn, loser_id, vector_index)?;
                    if keep_old {
                        conn.execute(
                            "UPDATE files SET path = ? WHERE id = ?",
                            params![new_path, file_id],
                        )?;
                    }
                    merged += 1;
                } else {
                    conn.execute(
                        "UPDATE files SET path = ? WHERE id = ?",
                        params![new_path, file_id],
                    )?;
                    moved += 1;
                }

                // 경로 문자열을 참조하는 부속 테이블 — 충돌은 무시 가능(UNIQUE 없음)
                for (table, col) in [("bookmarks", "file_path"), ("file_tags", "file_path")] {
                    let _ = conn.execute(
                        &format!("UPDATE {table} SET {col} = ?1 WHERE {col} = ?2"),
                        params![new_path, old_path],
                    );
                }
            }

            // 2. watched_folders: canonical row가 이미 있으면 변형 row 제거(중복 등록),
            //    없으면 표현만 교체
            let canonical_exists: Option<i64> = conn
                .query_row(
                    "SELECT id FROM watched_folders WHERE path = ?",
                    params![canonical_str],
                    |row| row.get(0),
                )
                .optional()?;
            if canonical_exists.is_some() {
                conn.execute(
                    "DELETE FROM watched_folders WHERE path = ? OR path = ?",
                    params![variant_win, variant_unix],
                )?;
            } else {
                conn.execute(
                    "UPDATE watched_folders SET path = ?1 WHERE path = ?2 OR path = ?3",
                    params![canonical_str, variant_win, variant_unix],
                )?;
            }
        }
        Ok((moved, merged))
    })();

    match result {
        Ok((moved, merged)) => {
            conn.execute_batch("COMMIT")?;
            tracing::info!(
                "경로 표현 수렴 완료: 이관 {}건, 중복 병합 {}건",
                moved,
                merged
            );
            Ok(())
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

/// 표현 비교/재작성용 구분자 통일 (`/` → `\`). 내용은 바꾸지 않는다.
fn unify_separators(s: &str) -> String {
    s.replace('/', "\\")
}

/// 트랜잭션 내부용 파일 삭제 (delete_file은 자체 BEGIN을 열어 중첩 불가).
/// chunks_fts → chunks → files 순서 + 벡터 제거.
fn delete_file_by_id_no_tx(
    conn: &Connection,
    file_id: i64,
    vector_index: Option<&VectorIndex>,
) -> rusqlite::Result<()> {
    if let Some(vi) = vector_index {
        if let Ok(chunk_ids) = db::get_chunk_ids_for_file(conn, file_id) {
            for chunk_id in chunk_ids {
                if let Err(e) = vi.remove(chunk_id) {
                    tracing::debug!("reconcile: vector remove 실패 {}: {}", chunk_id, e);
                }
            }
        }
    }
    conn.execute(
        "DELETE FROM chunks_fts WHERE rowid IN (SELECT id FROM chunks WHERE file_id = ?)",
        params![file_id],
    )?;
    conn.execute("DELETE FROM chunks WHERE file_id = ?", params![file_id])?;
    conn.execute("DELETE FROM files WHERE id = ?", params![file_id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("t.db");
        let conn = Connection::open(&db_path).unwrap();
        crate::db::migrate_schema(&conn, &db_path).unwrap();
        (dir, conn)
    }

    fn insert_file(conn: &Connection, path: &str, fts_at: Option<i64>) -> i64 {
        conn.execute(
            "INSERT INTO files (path, name, file_type, size, modified_at, fts_indexed_at) \
             VALUES (?1, 'n', 'txt', 1, 1, ?2)",
            params![path, fts_at],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    fn insert_chunk(conn: &Connection, file_id: i64, content: &str) -> i64 {
        conn.execute(
            "INSERT INTO chunks (file_id, chunk_index, start_offset, end_offset) \
             VALUES (?1, 0, 0, 1)",
            params![file_id],
        )
        .unwrap();
        let id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO chunks_fts (rowid, content) VALUES (?1, ?2)",
            params![id, content],
        )
        .unwrap();
        id
    }

    /// 매핑드라이브(Z:\) 표현으로 저장된 rows가 UNC canonical로 이관되고
    /// chunks(file_id 연결)는 그대로 유지되는지 — #34 핵심 시나리오
    #[test]
    fn migrates_legacy_rows_to_canonical() {
        let (_dir, conn) = test_db();
        conn.execute(
            "INSERT INTO watched_folders (path) VALUES (?)",
            params![r"Z:\docs"],
        )
        .unwrap();
        let fid = insert_file(&conn, r"Z:\docs\보고서.hwpx", Some(100));
        let cid = insert_chunk(&conn, fid, "내용");

        // Z: → \\srv\share 매핑을 canonical로 주입 (drive_map은 테스트 환경에 없으므로
        // 문자열 프리픽스 로직 자체를 검증 — normalize_for_compare는 Z:를 매핑 못 하면
        // 그대로 두므로, 여기서는 folder_path 인자 표현 vs canonical 케이스로 검증한다)
        reconcile_with_canonical(
            &conn,
            Path::new(r"Z:\docs"),
            Path::new(r"Z:\docs"), // canonical == 등록 표현 → 변형 없음 (no-op 확인)
            None,
        )
        .unwrap();
        let p: String = conn
            .query_row("SELECT path FROM files WHERE id = ?", [fid], |r| r.get(0))
            .unwrap();
        assert_eq!(p, r"Z:\docs\보고서.hwpx", "no-op이어야 함");

        // 대소문자 변형: 등록 표현 z:\docs (변형) vs canonical Z:\docs
        conn.execute("UPDATE watched_folders SET path = ?", params![r"z:\docs"])
            .unwrap();
        conn.execute(
            "UPDATE files SET path = ? WHERE id = ?",
            params![r"z:\docs\보고서.hwpx", fid],
        )
        .unwrap();
        reconcile_with_canonical(&conn, Path::new(r"z:\docs"), Path::new(r"Z:\docs"), None)
            .unwrap();

        let p: String = conn
            .query_row("SELECT path FROM files WHERE id = ?", [fid], |r| r.get(0))
            .unwrap();
        assert_eq!(p, r"Z:\docs\보고서.hwpx");
        let wf: String = conn
            .query_row("SELECT path FROM watched_folders", [], |r| r.get(0))
            .unwrap();
        assert_eq!(wf, r"Z:\docs");
        // 청크는 file_id로 그대로 연결
        let cnt: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM chunks WHERE file_id = ? AND id = ?",
                params![fid, cid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cnt, 1);
    }

    /// 양쪽 표현으로 중복 인덱싱된 파일: fts_indexed_at 최신 쪽이 살아남는다
    #[test]
    fn merges_duplicate_rows_keeping_fresher() {
        let (_dir, conn) = test_db();
        conn.execute(
            "INSERT INTO watched_folders (path) VALUES (?)",
            params![r"z:\docs"],
        )
        .unwrap();
        // 변형(z:) 쪽이 더 최신, canonical(Z:) 쪽이 옛날
        let old_id = insert_file(&conn, r"Z:\docs\a.txt", Some(100));
        insert_chunk(&conn, old_id, "옛 내용");
        let new_id = insert_file(&conn, r"z:\docs\a.txt", Some(200));
        insert_chunk(&conn, new_id, "새 내용");

        reconcile_with_canonical(&conn, Path::new(r"z:\docs"), Path::new(r"Z:\docs"), None)
            .unwrap();

        let survivors: Vec<(i64, String)> = {
            let mut stmt = conn.prepare("SELECT id, path FROM files").unwrap();
            let rows = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap()
                .collect::<rusqlite::Result<_>>()
                .unwrap();
            rows
        };
        assert_eq!(
            survivors.len(),
            1,
            "중복 병합 후 1행이어야 함: {survivors:?}"
        );
        assert_eq!(survivors[0].0, new_id, "최신(fts_at=200) 쪽 유지");
        assert_eq!(survivors[0].1, r"Z:\docs\a.txt", "경로는 canonical 표현");
        // 패자의 청크·FTS는 정리됨
        let orphan_chunks: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM chunks WHERE file_id = ?",
                [old_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(orphan_chunks, 0);
    }

    /// 심볼릭링크/junction divergence: 등록 표현과 canonical은 normalize 키도
    /// 다르지만 canonical이 인자 자체의 OS 해소 결과이므로 수렴돼야 한다.
    /// (안 하면 sync 걷기 루트만 canonical로 바뀌어 구표현 rows가 영구 중복 —
    /// fresh-context 검증에서 발견된 회귀)
    #[test]
    fn migrates_symlink_divergent_registration() {
        let (_dir, conn) = test_db();
        conn.execute(
            "INSERT INTO watched_folders (path) VALUES (?)",
            params!["/tmp/x"],
        )
        .unwrap();
        let fid = insert_file(&conn, "/tmp/x/notes/a.txt", Some(1));

        reconcile_with_canonical(
            &conn,
            Path::new("/tmp/x"),
            Path::new("/private/tmp/x"),
            None,
        )
        .unwrap();

        let p: String = conn
            .query_row("SELECT path FROM files WHERE id = ?", [fid], |r| r.get(0))
            .unwrap();
        // POSIX 구분자 보존 — 백슬래시 오염 없어야 함
        assert_eq!(p, "/private/tmp/x/notes/a.txt");
        let wf: String = conn
            .query_row("SELECT path FROM watched_folders", [], |r| r.get(0))
            .unwrap();
        assert_eq!(wf, "/private/tmp/x");
    }

    /// 후행 구분자 변형(`Z:\docs\`)에서 suffix 조인이 오염되지 않아야 한다
    /// (`Z:\docsa.txt` 방지). 이미 canonical인 rows는 no-op.
    #[test]
    fn trailing_separator_variant_joins_cleanly() {
        let (_dir, conn) = test_db();
        conn.execute(
            "INSERT INTO watched_folders (path) VALUES (?)",
            params![r"Z:\docs\"],
        )
        .unwrap();
        let fid = insert_file(&conn, r"Z:\docs\a.txt", Some(1));

        reconcile_with_canonical(&conn, Path::new(r"Z:\docs\"), Path::new(r"Z:\docs"), None)
            .unwrap();

        let p: String = conn
            .query_row("SELECT path FROM files WHERE id = ?", [fid], |r| r.get(0))
            .unwrap();
        assert_eq!(p, r"Z:\docs\a.txt", "이미 canonical 표현인 row는 불변");
        let wf: String = conn
            .query_row("SELECT path FROM watched_folders", [], |r| r.get(0))
            .unwrap();
        assert_eq!(wf, r"Z:\docs", "폴더 row는 canonical로 교체");
    }

    /// 하위 폴더 suffix + 구분자 혼용(`/`)도 프리픽스 스왑이 보존한다
    #[test]
    fn rewrites_nested_and_mixed_separator_paths() {
        let (_dir, conn) = test_db();
        conn.execute(
            "INSERT INTO watched_folders (path) VALUES (?)",
            params![r"z:/docs"],
        )
        .unwrap();
        let fid = insert_file(&conn, r"z:/docs/2026/보고서 최종.hwpx", Some(1));

        reconcile_with_canonical(&conn, Path::new(r"z:/docs"), Path::new(r"Z:\docs"), None)
            .unwrap();

        let p: String = conn
            .query_row("SELECT path FROM files WHERE id = ?", [fid], |r| r.get(0))
            .unwrap();
        assert_eq!(p, r"Z:\docs\2026\보고서 최종.hwpx");
    }
}
