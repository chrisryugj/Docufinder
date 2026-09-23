use super::process::failure_json;
use super::text::markdown_safe_prefix;
use super::*;

fn block(block_type: &str, text: Option<&str>, page: usize) -> KordocBlock {
    KordocBlock {
        block_type: block_type.to_string(),
        text: text.map(String::from),
        page_number: Some(page),
        table: None,
    }
}

#[test]
fn chunk_pages_annotated_from_blocks() {
    // 3페이지 문서 — 마크다운 헤딩 접두("## ")·escapeGfm(\*) 가 섞여도
    // 블록 원문 선두 조각으로 매칭돼야 한다.
    let content = "## 첫 페이지 제목\n\n첫 페이지 본문입니다.\n\n둘째 페이지 시작 문단.\n\n셋째 페이지 마지막 문단.";
    let blocks = vec![
        block("heading", Some("첫 페이지 제목"), 1),
        block("paragraph", Some("첫 페이지 본문입니다."), 1),
        block("paragraph", Some("둘째 페이지 시작 문단."), 2),
        block("paragraph", Some("셋째 페이지 마지막 문단."), 3),
    ];
    let mut chunks = chunk_text(content, 20, 4);
    annotate_chunk_pages(&mut chunks, content, &blocks);
    assert!(
        chunks.iter().all(|c| c.page_number.is_some()),
        "전 청크 페이지 부여"
    );
    assert_eq!(chunks.first().unwrap().page_number, Some(1));
    assert_eq!(chunks.last().unwrap().page_end, Some(3));
    let hint = chunks.first().unwrap().location_hint.as_deref().unwrap();
    assert!(hint.starts_with("페이지 "), "위치 힌트 형식: {hint}");
}

#[test]
fn chunk_pages_table_block_uses_first_cell() {
    let content = "본문 문단\n\n항목 값 비고 첫셀텍스트 둘째셀\n\n표 다음 문단";
    let blocks = vec![
        block("paragraph", Some("본문 문단"), 1),
        KordocBlock {
            block_type: "table".to_string(),
            text: None,
            page_number: Some(2),
            table: Some(KordocTable {
                cells: vec![vec![
                    KordocCell {
                        text: Some("항목 값 비고".to_string()),
                    },
                    KordocCell {
                        text: Some("둘째셀".to_string()),
                    },
                ]],
            }),
        },
        block("paragraph", Some("표 다음 문단"), 2),
    ];
    let mut chunks = chunk_text(content, 200, 20);
    annotate_chunk_pages(&mut chunks, content, &blocks);
    // 단일 청크: 1페이지에서 시작해 2페이지에서 끝난다
    assert_eq!(chunks[0].page_number, Some(1));
    assert_eq!(chunks[0].page_end, Some(2));
    assert_eq!(chunks[0].location_hint.as_deref(), Some("페이지 1-2"));
}

#[test]
fn chunk_pages_unmatched_blocks_are_skipped() {
    // 어떤 블록도 content 와 매칭되지 않으면 아무것도 하지 않는다 (종전 동작)
    let content = "완전히 다른 내용";
    let blocks = vec![block("paragraph", Some("매칭될 수 없는 텍스트"), 2)];
    let mut chunks = chunk_text(content, 100, 10);
    annotate_chunk_pages(&mut chunks, content, &blocks);
    assert_eq!(chunks[0].page_number, None);
    assert_eq!(chunks[0].location_hint, None);
}

#[test]
fn markdown_safe_prefix_cuts_specials() {
    assert_eq!(markdown_safe_prefix("가나다*라마"), "가나다");
    assert_eq!(markdown_safe_prefix("  선두공백 제거"), "선두공백 제거");
    assert_eq!(markdown_safe_prefix("한줄\n두줄"), "한줄");
    assert_eq!(
        markdown_safe_prefix("긴텍스트".repeat(20).as_str())
            .chars()
            .count(),
        24
    );
}

#[test]
fn html_table_serialized_to_plain_text() {
    let md = "앞\n<table><tr><td>A</td><td colspan=\"2\">B</td></tr>\
              <tr><td>C</td><td>D</td></tr></table>\n뒤";
    let out = html_tables_to_text(md);
    assert!(out.contains("A B"), "행1 셀이 공백 결합돼야: {out:?}");
    assert!(out.contains("C D"), "행2 셀이 공백 결합돼야: {out:?}");
    assert!(!out.contains("<td"), "td 태그 잔존: {out:?}");
    assert!(!out.contains("colspan"), "colspan 잔존: {out:?}");
    assert!(
        out.contains("앞") && out.contains("뒤"),
        "표 바깥 본문 보존: {out:?}"
    );
}

#[test]
fn no_table_is_noop() {
    let md = "# 제목\n본문 텍스트 — GFM 표 아님 | 열1 | 열2";
    assert_eq!(html_tables_to_text(md), md);
}

#[test]
fn underline_tags_removed_without_splitting_words() {
    // kordoc v4.7.0+ 는 밑줄을 <u>…</u> 로 방출한다. 낱말 중간에서 열려도
    // 공백이 끼면 FTS 토큰이 쪼개지므로 태그만 지운다.
    let md = "부서명은 <u>소방행정과</u>이고 밑<u>줄</u>도 있다";
    let out = html_tables_to_text(md);
    assert_eq!(out, "부서명은 소방행정과이고 밑줄도 있다", "출력: {out:?}");
}

#[test]
fn leftover_table_tags_removed() {
    // 닫는 </table> 가 없어 표 단위 변환에 실패해도 잔여 태그는 정리된다.
    let md = "<tr><td>x</td><td>y</td>";
    let out = html_tables_to_text(md);
    assert!(
        !out.contains("<td") && !out.contains("<tr"),
        "잔여 태그 정리 실패: {out:?}"
    );
    assert!(
        out.contains('x') && out.contains('y'),
        "셀 텍스트 보존: {out:?}"
    );
}

#[test]
fn th_header_and_inline_tags() {
    // <th> 헤더 셀 + 셀 내부 인라인 태그(<b>)도 텍스트만 남는다.
    let md = "<table><tr><th>구분</th><th>값</th></tr>\
              <tr><td><b>합계</b></td><td>100</td></tr></table>";
    let out = html_tables_to_text(md);
    assert!(out.contains("구분 값"), "헤더 행: {out:?}");
    assert!(
        out.contains("합계 100"),
        "본문 행 + 인라인 태그 제거: {out:?}"
    );
    assert!(!out.contains('<'), "꺾쇠 잔존: {out:?}");
}

#[test]
fn angle_bracket_text_in_cell_kept() {
    // 법령 별지서식 머리행: 연혁 표기의 꺾쇠는 태그가 아니라 본문이다.
    let md = "<table><tr><th colspan=\"4\">■ 시행규칙 [별지 제7호서식] <개정 2019. 7. 18.></th></tr></table>";
    let out = html_tables_to_text(md);
    assert!(
        out.contains("<개정 2019. 7. 18.>"),
        "연혁 표기 소실: {out:?}"
    );
    assert!(
        !out.contains("<th") && !out.contains("colspan"),
        "태그 잔존: {out:?}"
    );
}

#[test]
fn multiple_tables_with_br_and_between_text() {
    let md = "<table><tr><td>a</td></tr></table>중간<table><tr><td>b<br>c</td></tr></table>";
    let out = html_tables_to_text(md);
    assert!(
        out.contains('a') && out.contains('b') && out.contains('c') && out.contains("중간"),
        "내용 보존: {out:?}"
    );
    assert!(
        !out.contains("<br") && !out.contains("<td"),
        "태그 잔존: {out:?}"
    );
}

#[test]
fn nested_table_leaves_no_tags() {
    // 셀 안 중첩 표는 non-greedy 매칭이 안쪽 </table> 에서 멈춰 바깥 잔여가 생기지만,
    // leftover 정리로 태그가 본문에 노출되지 않는다 (스니펫 노출 방어가 목적).
    let md = "<table><tr><td><table><tr><td>안</td></tr></table></td></tr></table>";
    let out = html_tables_to_text(md);
    assert!(out.contains('안'), "내부 셀 텍스트 보존: {out:?}");
    assert!(
        !out.contains("<table") && !out.contains("<td") && !out.contains("</"),
        "태그 잔존: {out:?}"
    );
}

#[test]
fn empty_cells_skipped() {
    let md = "<table><tr><td></td><td>값</td><td>   </td></tr></table>";
    let out = html_tables_to_text(md);
    assert_eq!(out.trim(), "값", "빈 셀은 스킵: {out:?}");
}

/// kordoc v4.2.0 JSON 계약 — 구조화 신호(warnings.code / isImageBased /
/// pageQuality.ocrReason·ocrApplied) 역직렬화. 필드 추가/이름 변경 회귀 방지.
#[test]
fn kordoc_response_v42_signals_deserialize() {
    let json = r#"{
            "success": true,
            "fileType": "pdf",
            "markdown": "본문",
            "metadata": {"pageCount": 2},
            "warnings": [
                {"message": "이미지 기반 PDF", "code": "NEEDS_OCR"},
                {"page": 2, "message": "2개 페이지에 OCR 적용", "code": "OCR_APPLIED"}
            ],
            "isImageBased": true,
            "pageQuality": [
                {"page": 1, "textChars": 0, "needsOcr": true, "ocrReason": "low_text", "ocrApplied": true},
                {"page": 2, "textChars": 500, "needsOcr": true, "ocrReason": "garbled_hangul"}
            ],
            "qualitySummary": {"needsOcr": true, "ocrCandidatePages": [1, 2]}
        }"#;
    let resp: KordocResponse = serde_json::from_str(json).expect("역직렬화");
    assert!(resp.success);
    assert_eq!(resp.is_image_based, Some(true));
    assert_eq!(
        resp.warnings
            .iter()
            .filter_map(|w| w.code.as_deref())
            .collect::<Vec<_>>(),
        vec!["NEEDS_OCR", "OCR_APPLIED"]
    );
    assert!(resp.page_quality[0].ocr_applied);
    assert!(!resp.page_quality[1].ocr_applied);
    assert_eq!(
        resp.page_quality[1].ocr_reason.as_deref(),
        Some("garbled_hangul")
    );
}

/// 실 kordoc CLI + OCR 모델 E2E (로컬 전용 — 환경변수 없으면 skip).
///
/// 실행: KORDOC_CLI_PATH=<kordoc/dist/cli.js> KORDOC_E2E_SCAN_PDF=<scan.pdf> \
///       (선택) KORDOC_MODEL_CACHE=<models dir> \
///       cargo test kordoc_ocr_e2e -- --ignored --nocapture
#[test]
#[ignore = "실 kordoc CLI·OCR 모델 필요 (로컬 전용)"]
fn kordoc_ocr_e2e_scanned_pdf() {
    let Ok(pdf) = std::env::var("KORDOC_E2E_SCAN_PDF") else {
        eprintln!("KORDOC_E2E_SCAN_PDF 미설정 — skip");
        return;
    };
    let doc = parse_with_options(
        Path::new(&pdf),
        KordocOptions {
            formula_ocr: false,
            ocr: KordocOcrMode::Auto,
            password: None,
        },
    )
    .expect("스캔 PDF OCR 파싱");
    assert!(!doc.chunks.is_empty(), "OCR 본문이 청크로 추출돼야 함");
    eprintln!(
        "E2E ok — chunks {}, 첫 청크: {:?}",
        doc.chunks.len(),
        doc.chunks[0].content.chars().take(60).collect::<String>()
    );
}

/// 실행통제 계열 OS 에러만 "차단" 문구로, 그 외는 일반 점검 실패 문구로 분기한다.
#[test]
fn probe_spawn_error_classifies_policy_block() {
    let blocked = probe_spawn_error_message(
        "Parse error: kordoc 프로세스 시작 실패: 액세스가 거부되었습니다. (os error 5)",
    );
    assert!(blocked.contains("실행통제"), "{blocked}");
    assert!(blocked.contains("os error 5"), "원문 보존: {blocked}");
    let policy = probe_spawn_error_message("… (os error 1260)");
    assert!(policy.contains("실행통제"), "{policy}");
    // os error 53(네트워크 경로 없음) 같은 다른 코드는 오분류하지 않는다.
    let other = probe_spawn_error_message("… (os error 53)");
    assert!(!other.contains("실행통제"), "{other}");
    assert!(other.contains("점검 실패"), "{other}");
}

/// 실 kordoc CLI 로 시작 시 런타임 점검 E2E (로컬 전용).
///
/// 실행: KORDOC_CLI_PATH=<kordoc/dist/cli.js> cargo test probe_runtime_e2e -- --ignored --nocapture
#[test]
#[ignore = "실 kordoc CLI 필요 (로컬 전용)"]
fn probe_runtime_e2e_reports_version() {
    let version = probe_runtime().expect("번들/로컬 kordoc 은 --version 에 0 으로 종료해야 함");
    assert!(
        version.split('.').count() >= 3,
        "semver 형태여야 함: {version:?}"
    );
    eprintln!("probe ok — kordoc v{version}");
}

/// kordoc v4.12+ 실패 계약(#69): exit 1 + stdout 실패 JSON. 원인 코드가 살아서 넘어가야 한다.
#[test]
fn failure_json_survives_nonzero_exit() {
    let out = b"{\n  \"success\": false,\n  \"fileType\": \"hwpx\",\n  \"error\": \"\xec\x95\x94\xed\x98\xb8\",\n  \"code\": \"ENCRYPTED\"\n}\n";
    let json = failure_json(out).expect("실패 JSON 추출");
    let resp: KordocResponse = serde_json::from_str(&json).expect("역직렬화");
    assert!(!resp.success);
    assert_eq!(resp.code.as_deref(), Some("ENCRYPTED"));
    // pdfjs 경고가 앞에 붙어도 JSON 시작점부터 읽는다.
    assert!(failure_json(b"Warning: TT: x\n{\"success\": false}").is_some());
    // 성공 JSON·JSON 없음은 실패 JSON 이 아니다.
    assert!(failure_json(b"{\"success\": true, \"markdown\": \"x\"}").is_none());
    assert!(failure_json(b"error: unknown option").is_none());
}

#[test]
fn image_refs_stripped_from_index_text() {
    let md = "앞 문단\n\n![image](image_001.png)\n\n표 뒤 <img src=\"image_002.jpg\" alt=\"\"> 글";
    let out = strip_image_refs(md);
    assert!(!out.contains("image_00"), "이미지 참조 잔존: {out:?}");
    assert!(
        out.contains("앞 문단") && out.contains("글"),
        "본문 보존: {out:?}"
    );
    // 대괄호·괄호만 쓰는 일반 문장은 건드리지 않는다.
    assert_eq!(strip_image_refs("[붙임] 계획(안)"), "[붙임] 계획(안)");
}

/// 스캔 PDF 는 kordoc 이 `![image](…)` 만 내므로 본문이 비지 않는다. 이미지 기반 PDF 로
/// 분류돼야 한다. 실행: KORDOC_CLI_PATH=<kordoc/dist/cli.js> cargo test scanned_pdf_e2e -- --ignored
#[test]
#[ignore = "실 kordoc CLI 필요 (로컬 전용)"]
fn scanned_pdf_e2e_is_image_based() {
    let pdf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/multipage_scanned.pdf");
    let err = parse(&pdf).expect_err("스캔 PDF 는 본문 없음으로 분류돼야 한다");
    assert!(err.to_string().contains("이미지 기반 PDF"), "{err}");
}

/// 구버전 kordoc JSON (code/pageQuality 없음)도 그대로 파싱돼야 한다 — 하위호환.
#[test]
fn kordoc_response_legacy_shape_still_parses() {
    let json = r#"{"success": true, "markdown": "본문", "metadata": null,
            "warnings": [{"message": "경고만 있음"}], "error": null}"#;
    let resp: KordocResponse = serde_json::from_str(json).expect("역직렬화");
    assert!(resp.warnings[0].code.is_none());
    assert!(resp.page_quality.is_empty());
    assert_eq!(resp.is_image_based, None);
    assert!(resp.blocks.is_empty());
}

/// kordoc v4.7.3 실 JSON 형태 계약 — IRTable 의 rows/cols 는 **정수(개수)** 고
/// 셀 데이터는 cells 다. 배열로 가정하면 역직렬화가 통째로 죽는다 (실출력 대조로 확정).
#[test]
fn kordoc_response_v473_blocks_shape_parses() {
    let json = r#"{"success": true, "fileType": "hwpx", "markdown": "본문",
            "blocks": [
                {"type": "table", "pageNumber": 1, "table": {"rows": 2, "cols": 2, "hasHeader": true,
                    "cells": [[{"text": "셀A", "colSpan": 1, "rowSpan": 1}, {"text": "셀B", "colSpan": 1, "rowSpan": 1}]]}},
                {"type": "paragraph", "text": "둘째 쪽 문단", "pageNumber": 2}
            ],
            "metadata": {"pageCount": 2, "pageMode": "layout"}, "pageCount": 2}"#;
    let resp: KordocResponse = serde_json::from_str(json).expect("v4.7.3 형태 역직렬화");
    assert_eq!(resp.blocks.len(), 2);
    assert_eq!(resp.blocks[0].block_type, "table");
    assert_eq!(
        resp.blocks[0].table.as_ref().unwrap().cells[0][0]
            .text
            .as_deref(),
        Some("셀A")
    );
    assert_eq!(resp.blocks[1].page_number, Some(2));
    assert_eq!(resp.metadata.unwrap().page_mode.as_deref(), Some("layout"));
}

/// 실문서 폴더를 파싱해 파일별 본문 글자 수·지문·시간을 찍는다. kordoc 버전·경로(상주 워커 대
/// 1회성 실행, 이미지 끄기) 사이에 색인 본문이 같은지와 속도를 비교할 때 쓴다.
/// `KORDOC_E2E_DIR=<폴더> [KORDOC_CLI_PATH=<cli.js>] cargo test --lib kordoc_corpus_e2e -- --ignored --nocapture`
#[test]
#[ignore = "실문서 폴더 필요 (로컬 전용)"]
fn kordoc_corpus_e2e() {
    use std::hash::{Hash, Hasher};
    let Ok(dir) = std::env::var("KORDOC_E2E_DIR") else {
        return;
    };
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("KORDOC_E2E_DIR 읽기")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            matches!(
                p.extension().and_then(|e| e.to_str()),
                Some("hwp" | "hwpx" | "pdf" | "docx")
            )
        })
        .collect();
    files.sort();
    let started = std::time::Instant::now();
    for f in &files {
        let t = std::time::Instant::now();
        let name = f.file_name().unwrap_or_default().to_string_lossy();
        match parse(f) {
            Ok(doc) => {
                // 공백 차이(그림 자리 앞뒤 빈 줄 등)는 색인에 영향이 없어 지문에서 뺀다
                let dense: String = doc.content.split_whitespace().collect();
                let mut h = std::collections::hash_map::DefaultHasher::new();
                dense.hash(&mut h);
                println!(
                    "CORPUS\t{name}\t{}\t{:016x}\t{}ms",
                    dense.chars().count(),
                    h.finish(),
                    t.elapsed().as_millis()
                );
            }
            Err(e) => println!("CORPUS\t{name}\tERR\t{e}\t{}ms", t.elapsed().as_millis()),
        }
    }
    println!(
        "CORPUS_TOTAL\t{} files\t{}ms",
        files.len(),
        started.elapsed().as_millis()
    );
}
