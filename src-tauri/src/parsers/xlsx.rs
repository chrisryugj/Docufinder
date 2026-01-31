use super::{DocumentChunk, DocumentMetadata, ParseError, ParsedDocument};
use calamine::{open_workbook_auto, Data, Reader};
use std::path::Path;

/// 최대 XLSX 파일 크기 (100MB)
const MAX_FILE_SIZE: u64 = 100 * 1024 * 1024;

/// XLSX/XLS 파일 파싱
/// calamine 크레이트 사용, 시트/행 정보 포함
pub fn parse(path: &Path) -> Result<ParsedDocument, ParseError> {
    // 파일 크기 제한 (압축 폭탄 방어)
    if let Ok(metadata) = std::fs::metadata(path) {
        if metadata.len() > MAX_FILE_SIZE {
            return Err(ParseError::ParseError(format!(
                "파일 크기 초과: {} bytes (최대 {} bytes)",
                metadata.len(), MAX_FILE_SIZE
            )));
        }
    }

    let mut workbook =
        open_workbook_auto(path).map_err(|e| ParseError::ParseError(e.to_string()))?;

    let mut all_text = String::new();
    let mut chunks = Vec::new();
    let sheet_names = workbook.sheet_names().to_vec();
    let mut global_offset = 0;

    for sheet_name in sheet_names {
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

    for (row_idx, row) in range.rows().enumerate() {
        let actual_row = start_row + row_idx + 1; // 1-based Excel row

        let cells: Vec<String> = row
            .iter()
            .filter_map(|cell| cell_to_string(cell))
            .collect();

        if !cells.is_empty() {
            let row_text = cells.join("\t");
            all_rows_text.push(row_text.clone());
            row_infos.push((actual_row, row_text));
        }
    }

    let full_text = all_rows_text.join("\n");

    // 행 단위로 청크 생성
    let chunks = create_chunks_with_rows(&row_infos, sheet_name, base_offset, super::DEFAULT_CHUNK_SIZE, super::DEFAULT_CHUNK_OVERLAP);

    (full_text, chunks)
}

/// 행 정보를 유지하면서 청크 생성
fn create_chunks_with_rows(
    row_infos: &[(usize, String)],
    sheet_name: &str,
    base_offset: usize,
    chunk_size: usize,
    _overlap: usize,
) -> Vec<DocumentChunk> {
    let mut chunks = Vec::new();

    if row_infos.is_empty() {
        return chunks;
    }

    let mut current_chunk_text = String::new();
    let mut chunk_start_row: Option<usize> = None;
    let mut chunk_end_row = 0;
    let mut current_offset = base_offset;

    for (row_num, row_text) in row_infos {
        let row_with_newline = if current_chunk_text.is_empty() {
            row_text.clone()
        } else {
            format!("\n{}", row_text)
        };

        // 청크 크기 초과 시 새 청크 시작
        if current_chunk_text.len() + row_with_newline.len() > chunk_size
            && !current_chunk_text.is_empty()
        {
            // 현재 청크 저장
            let location = format_location_hint(sheet_name, chunk_start_row.unwrap(), chunk_end_row);
            chunks.push(DocumentChunk {
                content: current_chunk_text.clone(),
                start_offset: current_offset,
                end_offset: current_offset + current_chunk_text.len(),
                page_number: None,
                location_hint: Some(location),
            });

            current_offset += current_chunk_text.len() + 1; // +1 for newline
            current_chunk_text.clear();
            chunk_start_row = None;
        }

        // 행 추가
        if chunk_start_row.is_none() {
            chunk_start_row = Some(*row_num);
        }
        chunk_end_row = *row_num;

        if current_chunk_text.is_empty() {
            current_chunk_text = row_text.clone();
        } else {
            current_chunk_text.push('\n');
            current_chunk_text.push_str(row_text);
        }
    }

    // 마지막 청크 저장
    if !current_chunk_text.is_empty() {
        let location = format_location_hint(sheet_name, chunk_start_row.unwrap(), chunk_end_row);
        chunks.push(DocumentChunk {
            content: current_chunk_text.clone(),
            start_offset: current_offset,
            end_offset: current_offset + current_chunk_text.len(),
            page_number: None,
            location_hint: Some(location),
        });
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
