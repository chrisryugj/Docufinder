use std::path::Path;

/// Lightroom 카탈로그 캐시처럼 폴더명이 매번 달라지는 패키지형 디렉토리 접미사.
const DEFAULT_EXCLUDED_DIR_SUFFIXES: &[&str] = &[".lrdata", ".lrcat-data"];

fn normalize_for_compare(input: &str) -> String {
    input
        .trim()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_ascii_lowercase()
}

/// 제외 디렉토리 판정.
///
/// - 기본/커스텀 폴더명: `node_modules`, `AppData`
/// - 커스텀 전체 경로: `C:\Users\Chris\AppData\Local\Temp`
/// - 패키지형 캐시 폴더: `Catalog Previews.lrdata`
pub fn is_excluded_dir(path: &Path, excluded_dirs: &[String]) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };

    let name_lower = name.to_ascii_lowercase();
    let is_root_level_dollar_dir = name.starts_with('$')
        && path
            .parent()
            .map(|parent| parent.parent().is_none())
            .unwrap_or(false);
    if is_root_level_dollar_dir {
        return true;
    }

    if DEFAULT_EXCLUDED_DIR_SUFFIXES
        .iter()
        .any(|suffix| name_lower.ends_with(suffix))
    {
        return true;
    }

    let normalized_path = normalize_for_compare(&path.to_string_lossy());

    excluded_dirs.iter().any(|entry| {
        let entry = entry.trim();
        if entry.is_empty() {
            return false;
        }

        if entry.contains('\\') || entry.contains('/') {
            normalize_for_compare(entry) == normalized_path
        } else {
            name.eq_ignore_ascii_case(entry)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::is_excluded_dir;
    use std::path::Path;

    #[test]
    fn matches_exact_directory_name_case_insensitively() {
        let excluded = vec!["appdata".to_string()];
        assert!(is_excluded_dir(
            Path::new(r"C:\Users\Chris\AppData"),
            &excluded
        ));
    }

    #[test]
    fn matches_custom_full_path() {
        let excluded = vec![r"C:\Users\Chris\Work\Cache".to_string()];
        assert!(is_excluded_dir(
            Path::new(r"C:\Users\Chris\Work\Cache"),
            &excluded
        ));
    }

    #[test]
    fn matches_lightroom_package_suffix() {
        let excluded = vec![];
        assert!(is_excluded_dir(
            Path::new(r"D:\Photos\Catalog Previews.lrdata"),
            &excluded
        ));
    }

    #[test]
    fn matches_root_level_dollar_directory() {
        let excluded = vec![];
        assert!(is_excluded_dir(Path::new(r"C:\$WinREAgent"), &excluded));
    }

    #[test]
    fn ignores_non_matching_directory() {
        let excluded = vec!["node_modules".to_string()];
        assert!(!is_excluded_dir(
            Path::new(r"C:\Users\Chris\Documents"),
            &excluded
        ));
    }
}
