//! WatchedFolder Entity - 감시 중인 폴더를 나타내는 도메인 엔티티

use crate::domain::errors::DomainError;
use std::path::{Path, PathBuf};

/// 시스템 폴더 블랙리스트 (단일 경로 컴포넌트 매칭)
const BLOCKED_COMPONENTS: &[&str] = &[
    "windows",
    "system32",
    "program files",
    "program files (x86)",
    "programdata",
    "$recycle.bin",
    "node_modules",
    ".git",
    "target",
];

/// 다중 컴포넌트 블랙리스트 (연속 경로 세그먼트 매칭)
const BLOCKED_SEQUENCES: &[&[&str]] = &[&["appdata", "local", "temp"]];

/// 감시 폴더 엔티티 (비즈니스 로직 포함)
#[derive(Debug, Clone)]
pub struct WatchedFolder {
    path: PathBuf,
    added_at: i64,
    is_favorite: bool,
}

impl WatchedFolder {
    /// 새 감시 폴더 엔티티 생성 (도메인 규칙 검증 포함)
    pub fn new(path: PathBuf, added_at: i64) -> Result<Self, DomainError> {
        // 빈 경로 검증
        if path.as_os_str().is_empty() {
            return Err(DomainError::InvalidPath {
                path: "빈 경로".to_string(),
            });
        }

        // 블랙리스트 경로 검증
        Self::validate_safe_path(&path)?;

        Ok(Self {
            path,
            added_at,
            is_favorite: false,
        })
    }

    /// DB에서 로드할 때 사용 (검증 없이)
    pub fn reconstitute(path: PathBuf, added_at: i64, is_favorite: bool) -> Self {
        Self {
            path,
            added_at,
            is_favorite,
        }
    }

    /// 안전한 경로인지 검증 (컴포넌트 기반 매칭으로 false positive 방지)
    fn validate_safe_path(path: &Path) -> Result<(), DomainError> {
        let components: Vec<String> = path
            .components()
            .filter_map(|c| match c {
                std::path::Component::Normal(os) => os.to_str().map(|s| s.to_lowercase()),
                _ => None,
            })
            .collect();

        // 단일 컴포넌트 매칭: "windows" == 경로의 개별 폴더명
        for blocked in BLOCKED_COMPONENTS {
            if components.iter().any(|c| c == blocked) {
                return Err(DomainError::ForbiddenPath {
                    path: path.to_string_lossy().to_string(),
                });
            }
        }

        // 다중 컴포넌트 시퀀스 매칭: ["appdata", "local", "temp"] 연속 존재 확인
        for seq in BLOCKED_SEQUENCES {
            if seq.len() <= components.len()
                && components.windows(seq.len()).any(|window| {
                    window.iter().zip(seq.iter()).all(|(c, s)| c == s)
                })
            {
                return Err(DomainError::ForbiddenPath {
                    path: path.to_string_lossy().to_string(),
                });
            }
        }

        Ok(())
    }

    // === Getters ===

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn path_string(&self) -> String {
        self.path.to_string_lossy().to_string()
    }

    pub fn added_at(&self) -> i64 {
        self.added_at
    }

    pub fn is_favorite(&self) -> bool {
        self.is_favorite
    }

    // === 비즈니스 로직 ===

    /// 즐겨찾기 토글
    pub fn toggle_favorite(&mut self) {
        self.is_favorite = !self.is_favorite;
    }

    /// 즐겨찾기 설정
    pub fn set_favorite(&mut self, is_favorite: bool) {
        self.is_favorite = is_favorite;
    }

    /// 폴더명 반환
    pub fn name(&self) -> String {
        self.path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| self.path.to_string_lossy().to_string())
    }

    /// 경로가 이 폴더 아래에 있는지 확인
    pub fn contains_path(&self, path: &Path) -> bool {
        path.starts_with(&self.path)
    }

    /// 지원되는 파일 확장자 목록
    pub fn supported_extensions() -> &'static [&'static str] {
        &[
            "hwpx", "hwp", "docx", "doc", "xlsx", "xls", "pdf", "txt", "md",
        ]
    }

    /// 경로가 지원되는 파일인지 확인
    pub fn is_supported_file(path: &Path) -> bool {
        path.extension()
            .map(|ext| {
                let ext_lower = ext.to_string_lossy().to_lowercase();
                Self::supported_extensions()
                    .iter()
                    .any(|&supported| supported == ext_lower)
            })
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_folder_creation() {
        let folder =
            WatchedFolder::new(PathBuf::from("C:\\Users\\Test\\Documents"), 1234567890).unwrap();

        assert_eq!(folder.name(), "Documents");
        assert!(!folder.is_favorite());
    }

    #[test]
    fn test_blocked_paths() {
        // Windows 시스템 폴더
        assert!(WatchedFolder::new(PathBuf::from("C:\\Windows\\System32"), 0).is_err());

        // node_modules
        assert!(WatchedFolder::new(PathBuf::from("C:\\project\\node_modules"), 0).is_err());

        // .git
        assert!(WatchedFolder::new(PathBuf::from("C:\\project\\.git"), 0).is_err());

        // target (빌드 디렉토리)
        assert!(WatchedFolder::new(PathBuf::from("C:\\project\\target"), 0).is_err());

        // 다중 컴포넌트: appdata\local\temp
        assert!(
            WatchedFolder::new(PathBuf::from("C:\\Users\\User\\AppData\\Local\\Temp"), 0).is_err()
        );
    }

    #[test]
    fn test_blocked_paths_no_false_positives() {
        // "windows"가 폴더명 일부에 포함되지만 독립 컴포넌트가 아닌 경우 → 허용
        assert!(
            WatchedFolder::new(PathBuf::from("C:\\Users\\Test\\my-windows-backup"), 0).is_ok()
        );

        // "target"이 폴더명 일부에 포함 → 허용
        assert!(WatchedFolder::new(PathBuf::from("D:\\target-market\\docs"), 0).is_ok());

        // "git"이 폴더명 일부에 포함 → 허용 (.git은 차단하지만 git-repos는 아님)
        assert!(WatchedFolder::new(PathBuf::from("C:\\Users\\Test\\git-repos"), 0).is_ok());

        // "program"이 포함되지만 "program files"와 다름 → 허용
        assert!(WatchedFolder::new(PathBuf::from("C:\\Users\\Test\\programs"), 0).is_ok());

        // appdata가 있지만 local\temp 시퀀스가 아님 → 허용
        assert!(
            WatchedFolder::new(PathBuf::from("C:\\Users\\User\\AppData\\Roaming\\MyApp"), 0)
                .is_ok()
        );
    }

    #[test]
    fn test_favorite_toggle() {
        let mut folder =
            WatchedFolder::new(PathBuf::from("C:\\Users\\Test\\Documents"), 0).unwrap();

        assert!(!folder.is_favorite());
        folder.toggle_favorite();
        assert!(folder.is_favorite());
        folder.toggle_favorite();
        assert!(!folder.is_favorite());
    }

    #[test]
    fn test_contains_path() {
        let folder = WatchedFolder::new(PathBuf::from("C:\\Users\\Test\\Documents"), 0).unwrap();

        assert!(folder.contains_path(Path::new("C:\\Users\\Test\\Documents\\file.txt")));
        assert!(folder.contains_path(Path::new("C:\\Users\\Test\\Documents\\sub\\file.txt")));
        assert!(!folder.contains_path(Path::new("C:\\Users\\Test\\Other\\file.txt")));
    }

    #[test]
    fn test_supported_file() {
        assert!(WatchedFolder::is_supported_file(Path::new("test.docx")));
        assert!(WatchedFolder::is_supported_file(Path::new("test.HWPX")));
        assert!(WatchedFolder::is_supported_file(Path::new("test.pdf")));
        assert!(!WatchedFolder::is_supported_file(Path::new("test.exe")));
        assert!(!WatchedFolder::is_supported_file(Path::new("test.jpg")));
    }
}
