//! 폴더 scope 경계 매칭 — sibling 폴더 오탐 방지용 공통 헬퍼.
//!
//! 단순 `starts_with(scope)` 는 `C:\docs\a` 가 `C:\docs\a-old` 까지 잡아
//! 범위 제한 검색과 파일 화이트리스트에서 데이터 노출 위험이 있다.
//! 이 모듈은 scope 뒤에 반드시 path separator 를 붙여 segment 경계에서
//! 끊어지도록 한다. 또한 Windows 백슬래시 / POSIX 슬래시를 통일해
//! DB 저장 포맷과 입력 포맷이 달라도 일관된 결과를 낸다.

/// 경로를 scope 비교용으로 정규화 (lowercase + 슬래시 통일 + `\\?\` / `\\?\UNC\` 제거).
/// UNC 경로는 dunce 로 `\\?\UNC\srv\share\...` → `\\srv\share\...` 까지 복원한 뒤
/// `//srv/share/...` 로 슬래시 통일한다.
pub fn normalize_for_scope(path: &str) -> String {
    let simplified = crate::utils::network_path::simplify(std::path::Path::new(path));
    simplified
        .to_string_lossy()
        .replace('\\', "/")
        .to_lowercase()
}

/// Scope 문자열을 prefix 로 사용하도록 정규화.
/// 빈 문자열이면 `None` 을 반환해 "스코프 없음" 과 구분한다.
pub fn normalize_scope_prefix(scope: &str) -> Option<String> {
    let norm = normalize_for_scope(scope);
    let norm = norm.trim_end_matches('/').to_string();
    if norm.is_empty() {
        None
    } else {
        Some(format!("{}/", norm))
    }
}

/// `path` 가 `scope` 로 시작하는지 segment 경계 기준으로 확인.
/// scope 가 비어있으면 true (제약 없음).
pub fn path_in_scope(path: &str, scope: &str) -> bool {
    match normalize_scope_prefix(scope) {
        Some(prefix) => normalize_for_scope(path).starts_with(&prefix),
        None => true,
    }
}

/// FTS LIKE 패턴용: scope 에 segment 경계를 강제한 prefix 패턴 반환.
/// 호출자는 SQL 에서 `REPLACE(LOWER(path), '\\', '/') LIKE ? ESCAPE '\\'` 로 써야 한다.
///
/// SQLite `LOWER()`·`LIKE` 는 ASCII 만 대소문자를 가린다. 패턴을 유니코드 전체로 소문자화하면
/// `É`·`Ж` 같은 대문자가 든 폴더는 DB 쪽이 그대로라 영영 맞지 않았다 — ASCII 만 내린다.
pub fn scope_like_pattern(scope: &str) -> Option<String> {
    let simplified = crate::utils::network_path::simplify(std::path::Path::new(scope));
    let norm = simplified
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase();
    let norm = norm.trim_end_matches('/');
    if norm.is_empty() {
        return None;
    }
    let escaped = crate::db::escape_like_pattern(&format!("{norm}/"));
    Some(format!("{}%", escaped))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sibling_folder_rejected() {
        assert!(!path_in_scope(r"C:\docs\a-old\foo.txt", r"C:\docs\a"));
        assert!(!path_in_scope("C:/docs/a-old/foo.txt", "C:/docs/a"));
    }

    #[test]
    fn child_folder_accepted() {
        assert!(path_in_scope(r"C:\docs\a\foo.txt", r"C:\docs\a"));
        assert!(path_in_scope(r"C:\docs\a\sub\foo.txt", r"C:\docs\a\"));
    }

    #[test]
    fn case_insensitive_and_mixed_separators() {
        assert!(path_in_scope(r"C:\Docs\A\foo.txt", "c:/docs/a"));
        assert!(path_in_scope("c:/docs/a/foo.txt", r"C:\DOCS\A"));
    }

    #[cfg(windows)]
    #[test]
    fn unc_prefix_stripped() {
        assert!(path_in_scope(r"\\?\C:\docs\a\foo.txt", r"C:\docs\a"));
    }

    // verbatim UNC(`\\?\UNC\`)와 일반 UNC 가 같은 scope 로 매칭돼야 open_file 화이트리스트가
    // DB 저장 형식(구·신)과 무관하게 통과한다(이슈 #46). 문자열 처리라 OS 무관.
    #[test]
    fn verbatim_unc_matches_plain_unc_scope() {
        assert!(path_in_scope(
            r"\\?\UNC\srv\share\docs\a.txt",
            r"\\srv\share\docs"
        ));
        assert!(path_in_scope(
            r"\\srv\share\docs\a.txt",
            r"\\?\UNC\srv\share\docs"
        ));
        assert!(!path_in_scope(
            r"\\?\UNC\srv\share\docs-old\a.txt",
            r"\\srv\share\docs"
        ));
    }

    #[test]
    fn empty_scope_means_no_restriction() {
        assert!(path_in_scope(r"C:\docs\a\foo.txt", ""));
        assert!(scope_like_pattern("").is_none());
    }

    /// SQLite LOWER 는 ASCII 만 내리므로 패턴도 그래야 비ASCII 대문자 폴더가 맞는다.
    #[test]
    fn like_pattern_matches_sqlite_lower_for_non_ascii() {
        let pat = scope_like_pattern(r"C:\Users\Émile\Документы").unwrap();
        assert_eq!(pat, "c:/users/Émile/Документы/%");
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let hit: bool = conn
            .query_row(
                "SELECT REPLACE(LOWER(?1), '\\', '/') LIKE ?2 ESCAPE '\\'",
                [r"C:\Users\Émile\Документы\보고서.hwpx", pat.as_str()],
                |r| r.get(0),
            )
            .unwrap();
        assert!(hit, "비ASCII 대문자 폴더가 범위에서 빠졌다");
    }

    #[test]
    fn like_pattern_has_trailing_separator() {
        let pat = scope_like_pattern(r"C:\docs\a").unwrap();
        assert_eq!(pat, "c:/docs/a/%");
    }
}
