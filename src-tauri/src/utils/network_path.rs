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

/// 경로가 UNC(`\\server\share\...`) 인지 검사.
/// 매핑드라이브(예: `Z:\...`) 는 OS 가 추상화하므로 여기서는 false 로 두고,
/// 호출자가 필요하면 `GetDriveTypeW` 로 별도 판정한다.
pub fn is_network(path: &Path) -> bool {
    let s = path.as_os_str();
    let bytes = s.to_string_lossy();
    bytes.starts_with(r"\\") && !bytes.starts_with(r"\\?\")
        || bytes.starts_with(r"\\?\UNC\")
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
}
