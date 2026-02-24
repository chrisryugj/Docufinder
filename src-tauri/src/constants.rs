//! 앱 전역 상수 정의

/// 지원하는 파일 확장자 목록
pub const SUPPORTED_EXTENSIONS: &[&str] = &["txt", "md", "hwpx", "docx", "xlsx", "xls", "pdf"];

// ============================================
// 보안 관련 상수
// ============================================

/// 접근 차단 경로 패턴 (Path Traversal 방지)
///
/// Windows 시스템 폴더 및 보호된 영역을 블랙리스트로 관리
pub const BLOCKED_PATH_PATTERNS: &[&str] = &[
    // Windows 시스템 폴더
    "\\windows\\",
    "\\program files\\",
    "\\program files (x86)\\",
    "\\programdata\\",
    "\\$recycle.bin\\",
    "\\system volume information\\",
    // Unix 스타일 경로 (WSL 등 호환)
    "/windows/",
    "/program files/",
    "/program files (x86)/",
    "/programdata/",
];
