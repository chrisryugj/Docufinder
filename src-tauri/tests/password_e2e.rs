//! 암호 문서 열기 end-to-end — kordoc 사이드카 경유 (#59)
//!
//! 픽스처는 kordoc 저장소의 tests/fixtures/password/ (rhwp MIT samples 유래, 암호 123456).
//! kordoc CLI 번들이 없거나 픽스처를 못 찾으면 건너뛴다.

use std::path::PathBuf;

fn fixture(name: &str) -> Option<PathBuf> {
    for base in [
        std::env::var("KORDOC_DIR").unwrap_or_default(),
        format!(
            "{}/workspace/kordoc",
            std::env::var("HOME").unwrap_or_default()
        ),
    ] {
        if base.is_empty() {
            continue;
        }
        let p = PathBuf::from(base)
            .join("tests/fixtures/password")
            .join(name);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

#[test]
fn 암호_문서는_비밀번호_없이_열리지_않는다() {
    let Some(p) = fixture("HWP3-password-123456.hwp") else {
        eprintln!("픽스처 없음 — skip");
        return;
    };
    if !docufinder_lib::parsers::kordoc::is_available() {
        eprintln!("kordoc 번들 없음 — skip");
        return;
    }
    let err = docufinder_lib::parsers::kordoc::get_markdown(&p)
        .expect_err("비밀번호 없이 열리면 안 된다");
    assert!(
        matches!(
            err,
            docufinder_lib::parsers::ParseError::PasswordProtected(_)
        ),
        "암호 문서는 PasswordProtected 로 분류돼야 한다: {err:?}"
    );
}

#[test]
fn 올바른_비밀번호로_본문이_나온다() {
    let Some(p) = fixture("HWP3-password-123456.hwp") else {
        eprintln!("픽스처 없음 — skip");
        return;
    };
    if !docufinder_lib::parsers::kordoc::is_available() {
        eprintln!("kordoc 번들 없음 — skip");
        return;
    }
    let md = docufinder_lib::parsers::kordoc::get_markdown_with_password(&p, "123456")
        .expect("올바른 암호로 열려야 한다");
    assert!(md.len() > 100, "본문이 너무 짧다: {}자", md.len());
}

#[test]
fn 틀린_비밀번호는_성공으로_위장하지_않는다() {
    let Some(p) = fixture("HWP5-password-123456.hwpx") else {
        eprintln!("픽스처 없음 — skip");
        return;
    };
    if !docufinder_lib::parsers::kordoc::is_available() {
        eprintln!("kordoc 번들 없음 — skip");
        return;
    }
    let err = docufinder_lib::parsers::kordoc::get_markdown_with_password(&p, "wrong-password")
        .expect_err("틀린 암호로 열리면 안 된다");
    assert!(
        matches!(
            err,
            docufinder_lib::parsers::ParseError::PasswordProtected(_)
        ),
        "틀린 암호도 PasswordProtected 로: {err:?}"
    );
}
