//! 네트워크 경로(UNC, 매핑드라이브) 헬퍼.
//!
//! Windows 의 `canonicalize()` 는 UNC 경로를 `\\?\UNC\server\share\...` 로
//! 변환하는데, 이 prefix 를 단순 `strip("\\?\")` 만 하면 `UNC\server\...` 라는
//! 깨진 문자열이 남는다. 또한 SMB/매핑드라이브는 `notify` 의 inotify 류
//! 이벤트 파이프가 동작하지 않거나 누락이 잦아 PollWatcher 분기가 필요하다.
//!
//! 이 모듈은 두 가지 헬퍼만 제공한다:
//!   * `simplify(p)`   — `\\?\UNC\srv\share\...` → `\\srv\share\...` 정규화
//!   * `is_network(p)` — `\\` UNC prefix 감지(매핑드라이브는 보수적으로 false)

use std::path::{Path, PathBuf};

/// `\\?\` / `\\?\UNC\` 등 extended-length prefix 를 제거해 외부 도구·DB 와 일관된 경로로 만든다.
/// 내부적으로 `dunce::simplified` 를 사용 — Microsoft 공식 알고리즘과 동등.
pub fn simplify(path: &Path) -> PathBuf {
    dunce::simplified(path).to_path_buf()
}

/// best-effort 경로 정규화 — 인덱싱/검증 진입점이 공유한다(이슈 #29).
///
/// 1. 매핑 네트워크 드라이브(`Y:\`)는 UNC(`\\srv\share`)로 치환 — elevated 세션에서
///    드라이브 매핑이 보이지 않아 `os error 5` 로 막히는 문제를 우회한다.
/// 2. `dunce::canonicalize` 로 정규화(심볼릭 링크 해소 + `\\?\` prefix 회피).
/// 3. 실패(일부 SMB 서버가 핸들 오픈을 거부하는 `os error 5` 등) 시 원본 경로로
///    폴백한다 — `read_dir` 열거(=탐색기/PowerShell)는 가능하므로 정규화 실패가
///    인덱싱 전체를 막아선 안 된다(이슈 #29: resume/reindex/periodic_sync 차단 회귀).
pub fn canonicalize_best_effort(path: &Path) -> PathBuf {
    let resolved = resolve_mapped_drive_to_unc(path).unwrap_or_else(|| path.to_path_buf());
    match dunce::canonicalize(&resolved) {
        Ok(c) => c,
        Err(e) => {
            // os error 5(액세스 거부)는 환경 의존 원인이 많아 상세 진단을 함께 남긴다.
            if e.raw_os_error() == Some(5) {
                tracing::warn!(
                    "경로 정규화 실패, 원본 경로 사용\n{}",
                    crate::utils::access_diag::context(
                        &resolved,
                        "path-normalize",
                        "dunce::canonicalize",
                        &e
                    )
                );
            } else {
                tracing::warn!(
                    "경로 정규화 실패, 원본 경로 사용 ({}): {e}",
                    resolved.display()
                );
            }
            resolved
        }
    }
}

/// 현재 사용자 세션에 매핑된 네트워크 드라이브 전체를 `(드라이브문자, UNC base)` 로 수집.
/// resume skip 비교에서 드라이브 표현과 UNC 표현을 통일하기 위한 변환 테이블이다(이슈 #34).
/// 비-Windows 에서는 항상 빈 목록.
#[cfg(windows)]
pub fn network_drive_map() -> Vec<(char, String)> {
    ('A'..='Z')
        .filter_map(|c| mapped_drive_unc_base(c).map(|base| (c, base)))
        .collect()
}

#[cfg(not(windows))]
pub fn network_drive_map() -> Vec<(char, String)> {
    Vec::new()
}

/// 경로를 "비교용 정규형" 문자열로 변환한다(이슈 #34).
///
/// 같은 네트워크 폴더라도 세션/시점에 따라 매핑드라이브(`Z:\docs`)·UNC(`\\srv\share\docs`)·
/// `\\?\` prefix 변형으로 표현이 흔들려 resume skip 매칭이 실패(→ 전체 재인덱싱)하는 문제를
/// 막기 위해, 모든 표현을 하나로 수렴시킨다:
///   1. `\\?\` / `\\?\UNC\` extended-length prefix 제거(`simplify`)
///   2. `/` → `\` 통일
///   3. 매핑 네트워크 드라이브면 `drive_map` 으로 UNC base 치환(`Z:\docs` → `\\srv\share\docs`)
///   4. trailing separator 제거 + 소문자화(Windows 경로는 case-insensitive)
pub fn normalize_for_compare(path: &Path, drive_map: &[(char, String)]) -> String {
    let simplified = simplify(path);
    let s = simplified.to_string_lossy().replace('/', "\\");
    let bytes = s.as_bytes();
    let mut out = if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        let letter = (bytes[0] as char).to_ascii_uppercase();
        match drive_map.iter().find(|(c, _)| *c == letter) {
            Some((_, base)) => {
                let rest = s[2..].trim_start_matches('\\');
                let base = base.trim_end_matches('\\');
                if rest.is_empty() {
                    base.to_string()
                } else {
                    format!("{base}\\{rest}")
                }
            }
            None => s,
        }
    } else {
        s
    };
    while out.ends_with('\\') {
        out.pop();
    }
    out.to_ascii_lowercase()
}

/// 경로가 UNC(`\\server\share\...`) 인지 검사.
/// 매핑드라이브(예: `Z:\...`) 는 OS 가 추상화하므로 여기서는 false 로 두고,
/// 호출자가 필요하면 `GetDriveTypeW` 로 별도 판정한다.
pub fn is_network(path: &Path) -> bool {
    let s = path.as_os_str();
    let bytes = s.to_string_lossy();
    bytes.starts_with(r"\\") && !bytes.starts_with(r"\\?\") || bytes.starts_with(r"\\?\UNC\")
}

/// 매핑 네트워크 드라이브(`Y:\sub\dir`)를 UNC(`\\server\share\sub\dir`)로 변환.
///
/// UAC elevated 프로세스에서는 일반 logon 세션이 만든 드라이브 매핑(DosDevices
/// symbolic link)이 보이지 않아 `Y:\` 접근이 `os error 5`(액세스 거부)로 막힌다
/// (이슈 #29). UNC 경로는 드라이브 매핑 계층을 우회하므로 elevated 에서도 (네트워크
/// 자격증명이 유효하면) 접근 가능하다. 매핑된 네트워크 드라이브가 아니거나(로컬·이미
/// UNC) 매핑 정보를 못 찾으면 `None` — 호출자는 원본 경로를 그대로 쓰면 된다.
pub fn resolve_mapped_drive_to_unc(path: &Path) -> Option<PathBuf> {
    let letter = drive_letter(path)?;
    let base = mapped_drive_unc_base(letter)?;
    Some(rebuild_with_unc_base(path, &base))
}

/// 경로 맨 앞의 드라이브 문자 추출 (`Y:\foo` → `Y`). 드라이브 경로가 아니면 None.
fn drive_letter(path: &Path) -> Option<char> {
    let s = path.as_os_str().to_string_lossy();
    let b = s.as_bytes();
    if b.len() >= 2 && b[1] == b':' && b[0].is_ascii_alphabetic() {
        Some((b[0] as char).to_ascii_uppercase())
    } else {
        None
    }
}

/// 드라이브 prefix(`X:`)를 UNC base 로 치환해 전체 경로 재구성.
/// `Y:\foo\bar` + `\\srv\share` → `\\srv\share\foo\bar`.
/// `drive_letter` 가 Some 을 준 경로에만 적용되므로 `s[2..]` 는 드라이브 뒤 나머지다.
fn rebuild_with_unc_base(path: &Path, unc_base: &str) -> PathBuf {
    let s = path.as_os_str().to_string_lossy();
    let rest = s[2..].trim_start_matches(['\\', '/']);
    let base = unc_base.trim_end_matches(['\\', '/']);
    if rest.is_empty() {
        PathBuf::from(base)
    } else {
        PathBuf::from(format!("{base}\\{rest}"))
    }
}

/// 드라이브 문자 → 매핑된 UNC base (`\\server\share`). 매핑 네트워크 드라이브가
/// 아니거나 조회 실패 시 None.
#[cfg(windows)]
fn mapped_drive_unc_base(letter: char) -> Option<String> {
    // 1차: HKCU\Network\<letter>\RemotePath — 레지스트리는 logon session 에 묶이지
    //      않아 같은 사용자라면 elevated 토큰에서도 읽힌다(가장 안정적). 이 키의
    //      존재 자체가 "매핑 네트워크 드라이브"라는 권위 있는 증거다.
    //
    //      GetDriveTypeW(DRIVE_REMOTE) 로 게이트하지 않는다 — elevated 프로세스에선
    //      일반 세션의 매핑이 안 보여 REMOTE 가 아니게 나오고, 그러면 정작 UNC 우회가
    //      필요한 바로 그 상황에서 변환을 막아버리기 때문(이슈 #29).
    registry_remote_path(letter)
        // 2차: WNetGetConnectionW — 호출 세션 기준이라 elevated 에선 실패할 수
        //      있으므로 폴백으로만.
        .or_else(|| wnet_connection(letter))
        .filter(|s| s.starts_with(r"\\"))
}

#[cfg(not(windows))]
fn mapped_drive_unc_base(_letter: char) -> Option<String> {
    None
}

#[cfg(windows)]
fn wide(s: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    std::ffi::OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// `HKEY_CURRENT_USER\Network\<letter>\RemotePath` (REG_SZ) 읽기.
#[cfg(windows)]
fn registry_remote_path(letter: char) -> Option<String> {
    use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER};

    // 값/플래그는 winreg.h 안정값 — cloud_detect 관례대로 직접 정의(모듈 위치 흔들림 회피).
    const RRF_RT_REG_SZ: u32 = 0x0000_0002;
    const ERROR_SUCCESS: u32 = 0;

    let subkey = wide(&format!(r"Network\{letter}"));
    let value = wide("RemotePath");
    // RemotePath 는 짧은 UNC 문자열이라 고정 버퍼로 1-pass (UNC 가 2KB 넘을 일 없음).
    let mut buf = [0u16; 1024];
    let mut cb = std::mem::size_of_val(&buf) as u32;
    // SAFETY: null-terminated wide ptr + REG_SZ 강제. buf/cb 크기 일치.
    let rc = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            buf.as_mut_ptr() as *mut core::ffi::c_void,
            &mut cb,
        )
    };
    if rc != ERROR_SUCCESS {
        return None;
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    let s = String::from_utf16_lossy(&buf[..len]);
    (!s.is_empty()).then_some(s)
}

/// `WNetGetConnectionW("X:")` → 매핑 UNC. elevated 세션에 매핑이 없으면 실패(폴백용).
#[cfg(windows)]
fn wnet_connection(letter: char) -> Option<String> {
    use windows_sys::Win32::NetworkManagement::WNet::WNetGetConnectionW;
    const ERROR_SUCCESS: u32 = 0;

    let local = wide(&format!("{letter}:"));
    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    // SAFETY: null-terminated local name + buf/len 일치.
    let rc = unsafe { WNetGetConnectionW(local.as_ptr(), buf.as_mut_ptr(), &mut len) };
    if rc != ERROR_SUCCESS {
        return None;
    }
    let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    let s = String::from_utf16_lossy(&buf[..n]);
    (!s.is_empty()).then_some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unc_detected() {
        assert!(is_network(Path::new(r"\\server\share\file.txt")));
        assert!(is_network(Path::new(r"\\?\UNC\server\share\file.txt")));
    }

    #[test]
    fn local_not_network() {
        assert!(!is_network(Path::new(r"C:\Users\foo")));
        assert!(!is_network(Path::new(r"\\?\C:\Users\foo")));
    }

    #[test]
    fn drive_letter_parsed() {
        assert_eq!(drive_letter(Path::new(r"Y:\foo\bar")), Some('Y'));
        assert_eq!(drive_letter(Path::new(r"y:\foo")), Some('Y')); // 대문자 정규화
        assert_eq!(drive_letter(Path::new(r"Z:")), Some('Z'));
        assert_eq!(drive_letter(Path::new(r"\\srv\share")), None); // UNC
        assert_eq!(drive_letter(Path::new("/home/x")), None); // posix
        assert_eq!(drive_letter(Path::new("relative\\path")), None);
    }

    #[test]
    fn rebuild_unc_base() {
        assert_eq!(
            rebuild_with_unc_base(Path::new(r"Y:\foo\bar"), r"\\srv\share"),
            PathBuf::from(r"\\srv\share\foo\bar")
        );
        // 드라이브 루트만 → base 그대로
        assert_eq!(
            rebuild_with_unc_base(Path::new(r"Y:\"), r"\\srv\share"),
            PathBuf::from(r"\\srv\share")
        );
        assert_eq!(
            rebuild_with_unc_base(Path::new(r"Y:"), r"\\srv\share"),
            PathBuf::from(r"\\srv\share")
        );
        // base 에 trailing separator 가 있어도 중복 안 생김
        assert_eq!(
            rebuild_with_unc_base(Path::new(r"Z:\a\b"), r"\\s\sh\"),
            PathBuf::from(r"\\s\sh\a\b")
        );
    }

    // 비-Windows 에서는 매핑 조회가 항상 None → 원본 경로 사용(변환 없음).
    #[cfg(not(windows))]
    #[test]
    fn resolve_noop_off_windows() {
        assert_eq!(resolve_mapped_drive_to_unc(Path::new(r"Y:\foo")), None);
    }

    // normalize_for_compare 는 drive_map 을 인자로 받으므로 OS 무관하게 검증 가능(이슈 #34).
    #[test]
    fn normalize_unifies_mapped_drive_and_unc() {
        let map = vec![('Z', r"\\srv\share".to_string())];
        // 매핑드라이브와 UNC 가 같은 비교 키로 수렴해야 resume skip 이 매칭된다.
        assert_eq!(
            normalize_for_compare(Path::new(r"Z:\docs\a.pdf"), &map),
            normalize_for_compare(Path::new(r"\\srv\share\docs\a.pdf"), &map),
        );
        assert_eq!(
            normalize_for_compare(Path::new(r"Z:\docs\a.pdf"), &map),
            r"\\srv\share\docs\a.pdf"
        );
    }

    #[test]
    fn normalize_slash_case_trailing() {
        let map: Vec<(char, String)> = Vec::new();
        // 슬래시 방향·대소문자·trailing separator 차이를 흡수.
        assert_eq!(
            normalize_for_compare(Path::new(r"C:\Foo\Bar\"), &map),
            r"c:\foo\bar"
        );
        assert_eq!(
            normalize_for_compare(Path::new("C:/Foo/Bar"), &map),
            r"c:\foo\bar"
        );
    }

    #[test]
    fn normalize_unmapped_drive_untouched() {
        // drive_map 에 없는 드라이브(로컬 등)는 UNC 변환 없이 표현만 정규화.
        let map = vec![('Z', r"\\srv\share".to_string())];
        assert_eq!(
            normalize_for_compare(Path::new(r"D:\data\x.txt"), &map),
            r"d:\data\x.txt"
        );
    }
}
