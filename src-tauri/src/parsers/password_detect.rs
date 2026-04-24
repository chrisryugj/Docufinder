//! 암호 보호 파일 사전 감지
//!
//! 목적: 파서(특히 kordoc Node.js 사이드카) 호출 **전**에 암호 걸린 파일을 감지하여
//! kordoc 내부에서 호출할 수 있는 외부 프로그램(한컴 오피스 COM 등)이
//! 시스템 모달 다이얼로그("암호를 입력하세요")를 띄우는 걸 차단한다.
//!
//! 지원 포맷:
//! - HWP  (HWP5 / CFB) — FileHeader stream의 properties 플래그 (bit 1 = 암호)
//! - HWPX (ZIP + ODF-like) — META-INF/manifest.xml 의 encryption-data 요소
//! - DOCX / XLSX / PPTX (OOXML) — 정상은 ZIP, 암호화되면 CFB 포맷으로 래핑됨
//! - PDF — trailer 사전의 /Encrypt 키
//!
//! 감지되지 않더라도 각 파서 내부의 기존 fallback(calamine/zip 에러 메시지 기반)이
//! 남아있어 안전망 역할을 한다. 여기서는 **kordoc 호출 전 차단**이 핵심 목표.

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

/// OLE2 Compound File Binary (CFB) 매직 바이트.
const CFB_MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

/// HWP5 FileHeader stream의 signature 프리픽스.
/// CFB 내부 어디엔가 저장되므로 파일 앞부분을 byte-search 한다.
const HWP5_SIGNATURE: &[u8] = b"HWP Document File";

/// HWP5 암호 감지를 위해 읽을 최대 바이트. CFB 헤더 + FAT + FileHeader stream은
/// 통상 파일 앞 64KB 이내에 위치. 안전 마진으로 1MB.
const HWP5_SCAN_LIMIT: usize = 1024 * 1024;

/// PDF trailer 검색을 위한 tail 크기 (32KB).
/// 표준 trailer는 파일 끝 근처에 있으며 %%EOF 앞 수 KB 이내.
const PDF_TAIL_SCAN: u64 = 32 * 1024;

/// HWPX/ODF manifest 경로. 모든 OOXML 변종은 META-INF 아래 manifest.xml 사용.
const ODF_MANIFEST_NAME: &str = "META-INF/manifest.xml";

/// 파일 확장자로 분기하여 암호 보호 여부 사전 감지.
///
/// - 지원 확장자: `hwp`, `hwpx`, `docx`, `xlsx`, `pptx`, `pdf`
/// - 그 외 확장자 (xls/ppt/doc 레거시 포함) → `false` 리턴 후 파서 내부 감지에 위임
/// - 감지 중 IO 에러 등은 조용히 `false` 리턴 (보수적: 암호 아님으로 가정 → 기존 파서가 에러 처리)
pub fn is_password_protected(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    match ext.as_str() {
        "hwp" => hwp5_is_encrypted(path).unwrap_or(false),
        "hwpx" => hwpx_is_encrypted(path).unwrap_or(false),
        // OOXML: 정상일 때 ZIP(PK\x03\x04), 암호화되면 OLE2 CFB로 래핑됨.
        // 레거시 xls/ppt/doc는 원래 CFB라 이 휴리스틱으로 구분 불가 → 기존 파서 에러 기반 감지 유지.
        "docx" | "xlsx" | "pptx" => has_cfb_magic(path).unwrap_or(false),
        "pdf" => pdf_is_encrypted(path).unwrap_or(false),
        _ => false,
    }
}

/// 파일 앞 8바이트가 CFB 매직인지 검사.
fn has_cfb_magic(path: &Path) -> std::io::Result<bool> {
    let mut file = File::open(path)?;
    let mut header = [0u8; 8];
    if file.read_exact(&mut header).is_err() {
        return Ok(false);
    }
    Ok(header == CFB_MAGIC)
}

/// HWP5 (CFB) 파일의 FileHeader stream에서 properties 플래그를 읽어 암호 여부 판정.
///
/// 구조:
/// - bytes 0..31   : signature "HWP Document File V5.00\0..." (32 byte padding)
/// - bytes 32..35  : version (DWORD, little-endian)
/// - bytes 36..39  : properties (DWORD, little-endian) — bit 1 (0x2) = 암호,
///   bit 4 (0x10) = DRM 보안
///
/// CFB 컨테이너를 정식 파싱하지 않고 파일 버퍼에서 signature byte-search로 위치를 찾는다.
/// false positive 가능성은 매우 낮음 (signature 문자열이 본문 데이터에 우연히 나올 확률 미미).
fn hwp5_is_encrypted(path: &Path) -> std::io::Result<bool> {
    let mut file = File::open(path)?;

    // CFB 시그니처 확인 (빠른 reject)
    let mut magic = [0u8; 8];
    if file.read_exact(&mut magic).is_err() || magic != CFB_MAGIC {
        return Ok(false);
    }

    // 파일 앞부분 읽기 (FileHeader stream은 CFB 구조상 앞쪽에 있음)
    file.seek(SeekFrom::Start(0))?;
    let file_size = file.metadata()?.len() as usize;
    let read_len = file_size.min(HWP5_SCAN_LIMIT);
    let mut buf = vec![0u8; read_len];
    let n = file.read(&mut buf)?;
    buf.truncate(n);

    // "HWP Document File" signature 찾기
    let Some(pos) = find_subsequence(&buf, HWP5_SIGNATURE) else {
        return Ok(false);
    };

    // signature 시작 + 32 (padding 포함) 이후 4byte = properties
    let prop_offset = pos + 32;
    if buf.len() < prop_offset + 4 {
        return Ok(false);
    }
    let properties = u32::from_le_bytes([
        buf[prop_offset],
        buf[prop_offset + 1],
        buf[prop_offset + 2],
        buf[prop_offset + 3],
    ]);

    // bit 1 (암호) | bit 4 (DRM 보안) | bit 8 (공인인증 보안)
    const FLAG_PASSWORD: u32 = 0x0000_0002;
    const FLAG_DRM: u32 = 0x0000_0010;
    const FLAG_CERT_ENC: u32 = 0x0000_0100;
    Ok(properties & (FLAG_PASSWORD | FLAG_DRM | FLAG_CERT_ENC) != 0)
}

/// HWPX (ZIP) 파일의 META-INF/manifest.xml 에서 encryption-data 요소 존재 확인.
///
/// ODF 계열 spec:
/// - 평문: 각 파일 엔트리에 `<manifest:file-entry .../>` 만 존재
/// - 암호: 해당 엔트리 안에 `<manifest:encryption-data>` 자식 요소 포함
///
/// HWPX도 ODF 유사 스펙을 따르므로 이 검사로 대부분 커버된다.
fn hwpx_is_encrypted(path: &Path) -> std::io::Result<bool> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut archive = match zip::ZipArchive::new(reader) {
        Ok(a) => a,
        Err(_) => return Ok(false),
    };

    let mut manifest = match archive.by_name(ODF_MANIFEST_NAME) {
        Ok(m) => m,
        Err(_) => return Ok(false),
    };
    let mut content = String::new();
    if manifest.read_to_string(&mut content).is_err() {
        return Ok(false);
    }

    // encryption-data 요소(네임스페이스 prefix 무관) 또는 encryption 속성.
    // odf:manifest-version 네임스페이스가 섞여 있어도 substring 검색으로 충분.
    Ok(content.contains("encryption-data") || content.contains(":encryption "))
}

/// PDF trailer 사전의 /Encrypt 키 존재 확인.
///
/// PDF trailer는 파일 끝의 `trailer << ... /Encrypt N G R ... >>` 또는
/// cross-reference stream의 /Encrypt 엔트리에 등장. %%EOF 앞 수 KB 내에 있으므로
/// 파일 tail 32KB 만 스캔. 대용량 PDF도 빠르게 판정.
fn pdf_is_encrypted(path: &Path) -> std::io::Result<bool> {
    let mut file = File::open(path)?;
    let file_size = file.metadata()?.len();
    if file_size < 8 {
        return Ok(false);
    }
    let tail_size = PDF_TAIL_SCAN.min(file_size);
    let start = file_size - tail_size;
    file.seek(SeekFrom::Start(start))?;
    let mut buf = Vec::with_capacity(tail_size as usize);
    file.take(tail_size).read_to_end(&mut buf)?;

    Ok(find_subsequence(&buf, b"/Encrypt").is_some())
}

/// haystack 에서 needle 의 첫 등장 위치 반환. 없으면 None.
fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_extension_returns_false() {
        // 가상 경로 — 파일 없어도 확장자만으로 false 리턴
        let path = Path::new("test.txt");
        assert!(!is_password_protected(path));
    }

    #[test]
    fn missing_file_returns_false() {
        // IO 에러 시 조용히 false (암호 아님으로 가정 → 파서 내부 에러 처리에 위임)
        let path = Path::new("C:\\does_not_exist.hwp");
        assert!(!is_password_protected(path));
    }

    #[test]
    fn find_subsequence_basic() {
        assert_eq!(find_subsequence(b"hello world", b"world"), Some(6));
        assert_eq!(find_subsequence(b"hello", b"xyz"), None);
        assert_eq!(find_subsequence(b"", b"abc"), None);
        assert_eq!(find_subsequence(b"abc", b""), None);
    }

    #[test]
    fn hwp5_flag_calculation() {
        // bit 1 = 암호
        assert_eq!(0x02u32 & (0x02 | 0x10 | 0x100), 0x02);
        // bit 4 = DRM
        assert_eq!(0x10u32 & (0x02 | 0x10 | 0x100), 0x10);
        // bit 0 = 압축만 — 암호 아님
        assert_eq!(0x01u32 & (0x02 | 0x10 | 0x100), 0);
    }
}
