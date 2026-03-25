//! 앱 전역 상수 정의

/// 지원하는 파일 확장자 목록
/// 참고: "hwp"는 파서 미지원 (파싱 실패 시 변환 대상으로 수집됨, pipeline.rs 참조)
pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "txt", "md", "hwpx", "hwp", "docx", "pptx", "xlsx", "xls", "pdf",
];

/// OCR 대상 이미지 확장자 (ocr_enabled 설정 시에만 인덱싱)
pub const OCR_IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "bmp", "tiff", "tif"];

// ============================================
// 보안 관련 상수
// ============================================

/// 인덱싱 시 기본 제외 디렉토리 이름 (대소문자 무시 비교)
///
/// 드라이브 단위 인덱싱 시 시스템/빌드/캐시 폴더를 자동 건너뛰기
pub const DEFAULT_EXCLUDED_DIRS: &[&str] = &[
    // Windows 시스템
    "windows",
    "program files",
    "program files (x86)",
    "programdata",
    "$recycle.bin",
    "system volume information",
    "recovery",
    // 개발 도구
    "node_modules",
    ".git",
    "__pycache__",
    ".venv",
    "target",
    ".tox",
    // 사용자 캐시
    "appdata",
];

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
