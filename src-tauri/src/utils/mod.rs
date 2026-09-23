pub mod access_diag;
pub mod cloud_detect;
pub mod disk_info;
pub mod dll_diag;
pub mod elevation;
pub mod filename_normalize;
pub mod folder_scope;
pub mod idle_detector;
pub mod network_path;
pub mod process_job;
pub mod text_normalize;

pub use text_normalize::normalize_text;

/// 파일이 없다고 확인됐는가: NotFound 이고 상위 폴더는 보일 때만. 상위 폴더까지 안 보이면
/// 오프라인 공유·빠진 USB·끊긴 VPN 일 수 있어 판단하지 않는다. 권한 거부 같은 다른 오류도
/// "없음" 이 아니다. 색인 기록·북마크를 지우기 전의 공통 기준.
pub fn confirmed_missing(path: &std::path::Path) -> bool {
    matches!(std::fs::metadata(path), Err(e) if e.kind() == std::io::ErrorKind::NotFound)
        && path.parent().is_some_and(|p| p.is_dir())
}

/// PowerShell `-EncodedCommand`용 Base64(UTF-16LE) 인코딩.
/// 문자열 보간 기반 인젝션을 원천 차단한다.
/// lite 빌드는 PowerShell 을 아예 호출하지 않는다 (`disk_info` 주석 참고).
#[cfg(all(windows, feature = "online"))]
pub fn encode_powershell_command(script: &str) -> String {
    use base64::Engine;
    let utf16le: Vec<u8> = script
        .encode_utf16()
        .flat_map(|c| c.to_le_bytes())
        .collect();
    base64::engine::general_purpose::STANDARD.encode(&utf16le)
}

#[cfg(test)]
mod tests {
    use super::confirmed_missing;

    #[test]
    fn confirmed_missing_only_when_parent_is_visible() {
        let tmp = tempfile::tempdir().unwrap();
        let present = tmp.path().join("a.txt");
        std::fs::write(&present, "x").unwrap();
        assert!(!confirmed_missing(&present));
        assert!(confirmed_missing(&tmp.path().join("deleted.txt")));
        // 상위 폴더째 안 보이면(오프라인 공유 등) 없다고 단정하지 않는다
        assert!(!confirmed_missing(
            &tmp.path().join("offline-share").join("b.hwpx")
        ));
    }
}
