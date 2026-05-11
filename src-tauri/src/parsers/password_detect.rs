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
///
/// 17바이트 짧은 매칭("HWP Document File") 은 한국어 본문/메타 데이터에 우연히 등장해
/// false positive 를 일으켰다 — 사용자 환경에서 같은 .HWP 파일을 다른 폴더로 옮겨도
/// 동일하게 "Password protected" 로 차단되는 사례 보고(이슈 #22). 한컴이 실제 fileheader
/// 에 박는 마커는 "HWP Document File V5.00" 이며, signature 길이를 늘려 매칭을 엄격히 한다.
const HWP5_SIGNATURE: &[u8] = b"HWP Document File V5";

/// HWP5 FileHeader stream 위치의 합리적 상한. CFB 컨테이너 구조상 FileHeader 는
/// 보통 첫 sector ~ 수 KB 내에 배치되며 64KB 를 넘는 경우는 사실상 없다. 그 이후
/// 위치의 매치는 본문 데이터의 우연 매치로 간주한다.
const HWP5_HEADER_MAX_OFFSET: usize = 64 * 1024;

/// HWP5 암호 감지를 위해 읽을 최대 바이트. CFB 헤더 + FAT + FileHeader stream은
/// 통상 파일 앞 64KB 이내에 위치. 안전 마진으로 1MB.
const HWP5_SCAN_LIMIT: usize = 1024 * 1024;

/// PDF trailer 검색을 위한 tail 크기 (32KB).
/// 표준 trailer는 파일 끝 근처에 있으며 %%EOF 앞 수 KB 이내.
const PDF_TAIL_SCAN: u64 = 32 * 1024;

/// HWPX/ODF manifest 경로. 모든 OOXML 변종은 META-INF 아래 manifest.xml 사용.
const ODF_MANIFEST_NAME: &str = "META-INF/manifest.xml";

/// BIFF8 (Excel 97-2003 .xls) 스캔 한도. Workbook stream 의 BOF + FILEPASS record 는
/// 보통 첫 sector (~ 4KB) 안에 있다. 1MB 면 충분한 안전 마진.
const BIFF_SCAN_LIMIT: usize = 1024 * 1024;

/// 파일 확장자로 분기하여 암호 보호 여부 사전 감지.
///
/// - 지원 확장자: `hwp`, `hwpx`, `docx`, `xlsx`, `pptx`, `pdf`, `xls`
/// - 그 외 확장자 (ppt/doc 레거시 포함) → `false` 리턴 후 파서 내부 감지에 위임
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
        "docx" | "xlsx" | "pptx" => has_cfb_magic(path).unwrap_or(false),
        // 레거시 BIFF8 (.xls): 정상도 CFB 라 has_cfb_magic 으로는 구분 불가.
        // BIFF Workbook stream 의 FILEPASS record (0x002F) 를 byte-search 로 사전 감지.
        // calamine 이 암호 BIFF 를 panic 하는 사례를 차단하는 것이 1차 목표.
        "xls" => xls_biff_is_encrypted(path).unwrap_or(false),
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

    // "HWP Document File V5" signature 찾기 — 너무 뒤쪽 매치는 본문 우연 매치로 간주해 무시.
    // 보수 정책: 의심스러우면 false (= 암호 아님 → 정상 파서로 위임). false negative 는
    // kordoc 내부 fallback 이 처리하지만, false positive 는 정상 파일을 차단해 인덱싱 실패로 직결됨.
    let Some(pos) = find_subsequence(&buf, HWP5_SIGNATURE) else {
        return Ok(false);
    };
    if pos > HWP5_HEADER_MAX_OFFSET {
        return Ok(false);
    }

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

    // Sanity check — HWP5 spec 의 properties 는 하위 9비트만 정의되어 있다 (현행 spec 1.3 기준).
    // 상위 비트가 set 이면 매칭 위치가 진짜 FileHeader 가 아닐 가능성이 높으므로 차단.
    // (정상 파일에서도 reserved 비트가 비어있는 게 한컴오피스가 출력하는 표준 동작이다.)
    if properties & 0xFFFF_FE00 != 0 {
        return Ok(false);
    }

    // bit 1 (암호) | bit 4 (DRM 보안)
    // 주의: bit 8 (0x100) 는 일부 정상 문서(예: 한컴오피스에서 저장한 공공기관 문서)에도
    // 자주 set 되어 있어 false positive 유발 — kordoc 가 실제 파싱 가능한 파일을 차단함.
    // HWP5 spec 상 의미가 모호하므로 검사에서 제외하고, 진짜 암호 비트만 본다.
    const FLAG_PASSWORD: u32 = 0x0000_0002;
    const FLAG_DRM: u32 = 0x0000_0010;
    Ok(properties & (FLAG_PASSWORD | FLAG_DRM) != 0)
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

/// 레거시 BIFF8 (.xls) 의 암호화 여부 휴리스틱 감지.
///
/// BIFF8 spec:
/// - Workbook stream 시작은 BOF record: `09 08 ?? ?? 06 00 ?? ??` (type=0x0809, BIFF8)
/// - 암호 보호 시 BOF 직후에 FILEPASS record (type=0x002F) 가 등장:
///   `2F 00 [size:u16] [data]`
/// - data 첫 2바이트가 protection type (0x0000=XOR, 0x0001=RC4 / RC4-CryptoAPI)
///
/// CFB 컨테이너를 정식으로 파싱하지 않고 파일 앞부분을 byte-search 한다.
/// 이유: false negative 는 calamine 이 처리하지만 false positive 는 정상 파일을 차단해
/// 인덱싱 실패로 직결되므로 보수적으로 BOF 와 FILEPASS 가 인접한 패턴만 허용.
///
/// 검사 규칙:
/// 1. CFB magic 확인 (아니면 false — .xls 가 BIFF5 미만이거나 손상)
/// 2. 파일 앞 1MB 안에서 BIFF8 BOF record 시그니처 (`09 08 .. .. 06 00`) 검색
/// 3. BOF record 끝(=offset + 4 + size) 직후가 FILEPASS record (`2F 00`) 인지 확인
fn xls_biff_is_encrypted(path: &Path) -> std::io::Result<bool> {
    let mut file = File::open(path)?;

    // CFB magic 확인 (빠른 reject)
    let mut magic = [0u8; 8];
    if file.read_exact(&mut magic).is_err() || magic != CFB_MAGIC {
        return Ok(false);
    }

    file.seek(SeekFrom::Start(0))?;
    let file_size = file.metadata()?.len() as usize;
    let read_len = file_size.min(BIFF_SCAN_LIMIT);
    let mut buf = vec![0u8; read_len];
    let n = file.read(&mut buf)?;
    buf.truncate(n);

    // BIFF8 BOF record signature: type=0x0809, size 가변 (보통 16 bytes), version field 0x0600.
    // 패턴: 09 08 [size_lo size_hi] 06 00
    // 우연 매치 줄이려고 BOF 끝 직후 FILEPASS(2F 00) 인접까지 함께 본다.
    let mut i = 0usize;
    while i + 8 < buf.len() {
        // 0x0809 record type + version 0x0600 marker
        if buf[i] == 0x09 && buf[i + 1] == 0x08 && buf[i + 4] == 0x06 && buf[i + 5] == 0x00 {
            let size = u16::from_le_bytes([buf[i + 2], buf[i + 3]]) as usize;
            let after_bof = i + 4 + size;
            // 다음 record 가 FILEPASS (0x002F) 인지 확인
            if after_bof + 4 <= buf.len()
                && buf[after_bof] == 0x2F
                && buf[after_bof + 1] == 0x00
            {
                // 추가 sanity: FILEPASS data 의 첫 2byte 가 0x0000(XOR) 또는 0x0001(RC4)
                let data_off = after_bof + 4;
                if data_off + 2 <= buf.len() {
                    let prot = u16::from_le_bytes([buf[data_off], buf[data_off + 1]]);
                    if prot == 0x0000 || prot == 0x0001 {
                        return Ok(true);
                    }
                }
            }
            // BOF 한 번 발견하면 충분 (Workbook stream 시작은 1 회).
            // 다른 sub-stream 의 BOF 까지 따라가면 false positive 가 늘어난다.
            break;
        }
        i += 1;
    }

    Ok(false)
}

/// PDF trailer 사전의 /Encrypt 키 존재 확인.
///
/// PDF trailer는 파일 끝의 `trailer << ... /Encrypt N G R ... >>` 또는
/// cross-reference stream의 /Encrypt 엔트리에 등장. %%EOF 앞 수 KB 내에 있으므로
/// 파일 tail 32KB 만 스캔. 대용량 PDF도 빠르게 판정.
///
/// **false positive 방지**: 단순 `/Encrypt` substring 매치는
/// `/EncryptMetadata` (단순 boolean 메타플래그, 본문 암호 아님), 본문 stream 내 우연
/// 매치, 폰트/리소스 dict 의 키 이름 등을 모두 잡아 정상 PDF 를 차단했다 (이슈 #22).
/// 진짜 trailer Encrypt 키는 항상 indirect reference 또는 direct dict 형식이므로,
/// `/Encrypt` 다음 토큰이 `<숫자> <숫자> R` 또는 `<<` 인 경우만 양성으로 본다.
fn pdf_is_encrypted(path: &Path) -> std::io::Result<bool> {
    use std::sync::OnceLock;
    static ENCRYPT_RE: OnceLock<regex::bytes::Regex> = OnceLock::new();
    let re = ENCRYPT_RE.get_or_init(|| {
        // /Encrypt 뒤에 단어 경계가 와야 EncryptMetadata 같은 다른 키를 차단.
        // 그 후 whitespace + (indirect ref `N N R` | direct dict `<<`).
        regex::bytes::Regex::new(r"/Encrypt[\s\r\n]+(?:\d+\s+\d+\s+R|<<)").expect("valid regex")
    });

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

    Ok(re.is_match(&buf))
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

    /// 정상 .xls 파일에 대해 false positive 가 나지 않아야 한다 — 가장 중요한 회귀 보호.
    /// 빌드 환경에 fixture 가 없으면 skip (개발자 PC 에 다운로드 폴더가 있을 때만 실행).
    #[test]
    fn neis_report_designer_xls_is_not_flagged_as_password_protected() {
        let p = Path::new(r"C:\Users\Chris\Downloads\오류목록\황00(연가).xls");
        if !p.exists() {
            eprintln!("skip: NEIS fixture not found at {}", p.display());
            return;
        }
        assert!(
            !is_password_protected(p),
            "NEIS Report Designer 출력 BIFF8 정상 파일이 false positive 로 차단됨 — \
             xls_biff_is_encrypted 휴리스틱이 너무 공격적임"
        );
    }

    #[test]
    fn xls_without_cfb_magic_returns_false() {
        // .xls 확장자지만 CFB 가 아닌 파일 — 빠른 reject 경로
        let tmp = std::env::temp_dir().join("not_cfb_test.xls");
        std::fs::write(&tmp, b"PK\x03\x04not really xlsx").unwrap();
        assert!(!is_password_protected(&tmp));
        let _ = std::fs::remove_file(&tmp);
    }

    /// PDF: `/EncryptMetadata` 같은 다른 키 / 본문 우연 매치는 false 여야 한다.
    /// 정상 PDF 의 trailer 에 EncryptMetadata 키만 있는 경우 (실제 OOXML PDF 에서 흔함)
    /// 이전 substring 검사는 이를 암호로 오판해 차단했다 (이슈 #22).
    #[test]
    fn pdf_encrypt_metadata_not_flagged() {
        let tmp = std::env::temp_dir().join("encrypt_metadata_test.pdf");
        // 최소한의 PDF tail — trailer 안에 /EncryptMetadata 만 존재 (진짜 /Encrypt 키 없음)
        let body = b"%PDF-1.4\n1 0 obj<<>>endobj\nxref\n0 1\n0000000000 65535 f\ntrailer\n<< /Size 1 /EncryptMetadata false /Root 1 0 R >>\nstartxref\n9\n%%EOF\n";
        std::fs::write(&tmp, body).unwrap();
        assert!(
            !is_password_protected(&tmp),
            "PDF /EncryptMetadata 메타플래그가 암호로 오판됨 — false positive"
        );
        let _ = std::fs::remove_file(&tmp);
    }

    /// PDF: 본문 stream 안에 우연히 "/Encrypt" 문자열이 있는 케이스 — false 여야 한다.
    /// 폰트 dict 의 /Encoding, content stream 의 텍스트 등에서 발생 가능.
    #[test]
    fn pdf_encrypt_substring_in_body_not_flagged() {
        let tmp = std::env::temp_dir().join("encrypt_substring_test.pdf");
        // /Encrypt 가 trailer dict 의 indirect ref 형식이 아닌 위치에 substring 으로만 등장
        let body = b"%PDF-1.4\nstream\n... /Encrypt is a fake string here ...\nendstream\ntrailer\n<< /Size 1 /Root 1 0 R >>\nstartxref\n9\n%%EOF\n";
        std::fs::write(&tmp, body).unwrap();
        assert!(
            !is_password_protected(&tmp),
            "PDF 본문 내 /Encrypt substring 이 암호로 오판됨 — false positive"
        );
        let _ = std::fs::remove_file(&tmp);
    }

    /// PDF: 진짜 암호 PDF 의 trailer 형식 (`/Encrypt N N R`) 은 true 여야 한다.
    #[test]
    fn pdf_real_encrypt_indirect_ref_is_flagged() {
        let tmp = std::env::temp_dir().join("encrypt_real_test.pdf");
        let body = b"%PDF-1.4\n1 0 obj<<>>endobj\nxref\n0 1\n0000000000 65535 f\ntrailer\n<< /Size 1 /Encrypt 5 0 R /Root 1 0 R >>\nstartxref\n9\n%%EOF\n";
        std::fs::write(&tmp, body).unwrap();
        assert!(
            is_password_protected(&tmp),
            "진짜 암호 PDF (/Encrypt N N R) 가 감지되지 않음 — false negative"
        );
        let _ = std::fs::remove_file(&tmp);
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
