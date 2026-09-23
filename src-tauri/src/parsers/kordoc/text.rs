//! kordoc 출력 후처리: 청크 페이지 매핑, HTML 표 직렬화, 이미지 참조 제거, 날짜 파싱.

use super::KordocBlock;

// ─── 청크 페이지 매핑 (#66) ──────────────────────────

/// 블록 원문에서 마크다운 변환(escapeGfm `\*`·헤딩 `###`·HTML 표 직렬화)의 영향을
/// 받지 않는 선두 조각을 뽑는다 — content 커서 검색용 needle.
pub(super) fn markdown_safe_prefix(text: &str) -> String {
    text.trim_start()
        .chars()
        .take_while(|c| {
            !matches!(
                c,
                '\\' | '*' | '_' | '~' | '`' | '[' | ']' | '<' | '>' | '|' | '#' | '\n'
            )
        })
        .take(24)
        .collect()
}

/// chars[from..] 에서 needle(char 슬라이스)의 첫 등장 위치 (char 인덱스)
fn find_chars_from(chars: &[char], from: usize, needle: &[char]) -> Option<usize> {
    if needle.is_empty() || from >= chars.len() {
        return None;
    }
    chars[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| from + p)
}

/// 블록 IR 의 쪽 번호를 content 문자 오프셋 브레이크포인트로 사영.
/// 매칭 실패 블록은 건너뛴다 — 페이지 경계는 그 페이지의 후속 블록들로 재시도되므로
/// 근사 정확도가 유지되고, 못 찾으면 이전 페이지가 이어질 뿐 오배정은 없다.
fn page_breakpoints(chars: &[char], blocks: &[KordocBlock]) -> Vec<(usize, usize)> {
    let mut points: Vec<(usize, usize)> = Vec::new();
    let mut cursor = 0usize;
    let mut last_page = 0usize;
    for b in blocks {
        let Some(page) = b.page_number else { continue };
        // needle 후보: 문단/헤딩은 본문, 표는 첫 비어있지 않은 셀
        let text = if b.block_type == "table" {
            b.table.as_ref().and_then(|t| {
                t.cells
                    .iter()
                    .flatten()
                    .find_map(|c| c.text.as_deref().filter(|s| !s.trim().is_empty()))
            })
        } else {
            b.text.as_deref()
        };
        let Some(text) = text else { continue };
        let needle: Vec<char> = markdown_safe_prefix(text).chars().collect();
        if needle.len() < 4 {
            continue; // 너무 짧으면 오매칭 위험
        }
        if let Some(found) = find_chars_from(chars, cursor, &needle) {
            if page != last_page {
                points.push((found, page));
                last_page = page;
            }
            cursor = found + 1; // 동일 텍스트 반복 블록도 전진하도록 최소 전진
        }
    }
    // HWP5 머리말 블록이 본문 앞(오프셋 0 부근)에 본문보다 큰 쪽 번호로 놓이는 변칙 방어
    if points.len() >= 2 && points[0].1 > points[1].1 {
        points.remove(0);
    }
    points
}

/// 청크(char 오프셋)에 페이지 번호·위치 힌트를 부여한다. content 는 불변 —
/// 브레이크포인트를 못 만들면 아무것도 하지 않는다(종전 동작).
pub(super) fn annotate_chunk_pages(
    chunks: &mut [crate::parsers::DocumentChunk],
    content: &str,
    blocks: &[KordocBlock],
) {
    let chars: Vec<char> = content.chars().collect();
    let points = page_breakpoints(&chars, blocks);
    if points.is_empty() {
        return;
    }
    let page_at = |off: usize| -> usize {
        match points.binary_search_by(|p| p.0.cmp(&off)) {
            Ok(i) => points[i].1,
            Err(0) => 1, // 첫 브레이크포인트 이전 = 문서 시작부
            Err(i) => points[i - 1].1,
        }
    };
    for ch in chunks.iter_mut() {
        let start = page_at(ch.start_offset);
        let end = page_at(ch.end_offset.saturating_sub(1)).max(start);
        ch.page_number = Some(start);
        ch.page_end = Some(end);
        ch.location_hint = Some(if start == end {
            format!("페이지 {}", start)
        } else {
            format!("페이지 {}-{}", start, end)
        });
    }
}

/// ISO 8601 → Unix timestamp (chrono 활용)
pub(super) fn parse_iso_timestamp(s: &str) -> Option<i64> {
    // chrono는 이미 Cargo.toml에 의존성으로 포함되어 있음
    use chrono::{DateTime, NaiveDateTime};

    // "2024-01-15T09:30:00Z" 또는 "2024-01-15T09:30:00+09:00"
    if let Ok(dt) = DateTime::parse_from_rfc3339(s.trim()) {
        return Some(dt.timestamp());
    }
    // "2024-01-15T09:30:00" (timezone 없음 → UTC 가정)
    if let Ok(dt) = NaiveDateTime::parse_from_str(s.trim(), "%Y-%m-%dT%H:%M:%S") {
        return Some(dt.and_utc().timestamp());
    }
    // "2024-01-15" (날짜만)
    if let Ok(d) = chrono::NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d") {
        return d.and_hms_opt(0, 0, 0).map(|dt| dt.and_utc().timestamp());
    }
    None
}

/// kordoc 마크다운의 HTML 표(`<table>…</table>`)를 검색 인덱스용 plain text 로 변환한다.
///
/// 행은 줄바꿈, 셀은 공백으로 직렬화하고 변환되지 못한(중첩 표 등) 잔여 표/줄바꿈 태그를
/// 제거한다. 표가 없으면 입력을 그대로 돌려준다(정규식 매칭 0 → 사실상 무비용).
/// 밑줄 태그(`<u>`, kordoc v4.7.0+)도 함께 걷어낸다 — 표 바깥 본문에 그대로 남으면
/// 검색 스니펫에 태그가 노출되고 구절 검색이 태그 경계에서 끊긴다.
/// 미리보기 패널은 `get_markdown` 원본(HTML 표·밑줄 유지)을 쓰므로 GFM 렌더에는 영향이 없다.
pub(super) fn html_tables_to_text(md: &str) -> String {
    use std::sync::OnceLock;
    static TABLE_RE: OnceLock<regex::Regex> = OnceLock::new();
    static ROW_RE: OnceLock<regex::Regex> = OnceLock::new();
    static CELL_RE: OnceLock<regex::Regex> = OnceLock::new();
    static INNER_TAG_RE: OnceLock<regex::Regex> = OnceLock::new();
    static LEFTOVER_RE: OnceLock<regex::Regex> = OnceLock::new();
    static UNDERLINE_RE: OnceLock<regex::Regex> = OnceLock::new();

    let table_re =
        TABLE_RE.get_or_init(|| regex::Regex::new(r"(?is)<table[^>]*>.*?</table>").unwrap());
    let row_re = ROW_RE.get_or_init(|| regex::Regex::new(r"(?is)<tr[^>]*>(.*?)</tr>").unwrap());
    let cell_re =
        CELL_RE.get_or_init(|| regex::Regex::new(r"(?is)<t[dh][^>]*>(.*?)</t[dh]>").unwrap());
    // 태그 이름은 ASCII 영문자로 시작한다. kordoc 은 셀 텍스트의 꺾쇠를 이스케이프하지 않아
    // 별지서식 연혁 `<개정 2019. 7. 18.>` 가 그대로 오므로, `<[^>]+>` 로 지우면 색인에서 빠진다.
    let inner_tag_re =
        INNER_TAG_RE.get_or_init(|| regex::Regex::new(r"(?is)</?[a-z][^>]*>").unwrap());
    let leftover_re = LEFTOVER_RE.get_or_init(|| {
        regex::Regex::new(r"(?is)</?(?:table|thead|tbody|tfoot|tr|td|th|col|colgroup|br)[^>]*>")
            .unwrap()
    });
    // 낱말 중간에도 열리고 닫히므로 공백이 아니라 삭제 — 공백을 넣으면 단어가 쪼개진다.
    let underline_re = UNDERLINE_RE.get_or_init(|| regex::Regex::new(r"(?i)</?u>").unwrap());

    let replaced = table_re.replace_all(md, |caps: &regex::Captures| {
        let table = &caps[0];
        let mut rows: Vec<String> = Vec::new();
        for row in row_re.captures_iter(table) {
            let mut cells: Vec<String> = Vec::new();
            for cell in cell_re.captures_iter(&row[1]) {
                let stripped = inner_tag_re.replace_all(&cell[1], " ");
                let cell_text = stripped.split_whitespace().collect::<Vec<_>>().join(" ");
                if !cell_text.is_empty() {
                    cells.push(cell_text);
                }
            }
            if !cells.is_empty() {
                rows.push(cells.join(" "));
            }
        }
        if rows.is_empty() {
            String::new()
        } else {
            format!("\n{}\n", rows.join("\n"))
        }
    });

    // 중첩 표 등으로 위 변환을 빠져나간 잔여 표/줄바꿈 태그 제거 (정규식은 중첩을 셀 수 없음).
    let leftover_cleaned = leftover_re.replace_all(&replaced, " ");
    underline_re.replace_all(&leftover_cleaned, "").into_owned()
}

/// 마크다운 이미지 참조(`![image](image_001.png)`)와 `<img>` 태그를 지운다.
/// kordoc 은 그림 자리에 이 참조를 남기는데, 앱은 이미지 파일을 받지 않으므로 색인에는 잡음이다.
pub(super) fn strip_image_refs(text: &str) -> String {
    use std::sync::OnceLock;
    static IMAGE_RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = IMAGE_RE
        .get_or_init(|| regex::Regex::new(r"(?i)!\[[^\]]*\]\([^)]*\)|<img\b[^>]*>").unwrap());
    re.replace_all(text, "").into_owned()
}
