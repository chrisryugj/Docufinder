//! PDF 사전 감지 (이미지 기반 PDF 휴리스틱)
//!
//! 목적: 스캔본·이미지 PDF에 대해 kordoc(Node.js 사이드카) 호출을 **사전 차단**한다.
//! v2.5.6 의 "조기 스킵" 패치는 같은 파일 *재시도* 만 막았기에, 새 PDF 마다 매번 spawn
//! 되었고, 폴더에 이미지 PDF가 수백 개면 자식 프로세스/파이프/스레드 누적으로
//! Windows 0xc0000409 (`__fastfail` / GS-cookie) 크래시까지 이어졌다 (#17).
//!
//! 휴리스틱: 파일 첫 N KB 를 읽어 텍스트 오브젝트(`BT` / `/Font`) 부재 + 이미지 자원
//! (`/XObject` + `/Subtype /Image` 또는 `/Filter /DCTDecode|/JPXDecode|/CCITTFaxDecode`)
//! 존재 시 "이미지 기반 PDF"로 판정.
//!
//! False positive (텍스트 PDF인데 이미지로 오판) 시 사용자는 키워드 검색만 못 하고
//! 메타데이터 인덱싱은 정상이다 — 크래시 회피 가치가 더 크다.
//! False negative (이미지 PDF인데 sniff 통과) 시 기존 kordoc 경로가 그대로 동작.

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

/// PDF 헤더 + 객체 스트림 일부를 보기에 충분한 길이.
const PDF_SNIFF_BYTES: usize = 64 * 1024;

/// 이미지 PDF 휴리스틱 판정.
/// IO 에러 / 비-PDF / 판정 불가 시 `false` (기존 경로 fallback).
pub fn is_likely_scanned_pdf(path: &Path) -> bool {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let mut buf = vec![0u8; PDF_SNIFF_BYTES];
    let n = match BufReader::new(file).read(&mut buf) {
        Ok(n) => n,
        Err(_) => return false,
    };
    if n < 8 || &buf[..5] != b"%PDF-" {
        return false;
    }
    let head = &buf[..n];

    let has_image_xobj = window_contains(head, b"/Subtype/Image")
        || window_contains(head, b"/Subtype /Image")
        || window_contains(head, b"/DCTDecode")
        || window_contains(head, b"/JPXDecode")
        || window_contains(head, b"/CCITTFaxDecode")
        || window_contains(head, b"/JBIG2Decode");

    if !has_image_xobj {
        return false;
    }

    // 본문 텍스트의 강한 마커들. 하나라도 발견되면 텍스트 PDF로 간주 (보수적 판정).
    let text_markers: [&[u8]; 4] = [b"/Font", b" Tj", b" TJ", b"BT\n"];
    let text_hits = text_markers
        .iter()
        .filter(|m| window_contains(head, m))
        .count();

    text_hits == 0
}

/// 단순 byte-window 검색 (memmem 의존성 회피).
fn window_contains(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || haystack.len() < needle.len() {
        return false;
    }
    haystack.windows(needle.len()).any(|w| w == needle)
}
