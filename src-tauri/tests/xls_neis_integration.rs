//! NEIS Report Designer 가 출력한 구형 BIFF8 .xls 인덱싱 회귀 테스트.
//!
//! 사용자 보고: 해당 .xls 가 폴더에 있으면 "인덱싱 도중 강제종료" 발생.
//! 격리 검증 결과 calamine + lindera 단독으로는 panic 미재현 — 그러나 후속 단계의
//! catch_unwind 누락 + 트랜잭션 dangling 가능성으로 방어 패치를 적용했고,
//! 이 테스트는 패치된 경로가 정상 파일에서 회귀 없이 통과하는지 확인한다.
//!
//! Fixture 는 사용자 PC 의 다운로드 폴더에만 있어 commit 하지 않고, 경로 부재 시 skip.

use std::path::Path;

const NEIS_FIXTURES: &[&str] = &[
    r"C:\Users\Chris\Downloads\오류목록\황00(연가).xls",
    r"C:\Users\Chris\Downloads\오류목록\황00(외출).xls",
    r"C:\Users\Chris\Downloads\오류목록\황00(조퇴).xls",
];

#[test]
fn neis_xls_parse_no_panic_no_password_false_positive() {
    let mut tested = 0;
    for fixture in NEIS_FIXTURES {
        let path = Path::new(fixture);
        if !path.exists() {
            eprintln!("skip: NEIS fixture absent: {}", path.display());
            continue;
        }
        tested += 1;

        // 1) password_detect 가 false positive 로 차단하지 않아야 한다 — 가장 중요.
        assert!(
            !docufinder_lib::parsers::password_detect::is_password_protected(path),
            "정상 NEIS 파일이 password protected 로 잘못 분류됨: {}",
            path.display()
        );

        // 2) parse_file 이 panic 없이 ParsedDocument 반환해야 한다.
        let result = docufinder_lib::parsers::parse_file(path, None);
        let doc = match result {
            Ok(d) => d,
            Err(e) => panic!("parse_file 실패 ({}): {}", path.display(), e),
        };

        // 3) 본문 추출 검증 — NEIS "근무상황부" 키워드 또는 시트 헤더가 포함되어야 함.
        assert!(
            !doc.content.is_empty(),
            "본문이 비어있음: {}",
            path.display()
        );
        assert!(
            doc.content.contains("근") && doc.content.contains("무"),
            "본문에 '근무' 누락: {}",
            path.display()
        );

        // 4) 청크 분할 정상.
        assert!(
            !doc.chunks.is_empty(),
            "청크 0 개: {}",
            path.display()
        );
        for chunk in &doc.chunks {
            assert!(
                chunk.end_offset >= chunk.start_offset,
                "잘못된 청크 offset 범위: {}",
                path.display()
            );
            assert!(
                chunk.location_hint.is_some(),
                "XLSX 청크에 location_hint 필수: {}",
                path.display()
            );
        }

        eprintln!(
            "[OK] {} — {} chars, {} chunks",
            path.display(),
            doc.content.len(),
            doc.chunks.len()
        );
    }

    if tested == 0 {
        eprintln!("WARNING: NEIS fixture 가 하나도 없어 회귀 검증 미수행");
    } else {
        eprintln!("[neis_xls_integration] {} 파일 회귀 검증 통과", tested);
    }
}

#[test]
fn breadcrumb_cleared_after_successful_parse() {
    // parse_file 정상 종료 시 RAII Guard 가 breadcrumb 를 clear 했는지 확인.
    // 파싱 후 다른 코드가 처리 중이 아니면 snapshot 은 None 이어야 함.
    let path = Path::new(NEIS_FIXTURES[0]);
    if !path.exists() {
        eprintln!("skip: fixture absent");
        return;
    }
    docufinder_lib::breadcrumb::clear();
    let _ = docufinder_lib::parsers::parse_file(path, None);
    assert!(
        docufinder_lib::breadcrumb::snapshot().is_none(),
        "정상 종료 후 breadcrumb 가 남아있음 (RAII Guard 누수)"
    );
}
