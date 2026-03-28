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
    "$windows.~bt",
    "$windows.~ws",
    "$winreagent",
    "$getcurrent",
    "$sysreset",
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

/// 통합 경로 안전성 검증
///
/// BLOCKED_PATH_PATTERNS + DEFAULT_EXCLUDED_DIRS를 모두 검사하여
/// 시스템/보호 경로 접근을 차단합니다.
#[allow(dead_code)] // IndexService/FolderService에서 기존 로직 마이그레이션 시 활용
pub fn is_blocked_path(path: &std::path::Path) -> bool {
    let path_str = path.to_string_lossy().to_lowercase();

    // 1. BLOCKED_PATH_PATTERNS 체크 (부분 경로 매치)
    for pattern in BLOCKED_PATH_PATTERNS {
        if path_str.contains(&pattern.to_lowercase()) {
            return true;
        }
    }

    // 2. 경로 컴포넌트 기반 체크 (DEFAULT_EXCLUDED_DIRS의 시스템 폴더)
    for component in path.components() {
        if let std::path::Component::Normal(name) = component {
            let name_lower = name.to_string_lossy().to_lowercase();
            // 시스템 폴더만 체크 (node_modules 등 개발 폴더는 인덱싱 제외 전용)
            let system_dirs = [
                "windows", "$recycle.bin", "system volume information",
                "recovery", "programdata",
            ];
            if system_dirs.contains(&name_lower.as_str()) {
                return true;
            }
        }
    }

    false
}
