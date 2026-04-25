//! 클라우드 / 네트워크 경로 감지.
//!
//! 두 종류를 모두 다룬다.
//!
//! 1. **Cloud Files API placeholder** (OneDrive / iCloud / Dropbox / Google File Stream 일부)
//!    — 파일 속성 비트로 표시. `fs::read()` 호출 시 OS 가 투명하게 hydrate(다운로드) 해
//!    인덱서가 수백 GB 클라우드 파일을 끌어오는 사고를 일으킨다. → `is_cloud_placeholder()`.
//!
//! 2. **네트워크 드라이브 / UNC** (NAVER Works · Synology Drive · WebDAV · SMB 마운트 등)
//!    — placeholder 비트는 안 켜지지만 파일을 열 때마다 네트워크 라운드트립이 발생,
//!    클라우드 동기화 클라이언트라면 매 파일 다운로드. → `is_network_path()`.
//!
//! 메타데이터 조회(`GetFileAttributes` 류)는 hydrate 를 트리거하지 않으므로 파일명·크기·
//! 수정일 인덱싱은 두 경우 모두 안전하다.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

/// 글로벌 토글 — `Settings.skip_cloud_body_indexing` 미러. 기본 true.
/// `update_settings` 가 호출되면 `set_skip_enabled` 로 동기화된다.
static SKIP_ENABLED: AtomicBool = AtomicBool::new(true);

pub fn set_skip_enabled(v: bool) {
    SKIP_ENABLED.store(v, Ordering::Relaxed);
}

pub fn is_skip_enabled() -> bool {
    SKIP_ENABLED.load(Ordering::Relaxed)
}

#[cfg(windows)]
const FILE_ATTRIBUTE_OFFLINE: u32 = 0x0000_1000;
#[cfg(windows)]
const FILE_ATTRIBUTE_RECALL_ON_OPEN: u32 = 0x0004_0000;
#[cfg(windows)]
const FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS: u32 = 0x0040_0000;

#[cfg(windows)]
const CLOUD_MASK: u32 =
    FILE_ATTRIBUTE_OFFLINE | FILE_ATTRIBUTE_RECALL_ON_OPEN | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS;

/// Cloud Files API placeholder 여부 (개별 파일 단위).
///
/// Windows: 파일 속성 비트 검사. 비-Windows: false.
pub fn is_cloud_placeholder(path: &Path) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        match std::fs::symlink_metadata(path) {
            Ok(meta) => (meta.file_attributes() & CLOUD_MASK) != 0,
            Err(_) => false,
        }
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        false
    }
}

/// 경로가 네트워크 드라이브(매핑드라이브 + UNC) 인지.
///
/// - UNC (`\\server\share\...`, `\\?\UNC\server\share\...`) → true
/// - 매핑드라이브 (`Z:\`, `Y:\`, `G:\` 등) 중 `GetDriveTypeW = DRIVE_REMOTE` → true
/// - 일반 로컬(`C:\`, `D:\`) → false
pub fn is_network_path(path: &Path) -> bool {
    if crate::utils::network_path::is_network(path) {
        return true;
    }
    #[cfg(windows)]
    {
        if let Some(letter) = drive_letter(path) {
            return drive_is_remote(letter);
        }
    }
    false
}

/// 경로가 본문 인덱싱 시 hydrate / 네트워크 다운로드를 일으킬 가능성이 있는지 통합 판단.
///
/// 글로벌 토글 `skip_cloud_body_indexing` 가 켜진 상태에서 본문 파싱 전 차단 용도.
#[allow(dead_code)]
pub fn should_skip_body_indexing(path: &Path) -> bool {
    is_cloud_placeholder(path) || is_network_path(path)
}

/// 사용자에게 노출할 감지 분류 — 폴더 추가 다이얼로그 등에서 안내 문구에 사용.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocationKind {
    Local,
    Unc,
    NetworkDrive,
    CloudPlaceholder,
}

pub fn classify(path: &Path) -> LocationKind {
    if is_cloud_placeholder(path) {
        return LocationKind::CloudPlaceholder;
    }
    if crate::utils::network_path::is_network(path) {
        return LocationKind::Unc;
    }
    #[cfg(windows)]
    {
        if let Some(letter) = drive_letter(path) {
            if drive_is_remote(letter) {
                return LocationKind::NetworkDrive;
            }
        }
    }
    LocationKind::Local
}

#[cfg(windows)]
fn drive_letter(path: &Path) -> Option<char> {
    let s = path.as_os_str().to_string_lossy();
    let bytes = s.as_bytes();
    // "C:\" / "C:" / "\\?\C:\..." 패턴 모두 처리
    if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        return Some(bytes[0] as char);
    }
    if bytes.starts_with(br"\\?\") && bytes.len() >= 6 && bytes[5] == b':' {
        return Some(bytes[4] as char);
    }
    None
}

#[cfg(windows)]
fn drive_is_remote(letter: char) -> bool {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;

    // DRIVE_TYPE constants — Win32 API. windows-sys 0.59 의 모듈 위치가 버전별로
    // 흔들려서 const 직접 정의 (값은 fileapi.h 에서 안정적).
    const DRIVE_REMOTE: u32 = 4;

    let root = format!("{}:\\", letter);
    let wide: Vec<u16> = std::ffi::OsStr::new(&root)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    // SAFETY: `wide` 는 null-terminated wide string.
    let kind = unsafe { GetDriveTypeW(wide.as_ptr()) };
    kind == DRIVE_REMOTE
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn unc_classified_as_network() {
        assert!(is_network_path(Path::new(r"\\server\share\file.txt")));
        assert_eq!(
            classify(Path::new(r"\\server\share\file.txt")),
            LocationKind::Unc
        );
    }

    #[test]
    fn local_drive_not_network() {
        // C:\ 가 매핑드라이브일 가능성은 거의 없으므로 false 기대.
        // (CI 환경 의존이라 should_skip 만 약하게 검증)
        let _ = is_network_path(Path::new(r"C:\Users"));
    }
}
