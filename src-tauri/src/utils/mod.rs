pub mod disk_info;
pub mod idle_detector;

/// PowerShell `-EncodedCommand`용 Base64(UTF-16LE) 인코딩.
/// 문자열 보간 기반 인젝션을 원천 차단한다.
pub fn encode_powershell_command(script: &str) -> String {
    use base64::Engine;
    let utf16le: Vec<u8> = script
        .encode_utf16()
        .flat_map(|c| c.to_le_bytes())
        .collect();
    base64::engine::general_purpose::STANDARD.encode(&utf16le)
}
