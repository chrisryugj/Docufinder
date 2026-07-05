//! rhwp 네이티브 렌더 — HWP5(.hwp) 바이너리를 원본 조판 SVG 로 렌더 (레이아웃 미리보기 전용).
//!
//! kordoc 이 HWPX 를 `render_svg` 로 렌더하듯, .hwp 는 rhwp 크레이트(DocumentCore→SVG,
//! 순수 Rust `tiny-skia` 경로 — native-skia/C++ 미사용)로 페이지별 SVG 를 만들어 하나의
//! `data-page` 세로 스택 SVG 로 합쳐 반환한다. 프론트 LayoutView 가 kordoc HWPX SVG 와
//! 동일하게 소비한다(같은 뷰어 재사용).
//!
//! 인덱싱/마크다운은 여전히 kordoc(`parsers::kordoc`) 담당 — 이 모듈은 미리보기 SVG 전용이라
//! `parse_file` 디스패치를 건드리지 않는다.

use super::ParseError;
use std::panic::AssertUnwindSafe;
use std::path::Path;

/// 레이아웃 SVG 응답 상한 — 다페이지·임베드 이미지로 커질 수 있어 IPC/렌더 부담 방어.
/// kordoc render(RENDER_MAX_SVG_SIZE)와 동일한 50MB 정책.
const RENDER_MAX_SVG_SIZE: usize = 50 * 1024 * 1024;

/// 페이지 간 세로 여백(px) — 회색 컨테이너 배경이 비쳐 페이지 경계로 보인다.
const PAGE_GAP: f64 = 12.0;

/// .hwp(HWP5 바이너리)를 원본 조판 SVG 로 렌더.
///
/// 페이지별 `render_page_svg_native` 결과를 ID 네임스페이스(clip-path 충돌 방지) 후
/// `data-page` 그룹으로 세로 스택해 단일 SVG 로 합친다. 파서 패닉은 catch_unwind 로
/// 봉쇄한다(손상 .hwp 가 앱을 죽이지 않도록 — Docufinder 는 `panic="unwind"` 라 유효).
pub fn render_svg(path: &Path) -> Result<String, ParseError> {
    // 파일 크기 상한 (다른 파서와 동일 안전망)
    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    if size > super::MAX_FILE_SIZE {
        return Err(ParseError::ParseError(format!(
            "파일 크기 초과: {} bytes (최대 {} bytes)",
            size,
            super::MAX_FILE_SIZE
        )));
    }

    let data = std::fs::read(path)?;
    let parent = path.parent().map(|p| p.to_path_buf());

    // rhwp 파싱/렌더는 in-process — 손상 파일의 패닉을 catch_unwind 로 잡아 앱 크래시를 막는다.
    let pages: Vec<String> = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let mut doc = ::rhwp::DocumentCore::from_bytes(&data)
            .map_err(|e| ParseError::ParseError(format!("HWP 파싱 실패: {e}")))?;
        // 외부 파일 참조 이미지(같은 폴더 basename 매칭) 로드 — rhwp CLI 와 동일 동작.
        if let Some(dir) = &parent {
            let _ = doc.populate_external_images_from_dir(dir);
        }
        let n = doc.page_count();
        if n == 0 {
            return Err(ParseError::ParseError(
                "HWP: 렌더할 페이지가 없습니다".to_string(),
            ));
        }
        let mut svgs = Vec::with_capacity(n as usize);
        for p in 0..n {
            let svg = doc
                .render_page_svg_native(p)
                .map_err(|e| ParseError::ParseError(format!("HWP 페이지 {p} 렌더 실패: {e}")))?;
            svgs.push(svg);
        }
        Ok::<Vec<String>, ParseError>(svgs)
    }))
    .map_err(|_| ParseError::ParseError("HWP 렌더 내부 오류 (파일 손상 가능)".to_string()))??;

    let combined = stitch_pages(&pages);
    if combined.len() > RENDER_MAX_SVG_SIZE {
        return Err(ParseError::ParseError(format!(
            "레이아웃 SVG 크기 초과: {}MB (최대 {}MB)",
            combined.len() / 1_048_576,
            RENDER_MAX_SVG_SIZE / 1_048_576
        )));
    }
    Ok(combined)
}

/// 페이지별 SVG 를 `data-page` 세로 스택 단일 SVG 로 합친다.
///
/// 각 페이지의 clipPath id 를 `pN-` 로 네임스페이스해 페이지 간 충돌을 막는다
/// (rhwp SVG 의 유일한 id 참조는 `clip-path="url(#…)"` — gradient/mask/use/href 없음).
/// 각 페이지 SVG 는 자체 흰 배경 rect 를 포함하므로, 페이지 사이 `PAGE_GAP` 만큼 회색
/// 컨테이너 배경이 비쳐 자연스러운 페이지 경계가 된다. 프론트 LayoutView 는 `data-page`
/// 개수로 페이지를 세고 `[data-page="N"]` 로 점프한다.
fn stitch_pages(pages: &[String]) -> String {
    let mut y_offset = 0.0f64;
    let mut max_w = 0.0f64;
    let mut groups = String::new();

    for (i, svg) in pages.iter().enumerate() {
        let n = i + 1;
        let (w, h) = parse_viewbox(svg).unwrap_or((max_w.max(1.0), 1.0));
        if w > max_w {
            max_w = w;
        }
        let inner = strip_svg_wrapper(svg);
        let namespaced = namespace_ids(inner, n);
        groups.push_str(&format!(
            r#"<g data-page="{}" transform="translate(0,{:.3})">{}</g>"#,
            n, y_offset, namespaced
        ));
        y_offset += h + PAGE_GAP;
    }

    let total_h = (y_offset - PAGE_GAP).max(0.0);
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{:.3}" height="{:.3}" viewBox="0 0 {:.3} {:.3}">{}</svg>"#,
        max_w, total_h, max_w, total_h, groups
    )
}

/// SVG 루트의 `viewBox="0 0 W H"` 에서 (W, H) 추출.
fn parse_viewbox(svg: &str) -> Option<(f64, f64)> {
    const KEY: &str = "viewBox=\"";
    let start = svg.find(KEY)? + KEY.len();
    let end = svg[start..].find('"')? + start;
    let nums: Vec<f64> = svg[start..end]
        .split_whitespace()
        .filter_map(|s| s.parse().ok())
        .collect();
    if nums.len() == 4 {
        Some((nums[2], nums[3]))
    } else {
        None
    }
}

/// `<svg …>` 여는 태그와 `</svg>` 닫는 태그를 벗겨 내부 콘텐츠만 반환.
/// (여는 태그 속성값에는 `>` 가 없어 첫 `>` 가 태그 끝이다.)
fn strip_svg_wrapper(svg: &str) -> &str {
    let open = match svg.find("<svg") {
        Some(i) => i,
        None => return svg,
    };
    let content_start = match svg[open..].find('>') {
        Some(i) => open + i + 1,
        None => return svg,
    };
    let content_end = svg.rfind("</svg>").unwrap_or(svg.len());
    if content_end >= content_start {
        &svg[content_start..content_end]
    } else {
        svg
    }
}

/// 한 페이지 조각 내 `id="X"` 와 `url(#X)` 를 `pN-` 접두로 네임스페이스.
fn namespace_ids(inner: &str, page: usize) -> String {
    use std::sync::OnceLock;
    static ID_RE: OnceLock<regex::Regex> = OnceLock::new();
    static URL_RE: OnceLock<regex::Regex> = OnceLock::new();
    let id_re = ID_RE.get_or_init(|| regex::Regex::new(r#"id="([^"]+)""#).unwrap());
    let url_re = URL_RE.get_or_init(|| regex::Regex::new(r"url\(#([^)]+)\)").unwrap());

    let step1 = id_re.replace_all(inner, |c: &regex::Captures| {
        format!(r#"id="p{}-{}""#, page, &c[1])
    });
    url_re
        .replace_all(&step1, |c: &regex::Captures| {
            format!("url(#p{}-{})", page, &c[1])
        })
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewbox_parsed() {
        let svg = r#"<svg xmlns="..." width="793.7" height="1122.5" viewBox="0 0 793.7 1122.5">x</svg>"#;
        assert_eq!(parse_viewbox(svg), Some((793.7, 1122.5)));
    }

    #[test]
    fn wrapper_stripped() {
        let svg = r#"<svg a="b"><rect/><g/></svg>"#;
        assert_eq!(strip_svg_wrapper(svg), "<rect/><g/>");
    }

    #[test]
    fn ids_namespaced_per_page() {
        let inner = r#"<clipPath id="cell-clip-3"><rect/></clipPath><g clip-path="url(#cell-clip-3)"/>"#;
        let out = namespace_ids(inner, 2);
        assert!(out.contains(r#"id="p2-cell-clip-3""#), "id 접두: {out}");
        assert!(out.contains("url(#p2-cell-clip-3)"), "ref 접두: {out}");
        assert!(!out.contains("url(#cell-clip-3)"), "원본 ref 잔존: {out}");
    }

    #[test]
    fn stitch_two_pages_has_data_page_and_stacks() {
        let p1 = r#"<svg viewBox="0 0 100 200"><rect id="a"/><g clip-path="url(#a)"/></svg>"#;
        let p2 = r#"<svg viewBox="0 0 120 200"><rect id="a"/><g clip-path="url(#a)"/></svg>"#;
        let out = stitch_pages(&[p1.to_string(), p2.to_string()]);
        // data-page 2개
        assert_eq!(out.matches("data-page=").count(), 2, "{out}");
        // 페이지별 ID 네임스페이스로 충돌 제거
        assert!(out.contains(r#"id="p1-a""#) && out.contains(r#"id="p2-a""#), "{out}");
        // 2페이지는 1페이지 높이(200)+GAP(12) 만큼 아래로
        assert!(out.contains("translate(0,212.000)"), "2페이지 오프셋: {out}");
        // 외곽 폭은 최대폭(120), 높이는 200+12+200
        assert!(out.contains(r#"width="120.000""#), "최대폭: {out}");
        assert!(out.contains(r#"height="412.000""#), "총높이: {out}");
    }
}
