//! 파일명 정규화 — Document Lineage Graph의 1차 그루핑 키 생성.
//!
//! 같은 논리 문서의 여러 버전(`계약서_최종`, `계약서_최최종` 등)을 같은 stem으로
//! 접기 위해 한국 사무환경에서 관찰되는 버전/복사 suffix를 제거한다.
//!
//! **주의**: 정규식만으로 100% 판정하지 않는다. 이 함수의 결과가 같아도
//! 같은 lineage로 묶으려면 벡터 유사도 검증이 추가로 필요하다 (lineage.rs).

use once_cell::sync::Lazy;
use regex::Regex;

/// 맨앞에서 제거할 prefix 패턴들. 순서대로 반복 적용된다.
static PREFIX_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        // 별표
        Regex::new(r"^★+\s*").unwrap(),
        // 붙임N., 붙임 7.
        Regex::new(r"^붙임\s*\d+\s*[.)]\s*").unwrap(),
        // 정렬 번호: "00.", "21.", "21)" (점·닫는괄호만 허용 — "1-web", "10-foo" 오인식 방지)
        Regex::new(r"^\d{1,2}\s*[.)]\s*").unwrap(),
        // 대괄호 분류: [공고문], [붙임1], [홍보팀 수정]
        Regex::new(r"^\[[^\]]+\]\s*").unwrap(),
        // 버전/코드 성격의 소괄호 prefix만 (본문형 소괄호는 보존)
        Regex::new(
            r"^\((?:\d{3,4}|최최*종|진짜최종|간사_최종|최종|수정본?|확정|초안|완료|붙임\s*\d+|참고_최종)\)\s*",
        )
        .unwrap(),
        // 복사본 prefix (한국어 Windows): "복사본 ", "복사본 복사본 "
        Regex::new(r"^(?:복사본\s+)+").unwrap(),
        // Copy of XXX (영어)
        Regex::new(r"(?i)^Copy\s+of\s+").unwrap(),
        // 참고_
        Regex::new(r"^참고_").unwrap(),
    ]
});

/// 맨뒤에서 제거할 suffix 패턴들. 순서대로 반복 적용된다.
static SUFFIX_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        // 버전 마커 (괄호 버전). 최장 매치 우선을 위해 복합 키워드를 앞에 배치.
        Regex::new(
            r"[_\s\-]*[(（]\s*(?:최최+종|진짜최종|최종\s*확인용|최종\s*수정|최종완료|최종|수정본|수정중|수정|확정|확인용|변경완료|검토완료|검토|초안|임시저장|임시|원본|사본|완료|편집중)\s*[)）]\s*$",
        )
        .unwrap(),
        // 버전 마커 (공백/언더/하이픈 버전)
        Regex::new(
            r"[_\s\-]+(?:최최+종|진짜최종|최종완료|최종\s*확인용|최종\s*수정|최종|수정본|수정중|수정|확정|확인용|변경완료|검토완료|검토|초안|임시|원본|사본|완료|편집중)\s*$",
        )
        .unwrap(),
        // 복사본 suffix ("- 복사본", " 복사본 복사본")
        Regex::new(r"\s*[-_\s]*복사본(?:\s+복사본)*\s*$").unwrap(),
        // Windows 자동 번호 (1)~(999). 4자리 이상은 날짜(0626, 250627)일 가능성이 커 제외.
        Regex::new(r"\s*\(\d{1,3}\)\s*$").unwrap(),
        // 버전 번호: v1, v1.2, v1.4.x, ver25
        Regex::new(r"[_\s\-]+v(?:er\.?)?\s*[._\-]?\d+(?:\.\d+)*(?:\.[a-z0-9]+)?\s*$").unwrap(),
        // Rev1, Rev 2
        Regex::new(r"[_\s\-]+Rev\s*\d+\s*$").unwrap(),
        // 날짜 suffix: _20230729, _20201116_1748 — 8자리(YYYYMMDD)를 안전하게 제거.
        // 6자리 단독 suffix는 try_strip_6digit_date()에서 YYMMDD 유효성 검사 후 제거.
        Regex::new(r"[_\s\-]+20\d{6}(?:_\d{4})?\s*$").unwrap(),
        // 날짜+수정: (240418수정), _221216수정
        Regex::new(r"[_\s]*[(（]?\s*\d{6,8}\s*수정\s*[)）]?\s*$").unwrap(),
        // 영어 버전 키워드
        Regex::new(r"(?i)[_\s\-]+(?:final|draft|original)\s*$").unwrap(),
    ]
});

/// 공백/언더/+ 를 단일 공백으로 정리하는 패턴.
static WHITESPACE_NORMALIZE: Lazy<Regex> = Lazy::new(|| Regex::new(r"[\s_]+").unwrap());

/// 전체 stem이 `YYYYMMDD[_\s]HHMMSS` 패턴이면 (사진·스크린샷) 6자리 제거를 건너뛴다.
/// HHMMSS 중간 2자리(예: `121128` → 11)가 우연히 유효 MM/DD가 되는 함정을 차단한다.
static PHOTO_TIMESTAMP: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\d{8}[_\s\-]\d{6}\b").unwrap());

/// 6자리 날짜 suffix 탐지용: `XXX_YYMMDD` 또는 `XXX YYMMDD`, `XXX-YYMMDD` 끝.
static SIX_DIGIT_SUFFIX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(.+?)[_\s\-]\s*(\d{6})\s*$").unwrap());

/// 6자리 숫자 suffix를 YYMMDD 유효성 기준으로 제거한다.
///
/// 유효하면(MM ∈ 1..=12, DD ∈ 1..=31) `XXX` 부분만 반환, 아니면 None.
/// 앞부분이 YYYYMMDD로 시작하면 HHMMSS로 간주하여 None.
fn try_strip_6digit_date(s: &str) -> Option<String> {
    if PHOTO_TIMESTAMP.is_match(s) {
        return None;
    }
    let caps = SIX_DIGIT_SUFFIX.captures(s)?;
    let stem = caps.get(1)?.as_str();
    let digits = caps.get(2)?.as_str();
    let mm: u32 = digits.get(2..4)?.parse().ok()?;
    let dd: u32 = digits.get(4..6)?.parse().ok()?;
    if (1..=12).contains(&mm) && (1..=31).contains(&dd) {
        Some(stem.to_string())
    } else {
        None
    }
}

/// 파일명(확장자 포함 가능)에서 버전/복사본 힌트를 제거하고 정규화된 stem을 반환한다.
///
/// # 반환
/// - 정상 케이스: 소문자화 + 공백 통일된 stem
/// - 결과가 빈 문자열이 되면 원본(확장자 제외)을 lowercase로 반환 (overeager 방어)
pub fn normalize_stem(filename: &str) -> String {
    // 1. 확장자 제거 (단, 숨김파일 '.foo' 같은 건 건드리지 않음)
    let mut s = match filename.rfind('.') {
        Some(pos) if pos > 0 => filename[..pos].to_string(),
        _ => filename.to_string(),
    };

    let original_stem = s.clone();

    // 2. Prefix 반복 적용 (최대 5회 — 중첩 케이스 대비)
    for _ in 0..5 {
        let before = s.clone();
        for re in PREFIX_PATTERNS.iter() {
            s = re.replace(&s, "").to_string();
        }
        if s == before {
            break;
        }
    }

    // 3. Suffix 반복 적용 (최대 5회) — 정규식 + 6자리 날짜 유효성 검사
    for _ in 0..5 {
        let before = s.clone();
        for re in SUFFIX_PATTERNS.iter() {
            s = re.replace(&s, "").to_string();
        }
        if let Some(trimmed) = try_strip_6digit_date(&s) {
            s = trimmed;
        }
        if s == before {
            break;
        }
    }

    // 4. '+' → 공백 (URL 인코딩 잔재), 공백 정규화, 양끝 trim
    s = s.replace('+', " ");
    s = WHITESPACE_NORMALIZE.replace_all(&s, " ").to_string();
    s = s.trim().to_lowercase();

    // 5. Overeager 방어: 전부 제거되어 빈 stem이 되면 원본으로 복귀
    if s.is_empty() {
        return original_stem.trim().to_lowercase();
    }
    s
}

/// 버전 라벨 추출 — UI 뱃지에 "최최종", "v3" 등으로 표시하기 위함.
///
/// 발견되지 않으면 None.
pub fn extract_version_label(filename: &str) -> Option<String> {
    // 확장자 제거
    let stem = match filename.rfind('.') {
        Some(pos) if pos > 0 => &filename[..pos],
        _ => filename,
    };

    // "최최종", "최최최종" 등 (가장 구체적인 것 우선)
    static LABEL_FINAL_STACK: Lazy<Regex> = Lazy::new(|| Regex::new(r"(최최+종)").unwrap());
    if let Some(m) = LABEL_FINAL_STACK.captures(stem) {
        return Some(m[1].to_string());
    }

    // "진짜최종"
    if stem.contains("진짜최종") {
        return Some("진짜최종".to_string());
    }

    // v숫자 — `\b`는 `_`와 `v` 사이에서 매치 안 되므로 직접 앞경계 명시.
    static LABEL_V: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"(?i)(?:^|[\s_\-])v(?:er\.?)?\s*[._\-]?(\d+(?:\.\d+)*)").unwrap());
    if let Some(m) = LABEL_V.captures(stem) {
        return Some(format!("v{}", &m[1]));
    }

    // Rev숫자
    static LABEL_REV: Lazy<Regex> = Lazy::new(|| Regex::new(r"Rev\s*(\d+)").unwrap());
    if let Some(m) = LABEL_REV.captures(stem) {
        return Some(format!("Rev{}", &m[1]));
    }

    // "최종"
    if stem.contains("최종") {
        return Some("최종".to_string());
    }

    // 수정/초안 등 단순 라벨
    for kw in &["수정본", "수정중", "수정", "초안", "검토", "확정", "임시"] {
        if stem.contains(kw) {
            return Some(kw.to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_content_different_version_markers_match() {
        // 동일 문서의 서로 다른 버전 표기가 같은 stem으로 수렴해야 함
        let a = normalize_stem("★2025년 7월 고용산재보험료 부과내역(최종) - 복사본.xlsx");
        let b = normalize_stem("★2025년 7월 고용산재보험료 부과내역(최종).xlsx");
        assert_eq!(a, b);
        assert_eq!(a, "2025년 7월 고용산재보험료 부과내역");
    }

    #[test]
    fn spacing_variants_match() {
        // 공백 있고 없고 차이가 동일하게 수렴
        let a = normalize_stem("2024년 10월 고용산재 산출내역(최종확인용).xlsx");
        let b = normalize_stem("2024년 10월 고용산재 산출내역(최종 확인용).xlsx");
        let c = normalize_stem("2024년 10월 고용산재 산출내역_최종확인용.xlsx");
        assert_eq!(a, b);
        assert_eq!(b, c);
    }

    #[test]
    fn attached_prefix_removed() {
        // "붙임1." 제거, 날짜+수정 제거, 부서 suffix는 보존되나 본문은 유지
        let s = normalize_stem("붙임1. 2023년 사업 결과 및 정산보고(서식)_홍보팀(240418수정).hwpx");
        assert!(s.contains("2023년 사업 결과"));
        assert!(!s.contains("240418"));
        assert!(!s.contains("수정"));
    }

    #[test]
    fn copy_prefix_stripped() {
        let s = normalize_stem("복사본 복사본 결혼비용예산및지출_꼼깡이_1211.xlsx");
        assert!(s.starts_with("결혼비용예산및지출"));
        assert!(!s.contains("복사본"));
    }

    #[test]
    fn date_prefix_preserved() {
        // 6자리 날짜 prefix는 회차 식별자이므로 보존되어야 함
        let a = normalize_stem("180109 이태종T하프(수정).hwp");
        let b = normalize_stem("180115 이태종T하프 (수정).hwp");
        // 둘 다 날짜가 stem 안에 있음
        assert!(a.contains("180109"));
        assert!(b.contains("180115"));
        // 서로 다른 회차는 다른 stem
        assert_ne!(a, b);
    }

    #[test]
    fn v_number_stripped() {
        let a = normalize_stem("HIS Overview Presentation v5.5.pptx");
        let b = normalize_stem("HIS Overview Presentation v6.pptx");
        assert_eq!(a, b);
        assert_eq!(a, "his overview presentation");
    }

    #[test]
    fn windows_copy_number_stripped() {
        let a = normalize_stem("일시불 현금서비스 내역 (1).xls");
        let b = normalize_stem("일시불 현금서비스 내역 (2).xls");
        let c = normalize_stem("일시불 현금서비스 내역.xls");
        assert_eq!(a, b);
        assert_eq!(b, c);
    }

    #[test]
    fn final_ko_variants() {
        let a = normalize_stem("계약서_최종.hwpx");
        let b = normalize_stem("계약서_최최종.hwpx");
        let c = normalize_stem("계약서_최최최종.hwpx");
        let d = normalize_stem("계약서_진짜최종.hwpx");
        assert_eq!(a, b);
        assert_eq!(b, c);
        assert_eq!(c, d);
        assert_eq!(a, "계약서");
    }

    #[test]
    fn plus_converted_to_space() {
        let s = normalize_stem(
            "[공고문]2019년도+제1회+경상북도+지방공무원+공개경쟁임용시험+최종합격자+공고.pdf",
        );
        assert!(!s.contains('+'));
        assert!(s.contains("2019년도 제1회"));
    }

    #[test]
    fn empty_stem_falls_back() {
        // 모든 내용이 prefix 매칭되는 극단적 케이스 방어
        let s = normalize_stem("★★★.hwp");
        // 빈 문자열이 되지 않아야 함
        assert!(!s.is_empty());
    }

    #[test]
    fn extension_preserved_stem() {
        // 확장자만 제거, 동일 stem
        assert_eq!(normalize_stem("계약서.hwp"), "계약서");
        assert_eq!(normalize_stem("계약서.docx"), "계약서");
    }

    #[test]
    fn version_label_extraction() {
        assert_eq!(
            extract_version_label("계약서_최최종.hwpx"),
            Some("최최종".to_string())
        );
        assert_eq!(
            extract_version_label("계약서_최최최종.hwpx"),
            Some("최최최종".to_string())
        );
        assert_eq!(
            extract_version_label("계약서_최종.hwpx"),
            Some("최종".to_string())
        );
        assert_eq!(
            extract_version_label("HIS v5.5.pptx"),
            Some("v5.5".to_string())
        );
        // underscore prefix도 인식 (이전엔 \b 때문에 실패)
        assert_eq!(
            extract_version_label("서면자문_의견서_v8.docx"),
            Some("v8".to_string())
        );
        assert_eq!(
            extract_version_label("file_Rev3.pptx"),
            Some("Rev3".to_string())
        );
        assert_eq!(extract_version_label("just_a_file.txt"), None);
    }

    // === 실환경 파일명 7388개 regression 스위트 ===
    // fixtures/real_filenames.list: 사용자 E:\ 드라이브에서 실제 수집된 파일명.
    // .list 확장자 — Docufinder 파서(hwpx/docx/xlsx/pdf/txt) 대상 아님 → 사용자 DB 인덱싱 오염 방지.
    // 정규화 규칙을 수정한 뒤 이 스위트를 돌려 false positive/negative를 모니터링한다.

    const REAL_FIXTURE: &str = include_str!("../../tests/fixtures/real_filenames.list");

    fn load_real_filenames() -> Vec<String> {
        REAL_FIXTURE
            .lines()
            .map(|l| l.trim_start_matches('\u{feff}').trim())
            .filter(|l| !l.is_empty())
            .map(String::from)
            .collect()
    }

    #[test]
    fn real_all_normalize_without_empty() {
        let names = load_real_filenames();
        assert!(
            names.len() > 5000,
            "fixture 파일명이 너무 적다: {}",
            names.len()
        );
        for n in &names {
            let stem = normalize_stem(n);
            assert!(!stem.is_empty(), "빈 stem 발생: {}", n);
        }
    }

    #[test]
    fn real_no_overaggressive_collapse() {
        // 정규화로 2자 이하로 압축되는 케이스가 전체의 5%를 넘으면 공격적 의심.
        let names = load_real_filenames();
        let too_short: Vec<_> = names
            .iter()
            .filter_map(|n| {
                let s = normalize_stem(n);
                (s.chars().count() <= 2).then(|| (n.clone(), s))
            })
            .collect();
        let ratio = too_short.len() as f64 / names.len() as f64;
        assert!(
            ratio < 0.05,
            "stem이 2자 이하인 비율이 {:.1}% (허용 <5%)\n샘플:\n{}",
            ratio * 100.0,
            too_short
                .iter()
                .take(10)
                .map(|(n, s)| format!("  '{}' → '{}'", n, s))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    #[test]
    fn real_pairs_should_collapse() {
        let pairs = [
            (
                "★2025년 7월 고용산재보험료 부과내역(최종) - 복사본.xlsx",
                "★2025년 7월 고용산재보험료 부과내역(최종).xlsx",
            ),
            (
                "2024년 10월 고용산재 산출내역(최종확인용).xlsx",
                "2024년 10월 고용산재 산출내역(최종 확인용).xlsx",
            ),
            ("계약서_최종.hwpx", "계약서_최최종.hwpx"),
            (
                "일시불 현금서비스 내역 (1).xls",
                "일시불 현금서비스 내역.xls",
            ),
            // 6자리 YYMMDD 유효 날짜 suffix 제거 (사용자 요청)
            ("계약서_240418.hwpx", "계약서.hwpx"),
            ("report_221216.hwp", "report.hwp"),
            ("보고서_250101.pdf", "보고서.pdf"),
        ];
        for (a, b) in pairs {
            let sa = normalize_stem(a);
            let sb = normalize_stem(b);
            assert_eq!(
                sa, sb,
                "같은 문서여야 하는데 분리됨:\n  '{}' → '{}'\n  '{}' → '{}'",
                a, sa, b, sb
            );
        }
    }

    #[test]
    fn real_pairs_should_not_collapse() {
        let pairs = [
            // 다른 회차의 모의고사
            ("180109 이태종T하프(수정).hwp", "180115 이태종T하프.hwp"),
            // 다른 월의 보고서
            (
                "2024년 10월 고용산재 산출내역(최종확인용).xlsx",
                "2024년 11월 고용산재 산출내역(최종 확인용).xlsx",
            ),
            // 하이픈 정렬번호를 숫자 prefix로 오인식 방지 (1-web, 10-web 등)
            ("1-web.pdf", "10-web.pdf"),
            ("1-web.pdf", "web.pdf"),
            // 괄호 안 날짜(MMDD, YYMMDD)는 Windows 복사번호가 아님
            ("카드영수증(0626).pdf", "카드영수증(0705).pdf"),
            ("지급내역서(250627).hwpx", "지급내역서(250730).hwpx"),
            // 사진 파일: YYYYMMDD_HHMMSS — HHMMSS를 날짜 suffix로 오인식 금지
            ("20151115_103255.jpg", "20151115_103306.jpg"),
            ("20171104_210722.jpg", "20171104_210717.jpg"),
            // HHMMSS가 우연히 유효 MM/DD처럼 보여도 photo 패턴 방어로 제거 금지
            ("20260101_121128.jpg", "20260101_131529.jpg"),
            // 6자리 suffix지만 YYMMDD 유효 범위 아님 → 제거 안 됨 (다른 파일로 유지)
            ("log_243000.txt", "log_253000.txt"),
            ("pic_103255.jpg", "pic_103306.jpg"),
        ];
        for (a, b) in pairs {
            let sa = normalize_stem(a);
            let sb = normalize_stem(b);
            assert_ne!(
                sa, sb,
                "다른 문서여야 하는데 합쳐짐:\n  '{}' → '{}'\n  '{}' → '{}'",
                a, sa, b, sb
            );
        }
    }

    #[test]
    fn real_print_grouping_stats() {
        // 정보성 리포트 — 실패하지 않음. `cargo test -- --nocapture`로 출력 확인.
        use std::collections::HashMap;
        let names = load_real_filenames();
        let mut groups: HashMap<String, Vec<String>> = HashMap::new();
        for n in &names {
            groups.entry(normalize_stem(n)).or_default().push(n.clone());
        }
        let total = names.len();
        let unique = groups.len();
        let multi = groups.values().filter(|v| v.len() >= 2).count();
        let largest = groups.values().map(|v| v.len()).max().unwrap_or(0);
        let collapse = 1.0 - (unique as f64 / total as f64);

        eprintln!("\n=== Lineage 그루핑 통계 ===");
        eprintln!("총 파일: {}", total);
        eprintln!(
            "고유 stem: {} (collapse ratio: {:.1}%)",
            unique,
            collapse * 100.0
        );
        eprintln!("2개 이상 묶인 그룹: {}", multi);
        eprintln!("가장 큰 그룹 크기: {}", largest);

        let mut sorted: Vec<_> = groups.iter().collect();
        sorted.sort_by_key(|(_, v)| std::cmp::Reverse(v.len()));
        eprintln!("\n--- 상위 10개 그룹 (수동 검증) ---");
        for (stem, members) in sorted.iter().take(10) {
            eprintln!("\n[stem: '{}'] ({}개)", stem, members.len());
            for m in members.iter().take(4) {
                eprintln!("  · {}", m);
            }
            if members.len() > 4 {
                eprintln!("  · ... ({} more)", members.len() - 4);
            }
        }
    }
}
