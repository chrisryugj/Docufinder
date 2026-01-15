use std::path::Path;
use std::process::Command;

/// 파일을 기본 앱으로 열기 (페이지 지정 가능)
#[tauri::command]
pub async fn open_file(path: String, page: Option<i64>) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let file_path = Path::new(&path);
        let extension = file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        // PDF는 SumatraPDF로 페이지 지정 열기 시도
        if extension == "pdf" && page.is_some() {
            let page_num = page.unwrap();

            // SumatraPDF 경로 확인
            let sumatra_paths = [
                r"C:\Program Files\SumatraPDF\SumatraPDF.exe",
                r"C:\Program Files (x86)\SumatraPDF\SumatraPDF.exe",
            ];

            for sumatra_path in &sumatra_paths {
                if Path::new(sumatra_path).exists() {
                    Command::new(sumatra_path)
                        .args(["-page", &page_num.to_string(), &path])
                        .spawn()
                        .map_err(|e| format!("Failed to open PDF: {}", e))?;
                    return Ok(());
                }
            }

            // SumatraPDF 없으면 기본 앱으로 열기
            tracing::warn!("SumatraPDF not found, opening with default app");
        }

        // 기본 앱으로 열기
        Command::new("explorer")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        let _ = page; // unused on macOS
        Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        let _ = page; // unused on Linux
        Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
    }

    Ok(())
}
