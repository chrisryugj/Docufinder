//! 앱 전역 상수 정의

/// 지원하는 파일 확장자 목록
/// 참고: "hwp"는 파서 미지원 (파싱 실패 시 변환 대상으로 수집됨, pipeline.rs 참조)
pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "txt", "md", "hwpx", "hwp", "docx", "pptx", "xlsx", "xls", "pdf",
];

/// OCR 대상 이미지 확장자 (ocr_enabled 설정 시에만 인덱싱)
pub const OCR_IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "bmp", "tiff", "tif"];

/// 메타데이터(파일명) 저장에서 제외할 확장자
///
/// 전체 드라이브 인덱싱 시 DLL/EXE/SYS 등 수십만 개의 시스템 바이너리가
/// DB에 저장되어 파일명 검색 노이즈 증가 + DB 급팽창을 유발.
/// 이 확장자들은 파일명 검색 대상에서 제외하여 DB 크기와 검색 품질을 개선.
pub const METADATA_EXCLUDED_EXTENSIONS: &[&str] = &[
    // Windows 바이너리/시스템 파일
    "dll", "exe", "sys", "drv", "ocx", "cpl", "scr", // 임시/캐시 파일
    "tmp", "temp", "bak", "old", "cache", // 빌드 아티팩트
    "obj", "pdb", "ilk", "exp", "lib", "pch", // 로그 파일
    "log", "etl", // 설정/레지스트리
    "ini", "reg", // 데이터베이스/인덱스
    "db", "db-shm", "db-wal", "idx", "ldb",
    // 미디어 (파일명은 필요할 수 있지만 대량 존재 시 제외)
    "mp3", "mp4", "avi", "mkv", "mov", "wmv", "flv", "m4v", "m4a", "aac", "wav", "flac",
    // 압축 파일 (지원 파서 없음)
    "zip", "rar", "7z", "tar", "gz", "bz2", "xz", // 기타 바이너리
    "bin", "dat", "iso", "img",
];

// ============================================
// 파일 크기 제한
// ============================================

/// 단일 파일 최대 크기 기본값 (MB)
///
/// 일반 업무 문서 99%를 커버하면서 인덱싱 지연을 최소화하는 균형점.
/// 이 값 초과 파일은 인덱싱에서 스킵됨 (사용자 설정으로 변경 가능).
pub const DEFAULT_MAX_FILE_SIZE_MB: u64 = 200;

/// 단일 파일 크기 절대 상한 (MB)
///
/// 사용자가 설정할 수 있는 최대값 + 파서 단 하드캡.
/// 이 값 초과 파일은 설정과 무관하게 파서에서 거부됨 (메모리/성능 보호).
pub const MAX_FILE_SIZE_LIMIT_MB: u64 = 500;

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
                "windows",
                "$recycle.bin",
                "system volume information",
                "recovery",
                "programdata",
            ];
            if system_dirs.contains(&name_lower.as_str()) {
                return true;
            }
        }
    }

    false
}
