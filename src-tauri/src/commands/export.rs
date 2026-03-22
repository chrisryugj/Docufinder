use crate::{ApiError, ApiResult};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct ExportRow {
    pub file_name: String,
    pub file_path: String,
    pub location_hint: String,
    pub content_preview: String,
    pub score: f64,
    pub modified_at: Option<i64>,
}

/// 출력 경로 기본 검증 (시스템 폴더 차단)
fn validate_output_path(output_path: &str) -> ApiResult<()> {
    let path_lower = output_path.to_lowercase().replace('\\', "/");
    for pattern in crate::constants::BLOCKED_PATH_PATTERNS {
        if path_lower.contains(&pattern.to_lowercase()) {
            return Err(ApiError::AccessDenied(
                "시스템 폴더에는 저장할 수 없습니다".to_string(),
            ));
        }
    }
    Ok(())
}

/// CSV 이스케이프 (수식 주입 방어 포함)
fn escape_csv(value: &str) -> String {
    let mut v = value.to_string();
    if v.starts_with('=') || v.starts_with('+') || v.starts_with('-') || v.starts_with('@')
        || v.starts_with('\t') || v.starts_with('\r')
    {
        v = format!("'{}", v);
    }
    if v.contains(',') || v.contains('"') || v.contains('\n') {
        format!("\"{}\"", v.replace('"', "\"\""))
    } else {
        v
    }
}

/// 검색 결과를 CSV로 내보내기
#[tauri::command]
pub async fn export_csv(
    rows: Vec<ExportRow>,
    query: String,
    output_path: String,
) -> ApiResult<String> {
    validate_output_path(&output_path)?;
    tokio::task::spawn_blocking(move || {
        let _ = &query; // 향후 메타데이터용
        let bom = "\u{FEFF}";
        let header = "파일명,경로,위치,매칭내용,점수";

        let mut lines = Vec::with_capacity(rows.len() + 2);
        lines.push(format!("{}{}", bom, header));

        for row in &rows {
            let preview = if row.content_preview.len() > 500 {
                row.content_preview.chars().take(500).collect::<String>().replace('\n', " ") + "..."
            } else {
                row.content_preview.replace('\n', " ")
            };
            lines.push(format!(
                "{},{},{},{},{}",
                escape_csv(&row.file_name),
                escape_csv(&row.file_path),
                escape_csv(&row.location_hint),
                escape_csv(&preview),
                format!("{:.2}", row.score),
            ));
        }

        std::fs::write(&output_path, lines.join("\n"))
            .map_err(|e| ApiError::IndexingFailed(format!("CSV 저장 실패: {}", e)))?;

        Ok(output_path)
    })
    .await
    .map_err(|e| ApiError::IndexingFailed(e.to_string()))?
}

/// 검색 결과를 XLSX로 내보내기
#[tauri::command]
pub async fn export_xlsx(
    rows: Vec<ExportRow>,
    query: String,
    output_path: String,
) -> ApiResult<String> {
    use rust_xlsxwriter::*;

    validate_output_path(&output_path)?;
    tokio::task::spawn_blocking(move || {
        let mut workbook = Workbook::new();
        let sheet = workbook.add_worksheet();
        sheet
            .set_name("검색 결과")
            .map_err(|e| ApiError::IndexingFailed(e.to_string()))?;

        // 헤더 스타일
        let header_fmt = Format::new()
            .set_bold()
            .set_background_color(Color::RGB(0x4472C4))
            .set_font_color(Color::White)
            .set_border(FormatBorder::Thin);

        // 데이터 스타일
        let data_fmt = Format::new().set_border(FormatBorder::Thin);
        let score_fmt = Format::new()
            .set_border(FormatBorder::Thin)
            .set_num_format("0.00");
        let date_fmt = Format::new()
            .set_border(FormatBorder::Thin)
            .set_num_format("yyyy-mm-dd hh:mm");

        // 메타 정보
        sheet
            .write_string(0, 0, &format!("검색어: \"{}\"", query))
            .ok();
        sheet
            .write_string(0, 2, &format!("결과: {}건", rows.len()))
            .ok();
        sheet
            .write_string(
                0,
                4,
                &format!(
                    "내보내기: {}",
                    chrono::Local::now().format("%Y-%m-%d %H:%M")
                ),
            )
            .ok();

        // 헤더 (2행)
        let headers = ["파일명", "경로", "위치", "매칭 내용", "점수", "수정일"];
        for (col, h) in headers.iter().enumerate() {
            sheet
                .write_string_with_format(1, col as u16, *h, &header_fmt)
                .ok();
        }

        // 데이터
        for (i, row) in rows.iter().enumerate() {
            let r = (i + 2) as u32;
            sheet
                .write_string_with_format(r, 0, &row.file_name, &data_fmt)
                .ok();
            sheet
                .write_string_with_format(r, 1, &row.file_path, &data_fmt)
                .ok();
            sheet
                .write_string_with_format(r, 2, &row.location_hint, &data_fmt)
                .ok();
            // 내용: 500자 제한
            let preview = if row.content_preview.len() > 500 {
                format!("{}...", &row.content_preview.chars().take(500).collect::<String>())
            } else {
                row.content_preview.clone()
            };
            sheet
                .write_string_with_format(r, 3, &preview, &data_fmt)
                .ok();
            sheet
                .write_number_with_format(r, 4, row.score, &score_fmt)
                .ok();

            if let Some(ts) = row.modified_at {
                if let Some(dt) = chrono::DateTime::from_timestamp(ts, 0) {
                    let local = dt.with_timezone(&chrono::Local);
                    let excel_date = date_to_excel_serial(local);
                    sheet
                        .write_number_with_format(r, 5, excel_date, &date_fmt)
                        .ok();
                }
            }
        }

        // 열 너비 설정
        sheet.set_column_width(0, 30).ok(); // 파일명
        sheet.set_column_width(1, 50).ok(); // 경로
        sheet.set_column_width(2, 15).ok(); // 위치
        sheet.set_column_width(3, 60).ok(); // 매칭 내용
        sheet.set_column_width(4, 8).ok(); // 점수
        sheet.set_column_width(5, 18).ok(); // 수정일

        // 자동 필터
        let last_row = rows.len() as u32 + 1;
        sheet.autofilter(1, 0, last_row, 5).ok();

        // 틀 고정 (헤더 행)
        sheet.set_freeze_panes(2, 0).ok();

        workbook
            .save(&output_path)
            .map_err(|e| ApiError::IndexingFailed(format!("XLSX 저장 실패: {}", e)))?;

        Ok(output_path)
    })
    .await
    .map_err(|e| ApiError::IndexingFailed(e.to_string()))?
}

/// 선택된 파일들을 ZIP으로 패키징
#[tauri::command]
pub async fn package_zip(file_paths: Vec<String>, output_path: String) -> ApiResult<u32> {
    validate_output_path(&output_path)?;
    tokio::task::spawn_blocking(move || {
        let out_file = std::fs::File::create(&output_path)
            .map_err(|e| ApiError::IndexingFailed(format!("ZIP 파일 생성 실패: {}", e)))?;

        let mut zip = zip::ZipWriter::new(out_file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        let mut count = 0u32;
        let mut seen_names: std::collections::HashMap<String, u32> = std::collections::HashMap::new();

        for path_str in &file_paths {
            let path = Path::new(path_str);

            // 경로 정규화 + 존재/파일 확인 (path traversal, symlink 방지)
            let canonical = match path.canonicalize() {
                Ok(p) if p.is_file() => p,
                _ => continue,
            };

            // 시스템 폴더 접근 차단
            let path_lower = canonical.to_string_lossy().to_lowercase();
            let blocked = crate::constants::BLOCKED_PATH_PATTERNS.iter()
                .any(|pat| path_lower.contains(&pat.to_lowercase()));
            if blocked {
                tracing::warn!("Blocked path in ZIP: {}", path_str);
                continue;
            }

            // 파일명 중복 처리: report.pdf → report (2).pdf
            let original_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");

            let entry_name = {
                let counter = seen_names.entry(original_name.to_string()).or_insert(0);
                *counter += 1;
                if *counter == 1 {
                    original_name.to_string()
                } else {
                    let stem = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown");
                    let ext = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|e| format!(".{}", e))
                        .unwrap_or_default();
                    format!("{} ({}){}", stem, counter, ext)
                }
            };

            // 파일 크기 먼저 확인 (OOM 방지)
            match std::fs::metadata(path).map(|m| m.len()) {
                Ok(size) if size > 500 * 1024 * 1024 => {
                    tracing::warn!("Skipping large file (>500MB, {}bytes): {}", size, path_str);
                    continue;
                }
                Err(e) => {
                    tracing::warn!("Failed to stat file {}: {}", path_str, e);
                    continue;
                }
                _ => {}
            }

            match std::fs::read(path) {
                Ok(data) => {
                    if zip.start_file(&entry_name, options).is_ok() {
                        use std::io::Write;
                        if zip.write_all(&data).is_ok() {
                            count += 1;
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to read file {}: {}", path_str, e);
                }
            }
        }

        zip.finish()
            .map_err(|e| ApiError::IndexingFailed(format!("ZIP 완료 실패: {}", e)))?;

        Ok(count)
    })
    .await
    .map_err(|e| ApiError::IndexingFailed(e.to_string()))?
}

/// chrono DateTime → Excel serial date
fn date_to_excel_serial(dt: chrono::DateTime<chrono::Local>) -> f64 {
    use chrono::{Datelike, Timelike};
    // Excel epoch: 1899-12-30
    let days = dt.num_days_from_ce() - chrono::NaiveDate::from_ymd_opt(1899, 12, 30).unwrap().num_days_from_ce();
    let time_fraction =
        (dt.hour() as f64 * 3600.0 + dt.minute() as f64 * 60.0 + dt.second() as f64) / 86400.0;
    days as f64 + time_fraction
}
