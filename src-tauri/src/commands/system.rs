//! System Commands - 시스템 정보 조회

use crate::error::ApiResult;
use serde::Serialize;

/// 추천 폴더 정보
#[derive(Debug, Serialize)]
pub struct SuggestedFolder {
    pub path: String,
    pub label: String,
    /// "known" (바탕화면, 문서 등) | "drive" (드라이브)
    pub category: String,
    pub exists: bool,
}

/// 추천 폴더 목록 반환 (바탕화면, 문서, 다운로드 + 마운트된 드라이브)
#[tauri::command]
pub async fn get_suggested_folders() -> ApiResult<Vec<SuggestedFolder>> {
    let mut suggestions = Vec::new();

    // 1. Known Folders (dirs 크레이트)
    if let Some(p) = dirs::desktop_dir() {
        if p.exists() {
            suggestions.push(SuggestedFolder {
                path: p.to_string_lossy().to_string(),
                label: "바탕화면".into(),
                category: "known".into(),
                exists: true,
            });
        }
    }
    if let Some(p) = dirs::document_dir() {
        if p.exists() {
            suggestions.push(SuggestedFolder {
                path: p.to_string_lossy().to_string(),
                label: "문서".into(),
                category: "known".into(),
                exists: true,
            });
        }
    }
    if let Some(p) = dirs::download_dir() {
        if p.exists() {
            suggestions.push(SuggestedFolder {
                path: p.to_string_lossy().to_string(),
                label: "다운로드".into(),
                category: "known".into(),
                exists: true,
            });
        }
    }

    // 2. 마운트된 드라이브 목록 (Windows)
    #[cfg(windows)]
    {
        for letter in b'A'..=b'Z' {
            let drive = format!("{}:\\", letter as char);
            let path = std::path::Path::new(&drive);
            if path.exists() {
                suggestions.push(SuggestedFolder {
                    path: drive,
                    label: format!("{}: 드라이브", letter as char),
                    category: "drive".into(),
                    exists: true,
                });
            }
        }
    }

    Ok(suggestions)
}
