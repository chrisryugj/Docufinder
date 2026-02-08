use crate::error::{ApiError, ApiResult};
use crate::AppContainer;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, State};
use tauri_plugin_autostart::ManagerExt;

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
    #[serde(default)]
    pub auto_start: bool,
    #[serde(default)]
    pub start_minimized: bool,
    /// 파일명 하이라이트 색상 (hex)
    #[serde(default)]
    pub highlight_filename_color: Option<String>,
    /// 문서 내용 하이라이트 색상 (hex)
    #[serde(default)]
    pub highlight_content_color: Option<String>,
    /// 시맨틱 검색 활성화 여부
    #[serde(default)]
    pub semantic_search_enabled: bool,
    /// 벡터 인덱싱 모드 (manual / auto)
    #[serde(default)]
    pub vector_indexing_mode: VectorIndexingMode,
    /// 인덱싱 강도 (fast / balanced / background)
    #[serde(default)]
    pub indexing_intensity: IndexingIntensity,
    /// 단일 파일 최대 크기 (MB). 초과 시 스킵
    #[serde(default = "default_max_file_size_mb")]
    pub max_file_size_mb: u64,
}

fn default_include_subfolders() -> bool {
    true
}

fn default_max_file_size_mb() -> u64 {
    200
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum VectorIndexingMode {
    #[default]
    Manual,
    Auto,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum IndexingIntensity {
    Fast,
    #[default]
    Balanced,
    Background,
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
            search_mode: SearchMode::Keyword,
            max_results: 50,
            chunk_size: 512,
            chunk_overlap: 64,
            theme: Theme::Dark,
            min_confidence: 0,
            view_density: ViewDensity::Normal,
            include_subfolders: true,
            auto_start: false,
            start_minimized: false,
            highlight_filename_color: None,
            highlight_content_color: None,
            semantic_search_enabled: false,
            vector_indexing_mode: VectorIndexingMode::Manual,
            indexing_intensity: IndexingIntensity::Balanced,
            max_file_size_mb: 200,
        }
    }
}

/// 설정 파일 경로 가져오기
fn get_settings_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("settings.json")
}

/// 설정 조회 (캐시에서 읽기, 디스크 I/O 없음)
#[tauri::command]
pub async fn get_settings(state: State<'_, Mutex<AppContainer>>) -> ApiResult<Settings> {
    let container = state.lock()?;
    Ok(container.get_settings())
}

/// 설정 동기 조회 (내부 사용)
pub fn get_settings_sync(app_data_dir: &Path) -> Settings {
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
    app: AppHandle,
    settings: Settings,
    state: State<'_, Mutex<AppContainer>>,
) -> ApiResult<()> {
    tracing::info!("Updating settings: {:?}", settings);

    let app_data_dir = {
        let state = state.lock()?;
        state.db_path.parent().map(|p| p.to_path_buf())
    };

    let Some(app_data_dir) = app_data_dir else {
        return Err(ApiError::SettingsSave("앱 데이터 디렉토리를 찾을 수 없습니다".to_string()));
    };

    // 자동 시작 설정 변경
    let autostart_manager = app.autolaunch();
    if settings.auto_start {
        if let Err(e) = autostart_manager.enable() {
            tracing::warn!("Failed to enable autostart: {}", e);
        }
    } else if let Err(e) = autostart_manager.disable() {
        tracing::warn!("Failed to disable autostart: {}", e);
    }

    let settings_path = get_settings_path(&app_data_dir);
    let content = serde_json::to_string_pretty(&settings)
        .map_err(|e| ApiError::SettingsSave(e.to_string()))?;

    fs::write(&settings_path, content)
        .map_err(|e| ApiError::SettingsSave(e.to_string()))?;

    // 인메모리 캐시 갱신
    {
        let container = state.lock()?;
        container.update_settings_cache(settings);
    }

    tracing::info!("Settings saved to {:?}", settings_path);
    Ok(())
}

