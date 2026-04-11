use crate::{ApiError, ApiResult};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ExportRow {
    pub file_name: String,
    pub file_path: String,
    pub location_hint: String,
    pub content_preview: String,
    pub score: f64,
    #[allow(dead_code)]
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
    if v.starts_with('=')
        || v.starts_with('+')
        || v.starts_with('-')
        || v.starts_with('@')
        || v.starts_with('\t')
        || v.starts_with('\r')
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
                row.content_preview
                    .chars()
                    .take(500)
                    .collect::<String>()
                    .replace('\n', " ")
                    + "..."
            } else {
                row.content_preview.replace('\n', " ")
            };
            lines.push(format!(
                "{},{},{},{},{:.2}",
                escape_csv(&row.file_name),
                escape_csv(&row.file_path),
                escape_csv(&row.location_hint),
                escape_csv(&preview),
                row.score,
            ));
        }

        std::fs::write(&output_path, lines.join("\n"))
            .map_err(|e| ApiError::IndexingFailed(format!("CSV 저장 실패: {}", e)))?;

        Ok(output_path)
    })
    .await
    .map_err(|e| ApiError::IndexingFailed(e.to_string()))?
}
