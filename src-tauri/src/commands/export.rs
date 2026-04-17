use crate::{ApiError, ApiResult};
use serde::Deserialize;
use std::io::Write;
use std::path::{Path, PathBuf};

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

/// 출력 경로 검증 — 시스템 폴더 차단, 확장자 강제, 부모 canonicalize, 덮어쓰기 차단
fn validate_output_path_ext(output_path: &str, expected_ext: &str) -> ApiResult<PathBuf> {
    let path = Path::new(output_path);

    // 1. 확장자 강제
    let has_ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case(expected_ext))
        .unwrap_or(false);
    if !has_ext {
        return Err(ApiError::InvalidPath(format!(
            ".{} 파일로만 저장할 수 있습니다",
            expected_ext
        )));
    }

    // 2. 파일명 분리
    let parent = path
        .parent()
        .ok_or_else(|| ApiError::InvalidPath("저장 경로에 부모 디렉터리가 없습니다".to_string()))?;
    let file_name = path
        .file_name()
        .ok_or_else(|| ApiError::InvalidPath("저장 경로에 파일명이 없습니다".to_string()))?;

    // 파일명에 경로 구분자나 상위 이동 금지 (화이트리스트 느낌)
    let fname_str = file_name.to_string_lossy();
    if fname_str.contains('/') || fname_str.contains('\\') || fname_str == ".." {
        return Err(ApiError::InvalidPath(
            "저장 파일명이 올바르지 않습니다".to_string(),
        ));
    }

    // 3. 부모 디렉터리 canonicalize (실존 + 심볼릭 링크 해소)
    // parent가 빈 문자열이면 현재 작업 디렉터리로 해석 → 명시적 거부
    if parent.as_os_str().is_empty() {
        return Err(ApiError::InvalidPath(
            "절대 경로를 사용해 주세요".to_string(),
        ));
    }
    let canonical_parent = std::fs::canonicalize(parent).map_err(|_| {
        ApiError::InvalidPath(format!(
            "저장 경로의 상위 폴더를 찾을 수 없습니다: {}",
            parent.display()
        ))
    })?;

    // 4. canonical 전체 경로로 BLOCKED 체크 (슬래시/백슬래시 정규화 후 비교)
    let canonical_str = canonical_parent.to_string_lossy();
    let norm = canonical_str.to_lowercase().replace('\\', "/");
    for pattern in crate::constants::BLOCKED_PATH_PATTERNS {
        // 패턴도 동일 정규화 (BLOCKED는 Windows/Unix 양식 혼재 → /로 통일)
        let pat_norm = pattern.to_lowercase().replace('\\', "/");
        if norm.contains(&pat_norm) {
            return Err(ApiError::AccessDenied(
                "시스템 폴더에는 저장할 수 없습니다".to_string(),
            ));
        }
    }

    // 5. 최종 경로 조립
    let final_path = canonical_parent.join(file_name);

    // 6. 사전 exists 체크 (빠른 실패 경로) — 실제 원자 덮어쓰기 방지는
    //    write 시점에 OpenOptions::create_new로 재확인 (TOCTOU 방어)
    if final_path.exists() {
        return Err(ApiError::AccessDenied(format!(
            "이미 같은 이름의 파일이 존재합니다: {}",
            final_path.display()
        )));
    }

    Ok(final_path)
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
    let final_path = validate_output_path_ext(&output_path, "csv")?;
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

        // create_new(true): 파일이 이미 존재하면 AlreadyExists 에러 (원자적 덮어쓰기 방지)
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&final_path)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::AlreadyExists {
                    ApiError::AccessDenied(format!(
                        "이미 같은 이름의 파일이 존재합니다: {}",
                        final_path.display()
                    ))
                } else {
                    ApiError::IndexingFailed(format!("CSV 저장 실패: {}", e))
                }
            })?;
        file.write_all(lines.join("\n").as_bytes())
            .map_err(|e| ApiError::IndexingFailed(format!("CSV 저장 실패: {}", e)))?;

        Ok(final_path.to_string_lossy().into_owned())
    })
    .await
    .map_err(|e| ApiError::IndexingFailed(e.to_string()))?
}

/// 프리뷰 마크다운을 .md 파일로 저장
#[tauri::command]
pub async fn export_markdown(content: String, output_path: String) -> ApiResult<String> {
    const MAX_CONTENT_SIZE: usize = 50 * 1024 * 1024; // 50MB 상한
    if content.len() > MAX_CONTENT_SIZE {
        return Err(ApiError::Validation(
            "저장할 내용이 너무 큽니다 (최대 50MB)".to_string(),
        ));
    }

    let final_path = validate_output_path_ext(&output_path, "md")?;
    tokio::task::spawn_blocking(move || {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&final_path)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::AlreadyExists {
                    ApiError::AccessDenied(format!(
                        "이미 같은 이름의 파일이 존재합니다: {}",
                        final_path.display()
                    ))
                } else {
                    ApiError::IndexingFailed(format!("Markdown 저장 실패: {}", e))
                }
            })?;
        file.write_all(content.as_bytes())
            .map_err(|e| ApiError::IndexingFailed(format!("Markdown 저장 실패: {}", e)))?;
        Ok(final_path.to_string_lossy().into_owned())
    })
    .await
    .map_err(|e| ApiError::IndexingFailed(e.to_string()))?
}
