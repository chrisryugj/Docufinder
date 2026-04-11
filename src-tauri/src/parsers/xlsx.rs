use super::{DocumentChunk, DocumentMetadata, ParseError, ParsedDocument};
use calamine::{open_workbook_auto, Data, Reader};
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

    let mut workbook = open_workbook_auto(path).map_err(|e| {
        let msg = e.to_string().to_lowercase();
        if msg.contains("password") || msg.contains("encrypt") || msg.contains("cfb") {
            ParseError::PasswordProtected("암호로 보호된 엑셀 파일입니다".to_string())
        } else {
            ParseError::ParseError(e.to_string())
        }
    })?;

    let mut all_text = String::new();
    let mut chunks = Vec::new();
    let sheet_names = workbook.sheet_names().to_vec();
    let mut global_offset = 0;

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

        if let Ok(range) = workbook.worksheet_range(&sheet_name) {
            let (sheet_text, sheet_chunks) =
                extract_text_with_location(&range, &sheet_name, global_offset);

            if !sheet_text.is_empty() {
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
    }

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
    })
}

/// 시트에서 텍스트 추출 + 행 정보 포함 청크 생성
fn extract_text_with_location(
    range: &calamine::Range<Data>,
    sheet_name: &str,
    base_offset: usize,
) -> (String, Vec<DocumentChunk>) {
    let mut all_rows_text: Vec<String> = Vec::new();
    let mut row_infos: Vec<(usize, String)> = Vec::new(); // (1-based row, text)

    let (start_row, _) = range.start().unwrap_or((0, 0));
    let start_row = start_row as usize;

    let mut total_chars = 0usize;
    for (row_idx, row) in range.rows().enumerate() {
        if row_idx >= MAX_ROWS_PER_SHEET {
            tracing::warn!("Sheet '{}' truncated at {} rows (max {})", sheet_name, row_idx, MAX_ROWS_PER_SHEET);
            break;
        }

        let actual_row = start_row + row_idx + 1; // 1-based Excel row

        let cells: Vec<String> = row.iter().filter_map(cell_to_string).collect();

        if !cells.is_empty() {
            let row_text = cells.join("\t");
            total_chars += row_text.len();
            if total_chars > MAX_TOTAL_CHARS {
                tracing::warn!("Sheet '{}' truncated at {} chars (max {})", sheet_name, total_chars, MAX_TOTAL_CHARS);
                break;
            }
            all_rows_text.push(row_text.clone());
            row_infos.push((actual_row, row_text));
        }
    }

    let full_text = all_rows_text.join("\n");

    // 행 단위로 청크 생성
    let chunks = create_chunks_with_rows(
        &row_infos,
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
