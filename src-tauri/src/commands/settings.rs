use crate::AppState;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::State;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Settings {
    pub search_mode: SearchMode,
    pub max_results: usize,
    pub chunk_size: usize,
    pub chunk_overlap: usize,
    pub theme: Theme,
    #[serde(default)]
    pub min_confidence: u8,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SearchMode {
    Keyword,
    Semantic,
    Hybrid,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    Dark,
    Light,
    System,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            search_mode: SearchMode::Hybrid,
            max_results: 50,
            chunk_size: 512,
            chunk_overlap: 64,
            theme: Theme::Dark,
            min_confidence: 0,
        }
    }
}

/// 설정 파일 경로 가져오기
fn get_settings_path(app_data_dir: &PathBuf) -> PathBuf {
    app_data_dir.join("settings.json")
}

/// 설정 조회
#[tauri::command]
pub async fn get_settings(state: State<'_, Mutex<AppState>>) -> Result<Settings, String> {
    let app_data_dir = {
        let state = state.lock().map_err(|e| e.to_string())?;
        state.db_path.parent().map(|p| p.to_path_buf())
    };

    let Some(app_data_dir) = app_data_dir else {
        return Ok(Settings::default());
    };

    let settings_path = get_settings_path(&app_data_dir);

    if settings_path.exists() {
        let content = fs::read_to_string(&settings_path)
            .map_err(|e| format!("설정 파일 읽기 실패: {}", e))?;

        let settings: Settings = serde_json::from_str(&content)
            .unwrap_or_else(|_| {
                tracing::warn!("Invalid settings file, using defaults");
                Settings::default()
            });

        Ok(settings)
    } else {
        // 기본 설정 생성 및 저장
        let settings = Settings::default();
        let content = serde_json::to_string_pretty(&settings)
            .map_err(|e| format!("설정 직렬화 실패: {}", e))?;

        let _ = fs::write(&settings_path, content);

        Ok(settings)
    }
}

/// 설정 업데이트
#[tauri::command]
pub async fn update_settings(
    settings: Settings,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    tracing::info!("Updating settings: {:?}", settings);

    let app_data_dir = {
        let state = state.lock().map_err(|e| e.to_string())?;
        state.db_path.parent().map(|p| p.to_path_buf())
    };

    let Some(app_data_dir) = app_data_dir else {
        return Err("앱 데이터 디렉토리를 찾을 수 없습니다".to_string());
    };

    let settings_path = get_settings_path(&app_data_dir);
    let content = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("설정 직렬화 실패: {}", e))?;

    fs::write(&settings_path, content)
        .map_err(|e| format!("설정 저장 실패: {}", e))?;

    tracing::info!("Settings saved to {:?}", settings_path);
    Ok(())
}
