//! 앱 전역 상수 정의

/// 지원하는 파일 확장자 목록
pub const SUPPORTED_EXTENSIONS: &[&str] = &["txt", "md", "hwpx", "docx", "xlsx", "xls", "pdf"];

/// FTS snippet 컨텍스트 토큰 수 (검색 결과 하이라이트 주변 문자 수)
pub const SNIPPET_CONTEXT_TOKENS: i32 = 32;

/// 청크 최대 문자 수 (인덱싱 시 문서 분할 단위)
pub const CHUNK_MAX_CHARS: usize = 1000;

/// 청크 오버랩 문자 수
pub const CHUNK_OVERLAP_CHARS: usize = 200;

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
