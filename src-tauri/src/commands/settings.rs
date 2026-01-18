use crate::error::{ApiError, ApiResult};
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
    #[serde(default)]
    pub view_density: ViewDensity,
    #[serde(default = "default_include_subfolders")]
    pub include_subfolders: bool,
}

fn default_include_subfolders() -> bool {
    true
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ViewDensity {
    #[default]
    Normal,
    Compact,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SearchMode {
    Keyword,
    Semantic,
    Hybrid,
    Filename,
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
            view_density: ViewDensity::Normal,
            include_subfolders: true,
        }
    }
}

/// 설정 파일 경로 가져오기
fn get_settings_path(app_data_dir: &PathBuf) -> PathBuf {
    app_data_dir.join("settings.json")
}

/// 설정 조회
#[tauri::command]
pub async fn get_settings(state: State<'_, Mutex<AppState>>) -> ApiResult<Settings> {
    let app_data_dir = {
        let state = state.lock()?;
        state.db_path.parent().map(|p| p.to_path_buf())
    };

    let Some(app_data_dir) = app_data_dir else {
        return Ok(Settings::default());
    };

    let settings_path = get_settings_path(&app_data_dir);

    if settings_path.exists() {
        let content = fs::read_to_string(&settings_path)
            .map_err(|e| ApiError::SettingsLoad(e.to_string()))?;

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
            .map_err(|e| ApiError::SettingsSave(e.to_string()))?;

        let _ = fs::write(&settings_path, content);

        Ok(settings)
    }
}

/// 설정 동기 조회 (내부 사용)
pub fn get_settings_sync(app_data_dir: &PathBuf) -> Settings {
    let settings_path = get_settings_path(app_data_dir);

    if settings_path.exists() {
        let content = fs::read_to_string(&settings_path).ok();
        content
            .and_then(|c| serde_json::from_str(&c).ok())
            .unwrap_or_default()
    } else {
        Settings::default()
    }
}

/// 설정 업데이트
#[tauri::command]
pub async fn update_settings(
    settings: Settings,
    state: State<'_, Mutex<AppState>>,
) -> ApiResult<()> {
    tracing::info!("Updating settings: {:?}", settings);

    let app_data_dir = {
        let state = state.lock()?;
        state.db_path.parent().map(|p| p.to_path_buf())
    };

    let Some(app_data_dir) = app_data_dir else {
        return Err(ApiError::SettingsSave("앱 데이터 디렉토리를 찾을 수 없습니다".to_string()));
    };

    let settings_path = get_settings_path(&app_data_dir);
    let content = serde_json::to_string_pretty(&settings)
        .map_err(|e| ApiError::SettingsSave(e.to_string()))?;

    fs::write(&settings_path, content)
        .map_err(|e| ApiError::SettingsSave(e.to_string()))?;

    tracing::info!("Settings saved to {:?}", settings_path);
    Ok(())
}
