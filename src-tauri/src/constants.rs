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
    "dll",
    "exe",
    "sys",
    "drv",
    "ocx",
    "cpl",
    "scr", // 임시/캐시 파일
    "tmp",
    "temp",
    "bak",
    "old",
    "cache", // 빌드 아티팩트
    "obj",
    "pdb",
    "ilk",
    "exp",
    "lib",
    "pch", // 로그 파일
    "log",
    "etl",
    "jsonl", // 설정/레지스트리
    "ini",
    "reg", // 데이터베이스/인덱스
    "db",
    "db-shm",
    "db-wal",
    "sqlite",
    "sqlite-shm",
    "sqlite-wal",
    "idx",
    "ldb",
    // 미디어 (파일명은 필요할 수 있지만 대량 존재 시 제외)
    "mp3",
    "mp4",
    "avi",
    "mkv",
    "mov",
    "wmv",
    "flv",
    "m4v",
    "m4a",
    "aac",
    "wav",
    "flac",
    // 압축 파일 (지원 파서 없음)
    "zip",
    "rar",
    "7z",
    "tar",
    "gz",
    "bz2",
    "xz", // 기타 바이너리
    "bin",
    "dat",
    "iso",
    "img",
    "pb",
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
    // 개발 도구 - 빌드/의존성
    "node_modules",
    ".git",
    "__pycache__",
    ".venv",
    "venv",
    "target",
    ".tox",
    "dist",
    "build",
    "out",
    ".next",
    ".nuxt",
    ".svelte-kit",
    ".vite",
    ".parcel-cache",
    ".turbo",
    ".cache",
    "coverage",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
    // 에디터/IDE
    ".vscode",
    ".idea",
    // AI/CLI 도구 (세션 로그로 초당 수십 번 쓰여 watcher 노이즈 유발)
    ".claude",
    ".codex",
    ".gemini",
    ".cursor",
    ".aider",
    // 사용자 캐시
    "appdata",
];

/// 접근 차단 경로 패턴 (Path Traversal 방지)
///
/// Windows + macOS 시스템 폴더 및 보호된 영역을 블랙리스트로 관리.
/// `is_blocked_path` 가 to_lowercase() 후 contains 매칭하므로 모든 패턴은 lowercase.
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
    // macOS 시스템 영역
    // 주의: `~/Library/...` 도 contains "/library/" 에 매치되면 앱 데이터까지 막힘.
    // 그래서 `/library/` 단독 매치는 막고, 시스템 root 의 `/Library/` 만 차단하기 위해
    // path component 기반 체크(system_dirs) 에 추가하지 않고 prefix 형태로만 둔다.
    "/system/library/",
    "/system/applications/",
    "/private/var/",
    "/private/etc/",
    "/usr/bin/",
    "/usr/sbin/",
    "/usr/lib/",
    "/usr/local/bin/",
    "/.trashes/",
    "/.spotlight-v100/",
    "/.fseventsd/",
];

/// 통합 경로 안전성 검증
///
/// BLOCKED_PATH_PATTERNS + DEFAULT_EXCLUDED_DIRS를 모두 검사하여
/// 시스템/보호 경로 접근을 차단합니다.
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

/// 드라이브 루트 여부 판정 (Windows: `C:\`, `D:\` 등)
///
/// 드라이브 전체 감시는 notify 이벤트 폭주 + 시스템 리소스 고갈을 유발하므로
/// 인덱싱 대상에서 제외한다.
pub fn is_drive_root(path: &std::path::Path) -> bool {
    let s = path.to_string_lossy();
    // `\\?\C:\` prefix 제거
    let normalized = s.strip_prefix(r"\\?\").unwrap_or(&s);
    // `C:\` 또는 `C:/` (길이 2~3, 두번째 문자 `:`)
    let chars: Vec<char> = normalized.chars().collect();
    if chars.len() > 3 {
        return false;
    }
    chars.len() >= 2 && chars[1] == ':'
}

/// 감시 폴더 등록 가능 여부 검증
///
/// 실패 시 사용자에게 보여줄 한국어 사유 반환. `add_folder`/`reindex_folder`/
/// `resume_indexing`/`start_indexing_batch` 등 인덱싱 진입점에서 `canonicalize()`
/// 직후 호출한다.
///
/// 드라이브 루트(`C:\`, `D:\`)는 이 앱의 Everything 스타일 검색 설계상 허용한다.
/// 단 호출부에서 `is_drive_root`로 감지해 벡터 자동 시작 스킵 + 경고를 띄워야 한다.
pub fn validate_watch_path(path: &std::path::Path) -> Result<(), &'static str> {
    if is_blocked_path(path) {
        return Err("시스템 보호 폴더는 감시할 수 없습니다.");
    }
    Ok(())
}
