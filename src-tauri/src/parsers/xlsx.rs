use super::{chunk_text, DocumentMetadata, ParseError, ParsedDocument};
use calamine::{open_workbook_auto, DataType, Reader};
use std::path::Path;

/// XLSX/XLS 파일 파싱
/// calamine 크레이트 사용
pub fn parse(path: &Path) -> Result<ParsedDocument, ParseError> {
    let mut workbook =
        open_workbook_auto(path).map_err(|e| ParseError::ParseError(e.to_string()))?;

    let mut all_text = String::new();
    let sheet_names = workbook.sheet_names().to_vec();

    for sheet_name in sheet_names {
        if let Ok(range) = workbook.worksheet_range(&sheet_name) {
            let sheet_text = extract_text_from_sheet(&range);
            if !sheet_text.is_empty() {
                if !all_text.is_empty() {
                    all_text.push_str("\n\n");
                }
                // 시트 이름 추가
                all_text.push_str(&format!("[{}]\n", sheet_name));
                all_text.push_str(&sheet_text);
            }
        }
    }

    if all_text.is_empty() {
        tracing::warn!("XLSX file has no text content: {:?}", path);
    }

    // 청크 분할
    let chunks = chunk_text(&all_text, 512, 64);

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

/// 시트에서 텍스트 추출
fn extract_text_from_sheet(range: &calamine::Range<DataType>) -> String {
    let mut rows_text: Vec<String> = Vec::new();

    for row in range.rows() {
        let cells: Vec<String> = row
            .iter()
            .filter_map(|cell| {
                match cell {
                    DataType::Empty => None,
                    DataType::String(s) => {
                        let trimmed = s.trim();
                        if trimmed.is_empty() {
                            None
                        } else {
                            Some(trimmed.to_string())
                        }
                    }
                    DataType::Int(i) => Some(i.to_string()),
                    DataType::Float(f) => Some(format!("{:.2}", f)),
                    DataType::Bool(b) => Some(b.to_string()),
                    DataType::DateTime(dt) => Some(format!("{:.0}", dt)),
                    DataType::Duration(d) => Some(format!("{:.2}", d)),
                    DataType::DateTimeIso(s) => Some(s.to_string()),
                    DataType::DurationIso(s) => Some(s.to_string()),
                    DataType::Error(e) => {
                        tracing::debug!("Cell error: {:?}", e);
                        None
                    }
                }
            })
            .collect();

        if !cells.is_empty() {
            rows_text.push(cells.join("\t"));
        }
    }

    rows_text.join("\n")
}
