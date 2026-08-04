//! Windows COM(Excel.Application) 기반 XLS/XLSX 파서 — Windows 전용.
//!
//! `parsers::xlsx` (Rust ZIP/calamine 파서) 실패 시 `wincom_fallback_xlsx` 에서 호출.
//! 설치된 Excel 로 워크북을 열어 각 시트의 `UsedRange` 에서 셀 값을 추출한다.
//! 레거시 .xls (BIFF) 포맷도 Excel 이 직접 처리하므로 Rust 파서가 지원하지 못하는
//! 포맷의 fallback 역할을 한다.

use std::ffi::c_void;
use std::path::Path;

use windows::Win32::System::Com::SAFEARRAY;
use windows::Win32::System::Ole::{
    SafeArrayGetDim, SafeArrayGetElement, SafeArrayGetLBound, SafeArrayGetUBound,
};
use windows::Win32::System::Variant::{VARIANT, VT_BOOL, VT_BSTR, VT_R8};

use super::{
    to_parse_error, v_is_array, v_is_empty, v_string, v_to_display_string, var_bool, var_i32,
    var_str, AppGuard, ComApartment, Obj,
};
use crate::parsers::{
    DocumentChunk, DocumentMetadata, ParseError, ParsedDocument, DEFAULT_CHUNK_OVERLAP,
    DEFAULT_CHUNK_SIZE,
};

/// 로컬 파일 크기 상한 (100MB) — 전역 `MAX_FILE_SIZE` 와 별도로 운영.
/// .xls(OLE 복합 문서) 포맷은 압축이 없어 OOXML 보다 클 수 있어 여유를 둔다.
const MAX_FILE_SIZE: u64 = 100 * 1024 * 1024;
/// 시트당 최대 추출 행 수 (OOM 방지).
const MAX_ROWS_PER_SHEET: usize = 50_000;
/// 전체 추출 문자 수 상한 (OOM 방지).
const MAX_TOTAL_CHARS: usize = 5_000_000;

/// XLS/XLSX 파일을 COM(Excel.Application) 으로 파싱.
///
/// `parse_with_timeout` 래퍼를 통해 별도 STA 스레드에서 실행되며,
/// 타임아웃·패닉 방어가 적용된다.
pub fn parse(path: &Path) -> Result<ParsedDocument, ParseError> {
    let _bc = crate::breadcrumb::Guard::new(path, "parse_xlsx_com");

    // 파일 크기 체크 (대용량 파일 메모리 보호)
    if let Ok(metadata) = std::fs::metadata(path) {
        if metadata.len() > MAX_FILE_SIZE {
            return Err(ParseError::ParseError(format!(
                "XLS/XLSX 파일 크기 초과: {}MB (최대 {}MB)",
                metadata.len() / 1024 / 1024,
                MAX_FILE_SIZE / 1024 / 1024
            )));
        }
    }

    let _com = ComApartment::init();

    let sheets = extract_sheets(path).map_err(|e| to_parse_error("Excel", &e))?;

    let mut all_text = String::new();
    let mut chunks = Vec::new();
    let mut global_offset = 0;
    let mut sheets_extracted: usize = 0;

    for sheet in &sheets {
        // 전체 문자 수 상한 도달 시 남은 시트 건너뜀 (OOM 방지).
        if all_text.len() > MAX_TOTAL_CHARS {
            tracing::warn!(
                "XLSX(COM) truncated at {} chars (max {}), remaining sheets skipped",
                all_text.len(),
                MAX_TOTAL_CHARS
            );
            break;
        }

        let (sheet_text, sheet_chunks) = build_sheet(&sheet.name, &sheet.rows, global_offset);

        if !sheet_text.is_empty() {
            sheets_extracted += 1;
            if !all_text.is_empty() {
                all_text.push_str("\n\n");
                global_offset += 2;
            }
            // 시트명 헤더 — 시트 경계를 텍스트에 표시.
            let header = format!("[{}]\n", sheet.name);
            all_text.push_str(&header);
            global_offset += header.len();

            all_text.push_str(&sheet_text);
            global_offset += sheet_text.len();

            chunks.extend(sheet_chunks);
        }
    }

    tracing::info!(
        "XLSX(COM) done: {} (sheets extracted: {}, {} chunks, {} chars)",
        path.display(),
        sheets_extracted,
        chunks.len(),
        all_text.len()
    );

    if all_text.is_empty() {
        tracing::warn!("XLSX(COM) file has no text content: {:?}", path);
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

/// 시트별 추출 데이터 (시트명 + (행 번호, 탭 구분 행 텍스트) 목록).
struct SheetData {
    name: String,
    rows: Vec<(usize, String)>,
}

/// Excel 을 COM 자동화로 구동해 각 시트의 `UsedRange` 에서 셀 값을 추출.
///
/// Application → `Workbooks.Open` → `Worksheets` 컬렉션 순회 →
/// 각 시트의 `UsedRange.Value` (2차원 SAFEARRAY) 를 행별 문자열로 변환.
fn extract_sheets(path: &Path) -> windows::core::Result<Vec<SheetData>> {
    let app = Obj::create("Excel.Application")?;
    let _guard = AppGuard::new(&app);
    // Excel 을 백그라운드로 실행 — 경고·이벤트·링크 갱신·매크로 모두 차단.
    let _ = app.put("Visible", var_bool(false));
    let _ = app.put("DisplayAlerts", var_bool(false));
    let _ = app.put("ScreenUpdating", var_bool(false));
    let _ = app.put("EnableEvents", var_bool(false));
    let _ = app.put("AskToUpdateLinks", var_bool(false));
    let _ = app.put(
        "AutomationSecurity",
        var_i32(super::MSO_AUTOMATION_SECURITY_FORCE_DISABLE),
    );

    let workbooks = app.get_obj("Workbooks", &[])?;
    let path_str = super::path_arg(path);
    let wb_var = workbooks.call(
        "Open",
        &[
            var_str(&path_str),             // FileName — 절대 경로
            var_i32(0),                     // UpdateLinks — 외부 링크 갱신 안 함
            var_bool(true),                 // ReadOnly — 읽기 전용
            super::missing_arg(),           // Format — 생략 (기본 형식 자동 감지)
            var_str(super::BOGUS_PASSWORD), // Password — 가짜 암호 (보호 문서 즉시 실패)
        ],
    )?;
    let wb = super::as_obj(&wb_var)?;

    let worksheets = wb.get_obj("Worksheets", &[])?;
    let count = worksheets.get_i32("Count", &[]);

    let mut out: Vec<SheetData> = Vec::new();
    for i in 1..=count {
        let sheet = match worksheets.get_obj("Item", &[var_i32(i)]) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let name = sheet.get_string("Name", &[]);

        // UsedRange — 데이터가 있는 범위만 추출 (빈 시트 제외).
        let used = match sheet.get_obj("UsedRange", &[]) {
            Ok(u) => u,
            Err(_) => continue,
        };
        let first_row = used.get_i32("Row", &[]).max(1) as usize;

        // UsedRange.Value — 2차원 SAFEARRAY (행 × 열). 단일 셀인 경우 스칼라 VARIANT.
        let value = match used.get("Value", &[]) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let rows = read_used_range(&value, first_row);
        out.push(SheetData { name, rows });
    }

    let _ = wb.call("Close", &[var_bool(false)]);

    Ok(out)
}

/// `UsedRange.Value` (SAFEARRAY 또는 스칼라) 를 행별 문자열로 변환.
///
/// 2차원 SAFEARRAY 의 경우 각 셀을 `cell_to_string` 으로 변환하여
/// 탭(`\t`) 구분 행 텍스트를 생성. 단일 셀(스칼라)은 1행으로 처리.
/// `MAX_ROWS_PER_SHEET` · `MAX_TOTAL_CHARS` 상한으로 OOM 을 방지한다.
fn read_used_range(value: &VARIANT, first_row: usize) -> Vec<(usize, String)> {
    let mut rows: Vec<(usize, String)> = Vec::new();
    let mut total_chars = 0usize;

    if v_is_array(value) {
        let psa: *const SAFEARRAY =
            unsafe { value.Anonymous.Anonymous.Anonymous.parray } as *const SAFEARRAY;
        if psa.is_null() {
            return rows;
        }

        // 2차원 배열(행×열)만 처리 — 1차원이면 스킵.
        if unsafe { SafeArrayGetDim(psa) } != 2 {
            return rows;
        }
        let (r_lb, r_ub) = match (unsafe { SafeArrayGetLBound(psa, 1) }, unsafe {
            SafeArrayGetUBound(psa, 1)
        }) {
            (Ok(a), Ok(b)) => (a, b),
            _ => return rows,
        };
        let (c_lb, c_ub) = match (unsafe { SafeArrayGetLBound(psa, 2) }, unsafe {
            SafeArrayGetUBound(psa, 2)
        }) {
            (Ok(a), Ok(b)) => (a, b),
            _ => return rows,
        };
        for (row_idx, r) in (r_lb..=r_ub).enumerate() {
            if row_idx >= MAX_ROWS_PER_SHEET {
                tracing::warn!(
                    "XLSX(COM) sheet truncated at {} rows (max {})",
                    row_idx,
                    MAX_ROWS_PER_SHEET
                );
                break;
            }
            let actual_row = first_row + row_idx;

            let mut cells: Vec<String> = Vec::new();
            for c in c_lb..=c_ub {
                let indices = [r, c];
                let mut elem = VARIANT::default();
                let hr = unsafe {
                    SafeArrayGetElement(
                        psa,
                        indices.as_ptr(),
                        &mut elem as *mut VARIANT as *mut c_void,
                    )
                };
                if hr.is_ok() {
                    if let Some(s) = cell_to_string(&elem) {
                        cells.push(s);
                    }
                }
            }
            if !cells.is_empty() {
                let row_text = cells.join("\t");
                total_chars += row_text.len();
                if total_chars > MAX_TOTAL_CHARS {
                    tracing::warn!(
                        "XLSX(COM) sheet truncated at {} chars (max {})",
                        total_chars,
                        MAX_TOTAL_CHARS
                    );
                    break;
                }
                rows.push((actual_row, row_text));
            }
        }
    } else if !v_is_empty(value) {
        // 단일 셀 — UsedRange 가 1×1 인 경우 Value 가 SAFEARRAY 가 아닌 스칼라.
        if let Some(s) = cell_to_string(value) {
            rows.push((first_row, s));
        }
    }

    rows
}

/// VARIANT 셀 값을 문자열로 변환.
///
/// - `VT_BSTR`: 문자열 (trim 후 빈 문자열이면 `None`)
/// - `VT_BOOL`: `"true"` / `"false"`
/// - `VT_R8`: 숫자 (정수면 `i64`, 소수면 소수점 2자리)
/// - 그 외: `v_to_display_string` 으로 로케일 문자열 변환 (날짜·통화 등)
fn cell_to_string(v: &VARIANT) -> Option<String> {
    if v_is_empty(v) {
        return None;
    }
    let vt = v.vt();
    if vt == VT_BSTR {
        let s = v_string(v);
        let t = s.trim();
        if t.is_empty() {
            None
        } else {
            Some(t.to_string())
        }
    } else if vt == VT_BOOL {
        Some(bool::try_from(v).unwrap_or(false).to_string())
    } else if vt == VT_R8 {
        let f = f64::try_from(v).unwrap_or(0.0);
        Some(format_number(f))
    } else {
        let s = v_to_display_string(v);
        let t = s.trim();
        if t.is_empty() {
            None
        } else {
            Some(t.to_string())
        }
    }
}

/// 숫자 포맷팅 — 정수면 `i64` 로, 소수면 소수점 2자리로 변환.
fn format_number(f: f64) -> String {
    if f.fract() == 0.0 && f.abs() < 1e15 {
        format!("{}", f as i64)
    } else {
        format!("{:.2}", f)
    }
}

/// 시트 데이터를 전체 텍스트와 청크로 분할.
/// 행들을 `\n` 으로 연결해 전체 텍스트를 만들고, 행 경계를 유지하며 청크를 생성한다.
fn build_sheet(
    sheet_name: &str,
    row_infos: &[(usize, String)],
    base_offset: usize,
) -> (String, Vec<DocumentChunk>) {
    let full_text = row_infos
        .iter()
        .map(|(_, t)| t.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let chunks = create_chunks_with_rows(
        row_infos,
        sheet_name,
        base_offset,
        DEFAULT_CHUNK_SIZE,
        DEFAULT_CHUNK_OVERLAP,
    );

    (full_text, chunks)
}

/// 행 경계를 유지하며 청크 분할.
///
/// `chunk_size` 를 초과하지 않도록 행 단위로 청크를 묶되, 행 하나가
/// `chunk_size` 보다 크면 그 행을 단독 청크로 만든다. `overlap` 만큼
/// 이전 청크의 끝 행들을 다음 청크의 시작에 포함시킨다.
/// 각 청크에는 `location_hint` 로 시트명+행 범위를 기록한다.
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

        // chunk_size 를 채울 때까지 행 추가 (단일 행이 chunk_size 초과면 그 행만).
        while end_idx < n {
            let row_size = row_infos[end_idx].1.len() + if end_idx > start_idx { 1 } else { 0 };
            if current_size + row_size > chunk_size && end_idx > start_idx {
                break;
            }
            current_size += row_size;
            end_idx += 1;
        }

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

        // overlap 만큼 이전 행들을 다음 청크에 포함.
        if overlap == 0 {
            start_idx = end_idx;
        } else {
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
            start_idx = new_start.max(start_idx + 1);
        }
    }

    chunks
}

/// 위치 힌트 문자열 생성 (예: `"Sheet1!행3"` 또는 `"Sheet1!행3-10"`).
fn format_location_hint(sheet_name: &str, start_row: usize, end_row: usize) -> String {
    if start_row == end_row {
        format!("{}!행{}", sheet_name, start_row)
    } else {
        format!("{}!행{}-{}", sheet_name, start_row, end_row)
    }
}
