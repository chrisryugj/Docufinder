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
pub fn scope_like_pattern(scope: &str) -> Option<String> {
    normalize_scope_prefix(scope).map(|p| {
        let escaped = crate::db::escape_like_pattern(&p);
        format!("{}%", escaped)
    })
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

    #[test]
    fn unc_prefix_stripped() {
        assert!(path_in_scope(r"\\?\C:\docs\a\foo.txt", r"C:\docs\a"));
    }

    #[test]
    fn empty_scope_means_no_restriction() {
        assert!(path_in_scope(r"C:\docs\a\foo.txt", ""));
        assert!(scope_like_pattern("").is_none());
    }

    #[test]
    fn like_pattern_has_trailing_separator() {
        let pat = scope_like_pattern(r"C:\docs\a").unwrap();
        assert_eq!(pat, "c:/docs/a/%");
    }
}
