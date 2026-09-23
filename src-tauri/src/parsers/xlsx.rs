use super::{DocumentChunk, DocumentMetadata, ParseError, ParsedDocument};
use calamine::{open_workbook_auto, Data, Reader, Sheets};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;

/// 최대 XLSX 파일 크기 (100MB)
const MAX_FILE_SIZE: u64 = 100 * 1024 * 1024;

/// 시트당 최대 처리 행 수 (대용량 시트 행 방지)
const MAX_ROWS_PER_SHEET: usize = 50_000;

/// 전체 문서 최대 문자 수
const MAX_TOTAL_CHARS: usize = 5_000_000;

/// XLSX/XLS 파일 파싱
/// calamine 크레이트 사용, 시트/행 정보 포함
pub fn parse(path: &Path) -> Result<ParsedDocument, ParseError> {
    let _bc = crate::breadcrumb::Guard::new(path, "parse_xlsx");

    // 파일 크기 제한 (압축 폭탄 방어)
    if let Ok(metadata) = std::fs::metadata(path) {
        if metadata.len() > MAX_FILE_SIZE {
            return Err(ParseError::ParseError(format!(
                "파일 크기 초과: {} bytes (최대 {} bytes)",
                metadata.len(),
                MAX_FILE_SIZE
            )));
        }
    }

    // open_workbook_auto 자체도 calamine 내부에서 panic 가능 (CFB sector chain 비정상 등).
    // catch_unwind 로 격리해 한 파일 panic 이 thread 전체를 죽이지 않도록 한다.
    let open_result = catch_unwind(AssertUnwindSafe(|| open_workbook_auto(path)));
    let mut workbook = match open_result {
        Ok(Ok(wb)) => wb,
        Ok(Err(e)) => {
            let msg = e.to_string().to_lowercase();
            if msg.contains("password") || msg.contains("encrypt") || msg.contains("cfb") {
                return Err(ParseError::PasswordProtected(
                    "암호로 보호된 엑셀 파일입니다".to_string(),
                ));
            }
            return Err(ParseError::ParseError(e.to_string()));
        }
        Err(_) => {
            tracing::warn!(
                "XLSX open panicked (calamine internal, file likely malformed): {}",
                path.display()
            );
            return Err(ParseError::ParseError(
                "엑셀 파일 구조가 비정상입니다 (open 단계 패닉)".to_string(),
            ));
        }
    };

    let sheet_names = workbook.sheet_names().to_vec();
    tracing::info!(
        "XLSX parsing: {} ({} sheet(s))",
        path.display(),
        sheet_names.len()
    );

    let mut all_text = String::new();
    let mut chunks = Vec::new();
    let mut global_offset = 0;
    let mut sheets_panicked: usize = 0;
    let mut sheets_extracted: usize = 0;

    for sheet_name in sheet_names {
        // 전체 문서 문자 수 제한: 시트 간 누적 체크
        if all_text.len() > MAX_TOTAL_CHARS {
            tracing::warn!(
                "XLSX truncated at {} chars (max {}), remaining sheets skipped",
                all_text.len(),
                MAX_TOTAL_CHARS
            );
            break;
        }

        // 시트별 격리: worksheet_range + extract_text_with_location 을 catch_unwind 로 감싼다.
        // 한 시트가 panic 해도 다른 시트는 진행.
        // NEIS Report Designer 가 출력한 구형 BIFF8 같은 비표준 포맷에서 calamine 0.26 이
        // sector / range 빌드 단계에서 panic 한 사례가 보고되어 추가.
        let sheet_result = catch_unwind(AssertUnwindSafe(|| {
            let rows = sheet_rows(&mut workbook, &sheet_name)?;
            Some(rows_to_text_and_chunks(&rows, &sheet_name, global_offset))
        }));

        let (sheet_text, sheet_chunks) = match sheet_result {
            Ok(Some(extracted)) => extracted,
            Ok(None) => {
                tracing::warn!(
                    "XLSX sheet '{}' skipped (worksheet_range error): {}",
                    sheet_name,
                    path.display()
                );
                continue;
            }
            Err(_) => {
                sheets_panicked += 1;
                tracing::warn!(
                    "XLSX sheet '{}' panicked (skipped): {}",
                    sheet_name,
                    path.display()
                );
                continue;
            }
        };

        if !sheet_text.is_empty() {
            sheets_extracted += 1;
            if !all_text.is_empty() {
                all_text.push_str("\n\n");
                global_offset += 2;
            }
            // 시트 이름 추가
            let header = format!("[{}]\n", sheet_name);
            all_text.push_str(&header);
            global_offset += header.len();

            all_text.push_str(&sheet_text);
            global_offset += sheet_text.len();

            chunks.extend(sheet_chunks);
        }
    }

    tracing::info!(
        "XLSX done: {} (sheets: {} extracted, {} panicked, {} chunks, {} chars)",
        path.display(),
        sheets_extracted,
        sheets_panicked,
        chunks.len(),
        all_text.len()
    );

    if all_text.is_empty() {
        tracing::warn!("XLSX file has no text content: {:?}", path);
    }

    Ok(ParsedDocument {
        content: all_text,
        metadata: DocumentMetadata {
            title: path.file_stem().and_then(|s| s.to_str()).map(String::from),
            author: None,
            created_at: None,
            page_count: None,
        },
        chunks,
        garbled_hint: false,
    })
}

/// 시트 한 장의 (1-based 행 번호, 행 텍스트) 목록. 행 텍스트는 비지 않은 셀을 열 순서로 탭으로 잇는다.
///
/// xlsx·xlsb 는 셀을 하나씩 읽는다. `worksheet_range` 는 쓰인 셀을 감싸는 직사각형을 통째로
/// 할당해서(행×열), A1 과 XFD1048576 에만 값이 있어도 170억 칸을 잡다가 프로세스가 죽는다
/// (할당 실패는 catch_unwind 로 못 잡고, 시작 동기화 때마다 반복된다). xls(최대 65,536×256)·ods 는
/// 기존 경로를 쓴다.
fn sheet_rows<RS: std::io::Read + std::io::Seek>(
    workbook: &mut Sheets<RS>,
    sheet_name: &str,
) -> Option<Vec<(usize, String)>> {
    let mut cells: Vec<(u32, u32, String)> = Vec::new();
    let mut chars = 0usize;
    // 행 순서로 오므로 글자 상한을 넘기면 그 뒤는 읽지 않는다 (아래 행 조립에서 다시 자른다)
    let mut push = |pos: (u32, u32), value: Data| -> bool {
        if let Some(text) = cell_to_string(&value) {
            chars += text.len();
            cells.push((pos.0, pos.1, text));
        }
        chars <= MAX_TOTAL_CHARS
    };
    match workbook {
        Sheets::Xlsx(x) => {
            let mut reader = x.worksheet_cells_reader(sheet_name).ok()?;
            while let Ok(Some(cell)) = reader.next_cell() {
                if !push(cell.get_position(), Data::from(cell.get_value().clone())) {
                    break;
                }
            }
        }
        Sheets::Xlsb(x) => {
            let mut reader = x.worksheet_cells_reader(sheet_name).ok()?;
            while let Ok(Some(cell)) = reader.next_cell() {
                if !push(cell.get_position(), Data::from(cell.get_value().clone())) {
                    break;
                }
            }
        }
        _ => {
            let range = workbook.worksheet_range(sheet_name).ok()?;
            for (row, col, value) in range.used_cells() {
                let (start_row, start_col) = range.start().unwrap_or((0, 0));
                if !push(
                    (start_row + row as u32, start_col + col as u32),
                    value.clone(),
                ) {
                    break;
                }
            }
        }
    }
    Some(rows_from_cells(cells, sheet_name))
}

/// 셀 목록 → 행 목록. 시트 첫 행부터 MAX_ROWS_PER_SHEET 행, MAX_TOTAL_CHARS 글자까지.
fn rows_from_cells(mut cells: Vec<(u32, u32, String)>, sheet_name: &str) -> Vec<(usize, String)> {
    cells.sort_by_key(|(row, col, _)| (*row, *col));
    let Some(first_row) = cells.first().map(|(row, _, _)| *row as usize) else {
        return Vec::new();
    };
    let mut rows: Vec<(usize, String)> = Vec::new();
    let mut total_chars = 0usize;
    let mut i = 0;
    while i < cells.len() {
        let row = cells[i].0 as usize;
        if row - first_row >= MAX_ROWS_PER_SHEET {
            tracing::warn!(
                "Sheet '{}' truncated at {} rows (max {})",
                sheet_name,
                row - first_row,
                MAX_ROWS_PER_SHEET
            );
            break;
        }
        let mut texts: Vec<&str> = Vec::new();
        while i < cells.len() && cells[i].0 as usize == row {
            texts.push(&cells[i].2);
            i += 1;
        }
        let row_text = texts.join("\t");
        total_chars += row_text.len();
        if total_chars > MAX_TOTAL_CHARS {
            tracing::warn!(
                "Sheet '{}' truncated at {} chars (max {})",
                sheet_name,
                total_chars,
                MAX_TOTAL_CHARS
            );
            break;
        }
        rows.push((row + 1, row_text)); // 1-based Excel row
    }
    rows
}

/// 행 목록 → 시트 텍스트 + 행 정보 포함 청크
fn rows_to_text_and_chunks(
    row_infos: &[(usize, String)],
    sheet_name: &str,
    base_offset: usize,
) -> (String, Vec<DocumentChunk>) {
    let full_text = row_infos
        .iter()
        .map(|(_, text)| text.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    // 행 단위로 청크 생성
    let chunks = create_chunks_with_rows(
        row_infos,
        sheet_name,
        base_offset,
        super::DEFAULT_CHUNK_SIZE,
        super::DEFAULT_CHUNK_OVERLAP,
    );

    (full_text, chunks)
}

/// 행 정보를 유지하면서 청크 생성 (overlap 지원)
fn create_chunks_with_rows(
    row_infos: &[(usize, String)],
    sheet_name: &str,
    base_offset: usize,
    chunk_size: usize,
    overlap: usize,
) -> Vec<DocumentChunk> {
    let mut chunks = Vec::new();
    let n = row_infos.len();

    if n == 0 {
        return chunks;
    }

    let mut start_idx = 0;
    let mut current_offset = base_offset;

    while start_idx < n {
        let mut end_idx = start_idx;
        let mut current_size = 0;

        // chunk_size에 맞게 행 추가
        while end_idx < n {
            let row_size = row_infos[end_idx].1.len() + if end_idx > start_idx { 1 } else { 0 };
            if current_size + row_size > chunk_size && end_idx > start_idx {
                break;
            }
            current_size += row_size;
            end_idx += 1;
        }

        // 무한 루프 방지: 단일 행이 chunk_size를 초과해도 최소 1개 포함
        if end_idx == start_idx {
            end_idx = start_idx + 1;
        }

        let content: String = row_infos[start_idx..end_idx]
            .iter()
            .map(|(_, text)| text.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        let start_row = row_infos[start_idx].0;
        let end_row = row_infos[end_idx - 1].0;

        chunks.push(DocumentChunk {
            content: content.clone(),
            start_offset: current_offset,
            end_offset: current_offset + content.len(),
            page_number: None,
            page_end: None,
            location_hint: Some(format_location_hint(sheet_name, start_row, end_row)),
        });

        current_offset += content.len() + 1;

        if overlap == 0 {
            start_idx = end_idx;
        } else {
            // 오버랩: 이전 청크 끝 행들을 다음 청크에 포함하여 문맥 연속성 보장
            let mut overlap_size = 0;
            let mut new_start = end_idx;
            for idx in (start_idx..end_idx).rev() {
                let row_size = row_infos[idx].1.len() + 1;
                if overlap_size + row_size > overlap && new_start < end_idx {
                    break;
                }
                overlap_size += row_size;
                new_start = idx;
            }
            // 최소 1행 이상 전진 (무한 루프 방지)
            start_idx = new_start.max(start_idx + 1);
        }
    }

    chunks
}

/// 위치 힌트 포맷팅: "Sheet1!행1-50" 또는 "Sheet1!행5"
fn format_location_hint(sheet_name: &str, start_row: usize, end_row: usize) -> String {
    if start_row == end_row {
        format!("{}!행{}", sheet_name, start_row)
    } else {
        format!("{}!행{}-{}", sheet_name, start_row, end_row)
    }
}

/// 셀 값을 문자열로 변환
fn cell_to_string(cell: &Data) -> Option<String> {
    match cell {
        Data::Empty => None,
        Data::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Data::Int(i) => Some(i.to_string()),
        Data::Float(f) => Some(format!("{:.2}", f)),
        Data::Bool(b) => Some(b.to_string()),
        Data::DateTime(dt) => Some(dt.to_string()),
        Data::DateTimeIso(s) => Some(s.to_string()),
        Data::DurationIso(s) => Some(s.to_string()),
        Data::Error(e) => {
            tracing::debug!("Cell error: {:?}", e);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// 셀 두 개(A1, XFD1048576)만 있는 xlsx. 쓰인 셀을 감싸는 직사각형은 170억 칸이다.
    fn write_far_corner_xlsx(path: &Path) {
        let file = std::fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let opts: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default();
        let parts = [
            (
                "[Content_Types].xml",
                r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>"#,
            ),
            (
                "_rels/.rels",
                r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#,
            ),
            (
                "xl/workbook.xml",
                r#"<?xml version="1.0" encoding="UTF-8"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="자료" sheetId="1" r:id="rId1"/></sheets></workbook>"#,
            ),
            (
                "xl/_rels/workbook.xml.rels",
                r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>"#,
            ),
            (
                "xl/worksheets/sheet1.xml",
                r#"<?xml version="1.0" encoding="UTF-8"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>첫칸</t></is></c><c r="C1"><v>42</v></c></row><row r="1048576"><c r="XFD1048576" t="inlineStr"><is><t>끝칸</t></is></c></row></sheetData></worksheet>"#,
            ),
        ];
        for (name, body) in parts {
            zip.start_file(name, opts).unwrap();
            zip.write_all(body.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }

    /// 종전 추출(조밀 범위 순회)과 같은 행을 내는지 실제 엑셀로 대조 (로컬 전용).
    /// 실행: XLSX_CORPUS=<xlsx 폴더> cargo test xlsx_rows_match_dense -- --ignored --nocapture
    #[test]
    #[ignore = "실 엑셀 코퍼스 필요 (로컬 전용)"]
    fn xlsx_rows_match_dense_extraction() {
        let Ok(dir) = std::env::var("XLSX_CORPUS") else {
            eprintln!("XLSX_CORPUS 미설정 — skip");
            return;
        };
        let mut compared = 0;
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            if ext != "xlsx" && ext != "xls" {
                continue;
            }
            let mut dense_wb = open_workbook_auto(&path).unwrap();
            let mut sparse_wb = open_workbook_auto(&path).unwrap();
            for sheet in dense_wb.sheet_names().to_vec() {
                let Ok(range) = dense_wb.worksheet_range(&sheet) else {
                    continue;
                };
                // 종전 extract_text_with_location 의 행 조립
                let (start_row, _) = range.start().unwrap_or((0, 0));
                let mut dense: Vec<(usize, String)> = Vec::new();
                let mut total = 0usize;
                for (idx, row) in range.rows().enumerate() {
                    if idx >= MAX_ROWS_PER_SHEET {
                        break;
                    }
                    let cells: Vec<String> = row.iter().filter_map(cell_to_string).collect();
                    if !cells.is_empty() {
                        let text = cells.join("\t");
                        total += text.len();
                        if total > MAX_TOTAL_CHARS {
                            break;
                        }
                        dense.push((start_row as usize + idx + 1, text));
                    }
                }
                let sparse = sheet_rows(&mut sparse_wb, &sheet).unwrap();
                assert_eq!(dense, sparse, "{} / {}", path.display(), sheet);
                compared += 1;
            }
        }
        eprintln!("시트 {compared}장 일치");
        assert!(compared > 0);
    }

    /// 멀리 떨어진 셀 하나로 거대 할당이 일어나 프로세스가 죽지 않고, 값은 그대로 뽑힌다.
    #[test]
    fn far_apart_cells_do_not_allocate_the_whole_rectangle() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("far.xlsx");
        write_far_corner_xlsx(&path);
        let doc = parse(&path).expect("파싱 성공");
        assert!(doc.content.contains("첫칸\t42"), "{:?}", doc.content);
        assert!(
            !doc.content.contains("끝칸"),
            "시트 행 상한(5만 행) 밖은 잘린다: {:?}",
            doc.content
        );
        assert_eq!(doc.chunks[0].location_hint.as_deref(), Some("자료!행1"));
    }
}
