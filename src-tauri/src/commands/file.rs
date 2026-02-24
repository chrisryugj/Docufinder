use std::path::Path;
use std::process::Command;

use tauri::{AppHandle, Manager};

/// 허용된 파일 확장자 (대소문자 무관)
const ALLOWED_EXTENSIONS: &[&str] = &[
    "pdf", "docx", "doc", "xlsx", "xls", "pptx", "ppt", "hwp", "hwpx", "txt", "md", "rtf", "csv",
    "jpg", "jpeg", "png", "gif", "bmp", "webp",
];

/// 경로 검증 (path traversal 방지)
fn validate_path(path: &str) -> Result<std::path::PathBuf, String> {
    let path = Path::new(path);

    // canonicalize로 경로 정규화 (.. 등 해결)
    let canonical = path
        .canonicalize()
        .map_err(|_| "유효하지 않은 경로입니다".to_string())?;

    Ok(canonical)
}

/// 파일 확장자 검증
fn validate_extension(path: &Path) -> Result<(), String> {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if ALLOWED_EXTENSIONS.contains(&extension.as_str()) {
        Ok(())
    } else {
        Err(format!("지원하지 않는 파일 형식입니다: {}", extension))
    }
}

/// 파일을 기본 앱으로 열기 (페이지 지정 가능)
#[tauri::command]
pub async fn open_file(path: String, page: Option<i64>) -> Result<(), String> {
    // 경로 검증
    let canonical_path = validate_path(&path)?;

    // 파일 존재 확인
    if !canonical_path.is_file() {
        return Err("파일을 찾을 수 없습니다".to_string());
    }

    // 확장자 검증
    validate_extension(&canonical_path)?;

    let path_str = canonical_path.to_string_lossy().to_string();

    #[cfg(target_os = "windows")]
    {
        let extension = canonical_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        // PDF는 SumatraPDF로 페이지 지정 열기 시도
        if let (true, Some(page_num)) = (extension == "pdf", page) {
            // SumatraPDF 경로 확인
            let sumatra_paths = [
                r"C:\Program Files\SumatraPDF\SumatraPDF.exe",
                r"C:\Program Files (x86)\SumatraPDF\SumatraPDF.exe",
            ];

            for sumatra_path in &sumatra_paths {
                if Path::new(sumatra_path).exists() {
                    Command::new(sumatra_path)
                        .args(["-page", &page_num.to_string(), &path_str])
                        .spawn()
                        .map_err(|e| format!("PDF 열기 실패: {}", e))?;
                    return Ok(());
                }
            }

            // SumatraPDF 없으면 기본 앱으로 열기
            tracing::warn!("SumatraPDF not found, opening with default app");
        }

        // 기본 앱으로 열기
        Command::new("explorer")
            .arg(&path_str)
            .spawn()
            .map_err(|e| format!("파일 열기 실패: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        let _ = page; // unused on macOS
        Command::new("open")
            .arg(&path_str)
            .spawn()
            .map_err(|e| format!("파일 열기 실패: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        let _ = page; // unused on Linux
        Command::new("xdg-open")
            .arg(&path_str)
            .spawn()
            .map_err(|e| format!("파일 열기 실패: {}", e))?;
    }

    Ok(())
}

/// 프론트엔드 에러를 Rust 로그 파일에 기록
#[tauri::command]
pub async fn log_frontend_error(
    level: String,
    message: String,
    stack: Option<String>,
    source: Option<String>,
) -> Result<(), String> {
    let source_tag = source.as_deref().unwrap_or("unknown");
    let stack_info = stack
        .as_deref()
        .map(|s| format!("\n  Stack: {}", s))
        .unwrap_or_default();

    match level.as_str() {
        "error" => tracing::error!("[FRONTEND:{}] {}{}", source_tag, message, stack_info),
        "warn" => tracing::warn!("[FRONTEND:{}] {}{}", source_tag, message, stack_info),
        _ => tracing::info!("[FRONTEND:{}] {}{}", source_tag, message, stack_info),
    }
    Ok(())
}

/// 로그 폴더 경로 반환
#[tauri::command]
pub async fn get_log_dir(app_handle: AppHandle) -> Result<String, String> {
    let data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("앱 데이터 경로를 가져올 수 없습니다: {}", e))?;
    let logs_dir = data_dir.join("logs");
    Ok(logs_dir.to_string_lossy().to_string())
}

/// 로그 폴더를 파일 탐색기로 열기
#[tauri::command]
pub async fn open_log_dir(app_handle: AppHandle) -> Result<(), String> {
    let data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("앱 데이터 경로를 가져올 수 없습니다: {}", e))?;
    let logs_dir = data_dir.join("logs");
    let _ = std::fs::create_dir_all(&logs_dir);

    let path_str = logs_dir.to_string_lossy().to_string();

    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(&path_str)
            .spawn()
            .map_err(|e| format!("로그 폴더 열기 실패: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&path_str)
            .spawn()
            .map_err(|e| format!("로그 폴더 열기 실패: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(&path_str)
            .spawn()
            .map_err(|e| format!("로그 폴더 열기 실패: {}", e))?;
    }

    Ok(())
}

/// 폴더를 파일 탐색기로 열기
#[tauri::command]
pub async fn open_folder(path: String) -> Result<(), String> {
    // 경로 검증
    let canonical_path = validate_path(&path)?;

    // 폴더 존재 확인
    if !canonical_path.is_dir() {
        return Err("폴더를 찾을 수 없습니다".to_string());
    }

    let path_str = canonical_path.to_string_lossy().to_string();

    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(&path_str)
            .spawn()
            .map_err(|e| format!("폴더 열기 실패: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&path_str)
            .spawn()
            .map_err(|e| format!("폴더 열기 실패: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(&path_str)
            .spawn()
            .map_err(|e| format!("폴더 열기 실패: {}", e))?;
    }

    Ok(())
}
