//! 텍스트 정규화 — 인덱싱과 검색 쿼리가 동일 기준으로 매칭되도록 통일한다.
//!
//! FTS5 `unicode61` 토크나이저는 대소문자 폴딩은 하지만 **유니코드 정규화(NFC)는
//! 하지 않는다.** 그래서 macOS 파일명·일부 PDF/OCR 이 내보내는 NFD 분해 한글
//! (예: U+1112 U+1161 U+11AB) 은 키보드/IME 가 만드는 NFC 한글(U+D55C '한') 과
//! 영영 매칭되지 않는다. zero-width·BOM·NBSP 같은 비가시 문자도 토큰 경계를
//! 어긋나게 해 검색이 조용히 누락된다.
//!
//! `normalize_text` 는 인덱싱 텍스트와 검색 쿼리 **양쪽**에 동일하게 적용되어야
//! 의미가 있다(한쪽만 하면 불일치). 4단계로 구성:
//!   1. 줄바꿈 통일 (`\r\n`, `\r` → `\n`)
//!   2. 비가시/포맷 문자 제거 (ZWSP·ZWNJ·ZWJ·BOM·soft-hyphen)
//!   3. 공백류 폴딩 (NBSP·NNBSP·ideographic space → ASCII space, **길이 보존**)
//!   4. NFC 정규화 (NFKC 가 아님 — ²→2, ㈜→(주) 같은 의미 손실을 피한다)
//!
//! 연속 공백 압축은 **하지 않는다** — '서울 시청' 과 '서울시청' 구분을 유지하기 위해.

use std::borrow::Cow;
use unicode_normalization::{is_nfc_quick, IsNormalized, UnicodeNormalization};

/// 검색·매칭에서 제거할 비가시/포맷 문자.
#[inline]
fn is_removable(c: char) -> bool {
    matches!(
        c,
        '\u{200B}'       // ZERO WIDTH SPACE
            | '\u{200C}' // ZERO WIDTH NON-JOINER
            | '\u{200D}' // ZERO WIDTH JOINER
            | '\u{FEFF}' // ZERO WIDTH NO-BREAK SPACE (BOM)
            | '\u{00AD}' // SOFT HYPHEN
    )
}

/// 공백류 문자를 일반 ASCII space 로 폴딩 (1:1, 길이 보존).
#[inline]
fn fold_space(c: char) -> Option<char> {
    match c {
        '\u{00A0}'       // NO-BREAK SPACE
        | '\u{202F}'     // NARROW NO-BREAK SPACE
        | '\u{3000}'     // IDEOGRAPHIC SPACE
            => Some(' '),
        _ => None,
    }
}

/// NFC 정규화 — 이미 NFC 면 빌림 그대로 반환(흔한 경우 거의 무비용).
fn nfc(s: &str) -> Cow<'_, str> {
    match is_nfc_quick(s.chars()) {
        IsNormalized::Yes => Cow::Borrowed(s),
        _ => Cow::Owned(s.nfc().collect()),
    }
}

/// 인덱싱/검색 텍스트를 매칭 가능한 정규형으로 변환한다.
///
/// ⚠ 인덱싱 경로와 검색 쿼리 경로에 **반드시 동일하게** 적용해야 한다.
pub fn normalize_text(input: &str) -> String {
    // 1차 패스: 줄바꿈 통일 + 비가시 제거 + 공백 폴딩
    let mut buf = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                // `\r\n` 과 단독 `\r` 모두 `\n` 으로
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                buf.push('\n');
            }
            c if is_removable(c) => {} // skip
            c => buf.push(fold_space(c).unwrap_or(c)),
        }
    }

    // 2차 패스: NFC. 이미 NFC 면 1차 결과를 그대로 반환.
    match nfc(&buf) {
        Cow::Borrowed(_) => buf,
        Cow::Owned(s) => s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nfc_decomposed_hangul() {
        // NFD '한' = U+1112 U+1161 U+11AB → NFC U+D55C
        let nfd = "\u{1112}\u{1161}\u{11AB}";
        assert_eq!(normalize_text(nfd), "한");
        assert_eq!(normalize_text(nfd), "\u{D55C}");
    }

    #[test]
    fn nfc_idempotent() {
        assert_eq!(normalize_text("계약서"), "계약서");
    }

    #[test]
    fn remove_zwsp() {
        assert_eq!(normalize_text("계\u{200B}약"), "계약");
    }

    #[test]
    fn remove_bom() {
        assert_eq!(normalize_text("\u{FEFF}본문"), "본문");
    }

    #[test]
    fn remove_soft_hyphen() {
        assert_eq!(normalize_text("계약\u{00AD}산서"), "계약산서");
    }

    #[test]
    fn remove_zwj() {
        assert_eq!(normalize_text("가\u{200D}나"), "가나");
    }

    #[test]
    fn fold_nbsp_length_preserved() {
        assert_eq!(normalize_text("서울\u{00A0}시청"), "서울 시청");
    }

    #[test]
    fn fold_ideographic_space() {
        assert_eq!(normalize_text("제\u{3000}1조"), "제 1조");
    }

    #[test]
    fn unify_crlf_and_cr() {
        assert_eq!(
            normalize_text("line1\r\nline2\rline3"),
            "line1\nline2\nline3"
        );
    }

    #[test]
    fn lone_cr() {
        assert_eq!(normalize_text("a\rb"), "a\nb");
    }

    #[test]
    fn preserve_double_space() {
        // 연속 공백 압축 안 함 — '서울 시청' vs '서울시청' 구분 유지
        assert_eq!(normalize_text("a  b"), "a  b");
    }

    #[test]
    fn preserve_markdown_markers() {
        // 마크다운 구조는 건드리지 않음 (스코프 밖)
        assert_eq!(normalize_text("## 계약 | 항목"), "## 계약 | 항목");
    }

    #[test]
    fn nfkc_not_applied() {
        // NFC 는 호환 문자를 보존한다(NFKC 가 아님): ²↛2, ㈜↛(주)
        assert_eq!(normalize_text("²"), "²");
        assert_eq!(normalize_text("㈜"), "㈜");
    }

    #[test]
    fn mixed() {
        let input = "\u{FEFF}서울\u{00A0}시\u{200B}청\r\n끝";
        assert_eq!(normalize_text(input), "서울 시청\n끝");
    }

    #[test]
    fn empty() {
        assert_eq!(normalize_text(""), "");
    }

    #[test]
    fn ascii_passthrough() {
        assert_eq!(normalize_text("hello world 2024"), "hello world 2024");
    }

    #[test]
    fn mid_stream_bom() {
        assert_eq!(normalize_text("본문\u{FEFF}본문"), "본문본문");
    }

    #[test]
    fn consecutive_different_spaces() {
        // 서로 다른 공백류가 각각 ASCII space 로 (개수 보존, 압축 안 함)
        assert_eq!(normalize_text("\u{3000}\u{00A0}"), "  ");
    }

    #[test]
    fn combined_diacritics_nfc() {
        // e + combining acute(U+0301) → é(U+00E9)
        assert_eq!(normalize_text("e\u{0301}"), "\u{00E9}");
    }
}
