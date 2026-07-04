use crate::constants::{DEFAULT_MAX_FILE_SIZE_MB, MAX_FILE_SIZE_LIMIT_MB};
use crate::error::{ApiError, ApiResult};
use crate::model_downloader;
use crate::AppContainer;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use tauri::{AppHandle, Emitter, Manager, State};
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
    /// X 버튼 클릭 시 트레이로 숨김 (false면 앱 종료)
    #[serde(default)]
    pub close_to_tray: bool,
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
    /// 검색 결과 더 보기 단위 (한 번에 표시할 개수)
    #[serde(default = "default_results_per_page")]
    pub results_per_page: usize,
    /// 데이터 저장 경로 (DB, 벡터 인덱스)
    /// None이면 기본 AppData 사용. 변경 시 앱 재시작 필요.
    #[serde(default)]
    pub data_root: Option<String>,
    /// 사용자 커스텀 제외 디렉토리 목록 (DEFAULT_EXCLUDED_DIRS에 추가됨)
    #[serde(default)]
    pub exclude_dirs: Vec<String>,
    /// AI 기능 활성화
    #[serde(default)]
    pub ai_enabled: bool,
    /// LLM provider — Gemini (Google 공식) / OpenAI 호환 (사내·오프라인 LLM)
    #[serde(default)]
    pub ai_provider: AiProvider,
    /// OpenAI 호환 endpoint base URL. `ai_provider = OpenAi` 일 때만 사용.
    /// 예: `http://192.168.1.50:8000` (vLLM), `http://localhost:11434` (Ollama),
    ///     `https://api.together.xyz` 등. `/v1/chat/completions` 가 호출됨.
    #[serde(default)]
    pub ai_base_url: Option<String>,
    /// API 키 (provider 공통)
    #[serde(default)]
    pub ai_api_key: Option<String>,
    /// AI 모델 ID — provider 별 (Gemini: gemini-3.1-flash-lite,
    /// OpenAI 호환: qwen3-35b 등 사용자 입력)
    #[serde(default = "default_ai_model")]
    pub ai_model: String,
    /// AI 응답 온도 (0.0-2.0)
    #[serde(default = "default_ai_temperature")]
    pub ai_temperature: f32,
    /// AI 최대 토큰 수
    #[serde(default = "default_ai_max_tokens")]
    pub ai_max_tokens: u32,
    /// OCR 기능 활성화 (이미지 파일 텍스트 인식)
    #[serde(default)]
    pub ocr_enabled: bool,
    /// 스캔/이미지 OCR 레이아웃 분석 (PP-DocLayout, 실험적, 기본 off). 켜면 layout.onnx
    /// (~22MB)를 내려받아 OCR 텍스트 영역을 제목·표·머리글/바닥글로 분류하고, 본문에서
    /// 머리글/바닥글/페이지번호를 제외한다. off 면 모델을 받지 않아 기존 동작과 완전히 동일.
    #[serde(default)]
    pub ocr_layout_enabled: bool,
    /// 검색 결과에서 같은 문서의 여러 버전을 대표 1개로 접기 (Document Lineage Graph)
    #[serde(default = "default_group_versions")]
    pub group_versions: bool,
    /// 자동 동기화 주기 (분). 0 = 끄기, 기본 10분.
    /// 주기 sync 는 watcher 이벤트 누락(전체 드라이브 감시 시 흔함)을 보완.
    #[serde(default = "default_auto_sync_interval_minutes")]
    pub auto_sync_interval_minutes: u32,
    /// 오류 발생 시 개발자에게 자동 리포트 전송 (Telegram Bot).
    /// 파일 경로는 익명화, 문서 내용/검색어는 전송하지 않음.
    #[serde(default = "default_error_reporting_enabled")]
    pub error_reporting_enabled: bool,

    /// PDF 수식 OCR 활성화 (기본 false). kordoc CLI 에 `--formula-ocr` 전달.
    /// 첫 사용 시 Pix2Text MFD + MFR ONNX 모델(~155MB)이 HuggingFace 에서 자동 다운로드됨.
    #[serde(default)]
    pub formula_ocr_enabled: bool,

    /// 클라우드/네트워크 폴더(OneDrive·구글·NAVER Works·UNC·매핑 SMB 등)의 본문 인덱싱 자동 스킵.
    /// true(기본): 메타데이터만 인덱싱 → 파일명 검색은 가능, hydrate/네트워크 다운로드 차단.
    /// false: 일반 로컬 폴더와 동일하게 본문까지 인덱싱 (NAS 등 빠른 환경에서 사용자 선택).
    #[serde(default = "default_skip_cloud_body_indexing")]
    pub skip_cloud_body_indexing: bool,

    /// 시스템 보호 폴더(C:\Windows, /System, /usr/bin 등) 수동 추가 허용.
    /// false(기본): `validate_watch_path`가 차단 — 일반 사용자 보호.
    /// true: 사용자가 폴더 다이얼로그에서 시스템 폴더를 직접 골라 인덱싱 가능. 강한 경고 후 진행.
    /// 시스템 폴더는 파일 수가 많고 본문이 의미 없는(바이너리) 경우가 많아, 활성화돼도 자동 벡터 인덱싱은 스킵.
    #[serde(default)]
    pub allow_system_folders: bool,
}

fn default_skip_cloud_body_indexing() -> bool {
    true
}

fn default_group_versions() -> bool {
    true
}

fn default_auto_sync_interval_minutes() -> u32 {
    10
}

fn default_include_subfolders() -> bool {
    true
}

fn default_max_file_size_mb() -> u64 {
    DEFAULT_MAX_FILE_SIZE_MB
}

fn default_results_per_page() -> usize {
    50
}

fn default_ai_model() -> String {
    "gemini-3.1-flash-lite".to_string()
}

fn default_ai_temperature() -> f32 {
    0.2
}

fn default_ai_max_tokens() -> u32 {
    2048
}

fn default_error_reporting_enabled() -> bool {
    // 프라이버시 기본값: opt-out → opt-in 전환 (prod review 2026-05-17).
    // 사용자가 설정에서 명시적으로 켜기 전까지 외부 전송 없음.
    false
}

/// LLM provider 선택.
///
/// `Gemini` (기본): Google 공식 Generative Language API.
/// `OpenAi`: OpenAI Chat Completions 호환 endpoint (vLLM·Ollama·LiteLLM·사내 LLM 등).
///         `ai_base_url` 필수.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AiProvider {
    #[default]
    Gemini,
    OpenAi,
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
            close_to_tray: false,
            highlight_filename_color: None,
            highlight_content_color: None,
            semantic_search_enabled: false,
            vector_indexing_mode: VectorIndexingMode::Manual,
            indexing_intensity: IndexingIntensity::Balanced,
            max_file_size_mb: DEFAULT_MAX_FILE_SIZE_MB,
            results_per_page: 50,
            data_root: None,
            exclude_dirs: Vec::new(),
            ai_enabled: false,
            ai_provider: AiProvider::default(),
            ai_base_url: None,
            ai_api_key: None,
            ai_model: default_ai_model(),
            ai_temperature: default_ai_temperature(),
            ai_max_tokens: default_ai_max_tokens(),
            ocr_enabled: false,
            ocr_layout_enabled: false,
            group_versions: true,
            auto_sync_interval_minutes: default_auto_sync_interval_minutes(),
            error_reporting_enabled: default_error_reporting_enabled(),
            formula_ocr_enabled: false,
            skip_cloud_body_indexing: default_skip_cloud_body_indexing(),
            allow_system_folders: false,
        }
    }
}

/// 설정 파일 경로 가져오기
fn get_settings_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("settings.json")
}

/// API 키 전용 파일 경로 (settings.json과 분리)
fn get_credentials_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("credentials.json")
}

#[derive(Serialize, Deserialize, Default)]
struct Credentials {
    #[serde(default)]
    ai_api_key: Option<String>,
}

/// credentials.json에서 API 키 로드
fn load_api_key(app_data_dir: &Path) -> Option<String> {
    let path = get_credentials_path(app_data_dir);
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str::<Credentials>(&s).ok())
        .and_then(|c| c.ai_api_key)
}

/// API 키 마스킹: "***" + 마지막 4자리 (프론트엔드 메모리에 평문 잔류 방지)
///
/// 짧은 키(4자 이하)는 단순히 "***"로 마스킹한다.
fn mask_api_key(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    let n = chars.len();
    if n <= 4 {
        return "***".to_string();
    }
    let last4: String = chars[n - 4..].iter().collect();
    format!("***{}", last4)
}

/// 센티넬 판별: `get_settings`가 반환한 마스킹된 키인지.
///
/// 실제 Gemini API 키는 `AIzaSy`로 시작하는 39자이므로 `***`로 시작하는
/// 짧은 문자열은 안전하게 센티넬로 식별 가능.
fn is_masked_sentinel(value: &str) -> bool {
    value.starts_with("***") && value.chars().count() <= 7
}

/// API 키를 credentials.json에 저장 (atomic write)
/// Windows에서는 icacls로 현재 사용자만 접근 가능하도록 ACL 격리.
fn save_api_key(app_data_dir: &Path, key: Option<&str>) -> Result<(), std::io::Error> {
    let path = get_credentials_path(app_data_dir);
    let creds = Credentials {
        ai_api_key: key.map(|k| k.to_string()),
    };
    let json = serde_json::to_string_pretty(&creds).map_err(std::io::Error::other)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, &json)?;
    fs::rename(&tmp, &path)?;

    // 평문 API 키 파일은 현재 사용자 계정으로만 접근 제한
    // (멀티유저 PC에서 다른 계정/프로세스 접근 차단)
    #[cfg(windows)]
    restrict_file_to_owner(&path);

    Ok(())
}

/// Windows에서 파일 ACL을 현재 사용자 계정만 접근 가능하도록 제한
///
/// `icacls <file> /inheritance:r /grant:r "<USER>":F` 실행:
/// - `/inheritance:r` → 부모 폴더 ACL 상속 제거 (Users 그룹 등 차단)
/// - `/grant:r "<USER>":F` → 현재 사용자에게만 Full control 부여
///
/// 실패해도 저장 자체는 성공시킨다. ACL 실패는 tracing::warn으로만 기록.
#[cfg(windows)]
fn restrict_file_to_owner(path: &Path) {
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let username = match std::env::var("USERNAME") {
        Ok(u) if !u.is_empty() => u,
        _ => {
            tracing::warn!("USERNAME env missing; skipping credentials.json ACL lockdown");
            return;
        }
    };

    let grant = format!(r#"{}:F"#, username);
    // System32 풀패스 사용으로 PATH hijack 방지.
    let system_root = std::env::var("SystemRoot").unwrap_or_else(|_| String::from("C:\\Windows"));
    let icacls = std::path::PathBuf::from(system_root)
        .join("System32")
        .join("icacls.exe");
    let result = Command::new(icacls)
        .arg(path)
        .args(["/inheritance:r", "/grant:r", &grant])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    match result {
        Ok(out) if out.status.success() => {
            tracing::debug!("credentials.json ACL restricted to {}", username);
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            tracing::warn!("icacls failed ({}): {}", out.status, stderr.trim());
        }
        Err(e) => {
            tracing::warn!("icacls invocation failed: {}", e);
        }
    }
}

/// 기존 settings.json에 ai_api_key가 남아있으면 credentials.json으로 마이그레이션
fn migrate_api_key_if_needed(app_data_dir: &Path) {
    let settings_path = get_settings_path(app_data_dir);
    let creds_path = get_credentials_path(app_data_dir);

    // credentials.json이 이미 존재하면 마이그레이션 불필요
    if creds_path.exists() {
        return;
    }

    // settings.json에서 ai_api_key 추출 시도
    let Ok(content) = fs::read_to_string(&settings_path) else {
        return;
    };
    let Ok(mut json_value) = serde_json::from_str::<serde_json::Value>(&content) else {
        return;
    };

    if let Some(key) = json_value.get("ai_api_key").and_then(|v| v.as_str()) {
        if !key.is_empty() {
            tracing::info!("Migrating API key from settings.json to credentials.json");
            let _ = save_api_key(app_data_dir, Some(key));
        }
    }

    // settings.json에서 ai_api_key 필드 제거
    if let Some(obj) = json_value.as_object_mut() {
        if obj.remove("ai_api_key").is_some() {
            if let Ok(cleaned) = serde_json::to_string_pretty(&json_value) {
                let tmp = settings_path.with_extension("json.tmp");
                let _ = fs::write(&tmp, &cleaned).and_then(|_| fs::rename(&tmp, &settings_path));
            }
        }
    }
}

/// 설정 조회 (캐시에서 읽기, 디스크 I/O 없음)
///
/// 프론트엔드 반환 직전에 `ai_api_key`를 마스킹 센티넬로 교체해
/// renderer 프로세스 메모리에 평문 키가 잔류하지 않도록 한다.
/// 인메모리 캐시 자체는 평문 유지(LLM 호출용).
#[tauri::command]
pub async fn get_settings(state: State<'_, RwLock<AppContainer>>) -> ApiResult<Settings> {
    let container = state.read()?;
    let mut settings = container.get_settings();
    if let Some(ref k) = settings.ai_api_key {
        if !k.is_empty() {
            settings.ai_api_key = Some(mask_api_key(k));
        }
    }
    Ok(settings)
}

/// OCR 재인덱싱 후보. OCR 를 새로 켜면 이미 인덱싱된 이미지·PDF 는 재인덱싱해야 OCR 로
/// 인식된다. 그 파일을 담은 감시 폴더 목록과 총 개수 — 프론트가 "재인덱싱?" 프롬프트에 사용.
#[derive(Debug, Serialize)]
pub struct OcrReindexCandidates {
    pub count: usize,
    pub folders: Vec<String>,
}

/// 이미 인덱싱된 OCR 대상(이미지·PDF) 파일 수와 그 파일을 담은 감시 폴더를 센다(읽기 전용).
///
/// 재인덱싱 단위가 폴더이므로 파일 단위 스캔 판정은 불필요 — 이미지·PDF 를 담은 폴더만
/// 재인덱싱하면 스캔 PDF·이미지가 OCR 로 인식된다. SQLite `LIKE` 는 ASCII 대소문자 무관이라
/// 대문자 확장자(.PDF)도 매칭된다.
#[tauri::command]
pub async fn count_ocr_reindex_candidates(
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<OcrReindexCandidates> {
    let db_path = { state.read()?.db_path.clone() };
    let conn = crate::db::get_connection(&db_path)
        .map_err(|e| ApiError::DatabaseConnection(e.to_string()))?;

    // OCR 대상 확장자 = 이미지 + pdf → 확장자별 LIKE 패턴.
    let patterns: Vec<String> = crate::constants::OCR_IMAGE_EXTENSIONS
        .iter()
        .copied()
        .chain(std::iter::once("pdf"))
        .map(|e| format!("%.{e}"))
        .collect();

    let count_clause = vec!["path LIKE ?"; patterns.len()].join(" OR ");
    let count: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM files WHERE {count_clause}"),
        rusqlite::params_from_iter(patterns.iter()),
        |row| row.get(0),
    )?;

    // 후보 파일을 하나라도 담은 감시 폴더(경로 접두 매칭).
    let folder_clause = vec!["f.path LIKE ?"; patterns.len()].join(" OR ");
    let sql = format!(
        "SELECT DISTINCT w.path FROM watched_folders w \
         JOIN files f ON f.path LIKE w.path || '/%' \
         WHERE {folder_clause}"
    );
    let mut stmt = conn.prepare(&sql)?;
    let folders = stmt
        .query_map(rusqlite::params_from_iter(patterns.iter()), |row| {
            row.get::<_, String>(0)
        })?
        .filter_map(Result::ok)
        .collect::<Vec<String>>();

    Ok(OcrReindexCandidates {
        count: count as usize,
        folders,
    })
}

/// 설정 동기 조회 (내부 사용)
/// 수동 편집된 설정 파일의 비정상 값에 대비하여 범위 클램핑 적용
pub fn get_settings_sync(app_data_dir: &Path) -> Settings {
    // 기존 settings.json에 남은 API 키 → credentials.json으로 마이그레이션
    migrate_api_key_if_needed(app_data_dir);

    let settings_path = get_settings_path(app_data_dir);

    let mut settings: Settings = if settings_path.exists() {
        match fs::read_to_string(&settings_path).map(|c| serde_json::from_str::<Settings>(&c)) {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => {
                // 손상 파일을 기본값으로 조용히 대체하면 다음 저장에서 사용자 설정이
                // 통째로 기본값으로 덮여 영구 소실된다 — 백업을 남기고 경고.
                let backup = settings_path.with_extension("json.corrupt");
                match fs::copy(&settings_path, &backup) {
                    Ok(_) => {
                        tracing::warn!("settings.json 파싱 실패({}), 손상본 백업: {:?}", e, backup)
                    }
                    Err(be) => {
                        tracing::warn!("settings.json 파싱 실패({}), 백업도 실패: {}", e, be)
                    }
                }
                Settings::default()
            }
            Err(e) => {
                tracing::warn!("settings.json 읽기 실패({}), 기본값 사용", e);
                Settings::default()
            }
        }
    } else {
        Settings::default()
    };

    // API 키는 credentials.json에서 로드
    settings.ai_api_key = load_api_key(app_data_dir);

    // 단종/구버전 모델 ID → 최신 GA 자동 마이그레이션 (Gemini API에 실존하지 않거나
    // preview→GA 승격으로 셧다운돼 404 를 유발하던 값들)
    let migrated_model = match settings.ai_model.as_str() {
        // 오타: Gemini 실제 ID는 하이픈 표기
        "gemini-3.0-flash" => Some("gemini-3.5-flash".to_string()),
        // preview → GA 승격 후 구 preview ID 셧다운 (2026-06)
        "gemini-3.1-flash-lite-preview" => Some("gemini-3.1-flash-lite".to_string()),
        "gemini-3-flash-preview" => Some("gemini-3.5-flash".to_string()),
        "gemini-3-pro-preview" => Some("gemini-3.1-pro-preview".to_string()),
        // 2.0 계열 단종 → 2.5 GA
        "gemini-2.0-flash" | "gemini-2.0-flash-lite" => Some("gemini-2.5-flash".to_string()),
        _ => None,
    };
    if let Some(m) = migrated_model {
        tracing::info!(
            "ai_model 자동 마이그레이션: '{}' → '{}'",
            settings.ai_model,
            m
        );
        settings.ai_model = m;
    }

    // 범위 클램핑 (수동 편집된 비정상 값 방어)
    settings.max_results = settings.max_results.clamp(1, 500);
    settings.chunk_size = settings.chunk_size.clamp(256, 4096);
    settings.chunk_overlap = settings
        .chunk_overlap
        .min(settings.chunk_size.saturating_sub(1));
    settings.results_per_page = settings.results_per_page.clamp(1, 200);
    settings.max_file_size_mb = settings.max_file_size_mb.min(MAX_FILE_SIZE_LIMIT_MB);
    settings.min_confidence = settings.min_confidence.min(100);
    settings.ai_temperature = settings.ai_temperature.clamp(0.0, 2.0);
    settings.ai_max_tokens = settings.ai_max_tokens.clamp(1, 8192);
    // auto_sync_interval_minutes: 0(끄기) 또는 [1, 60*24] 범위
    if settings.auto_sync_interval_minutes > 60 * 24 {
        settings.auto_sync_interval_minutes = 60 * 24;
    }

    settings
}

/// 설정 값 범위 검증 (서버측 — 프론트엔드 우회 방지)
fn validate_settings(settings: &Settings) -> ApiResult<()> {
    if settings.max_results == 0 || settings.max_results > 500 {
        return Err(ApiError::Validation(
            "max_results는 1~500 범위여야 합니다".into(),
        ));
    }
    if settings.chunk_size < 256 || settings.chunk_size > 4096 {
        return Err(ApiError::Validation(
            "chunk_size는 256~4096 범위여야 합니다".into(),
        ));
    }
    if settings.chunk_overlap >= settings.chunk_size {
        return Err(ApiError::Validation(
            "chunk_overlap은 chunk_size보다 작아야 합니다".into(),
        ));
    }
    if settings.results_per_page == 0 || settings.results_per_page > 200 {
        return Err(ApiError::Validation(
            "results_per_page는 1~200 범위여야 합니다".into(),
        ));
    }
    if settings.max_file_size_mb > MAX_FILE_SIZE_LIMIT_MB {
        return Err(ApiError::Validation(format!(
            "max_file_size_mb는 최대 {}MB입니다",
            MAX_FILE_SIZE_LIMIT_MB
        )));
    }
    if settings.ai_temperature < 0.0 || settings.ai_temperature > 2.0 {
        return Err(ApiError::Validation(
            "ai_temperature는 0.0~2.0 범위여야 합니다".into(),
        ));
    }
    if settings.ai_max_tokens == 0 || settings.ai_max_tokens > 8192 {
        return Err(ApiError::Validation(
            "ai_max_tokens는 1~8192 범위여야 합니다".into(),
        ));
    }
    // Provider / 모델 ID 정합성 — OpenAI 호환 모드에 Gemini 모델 ID 가 들어가면
    // 사내 LLM 서버가 404 "model not found" 를 던지는 사고 방지. 프론트가 이미
    // provider 전환 시 자동 swap 하지만 수동 편집 / 마이그레이션 케이스 방어용.
    if matches!(settings.ai_provider, AiProvider::OpenAi)
        && settings.ai_model.starts_with("gemini-")
    {
        return Err(ApiError::Validation(
            "OpenAI 호환 provider 에 Gemini 모델 ID 가 설정돼 있습니다. \
             사내 LLM 서버의 실제 모델 ID (예: qwen3.6-35b-a3b) 로 바꿔주세요."
                .into(),
        ));
    }
    if settings.min_confidence > 100 {
        return Err(ApiError::Validation(
            "min_confidence는 0~100 범위여야 합니다".into(),
        ));
    }
    if settings.auto_sync_interval_minutes > 60 * 24 {
        return Err(ApiError::Validation(
            "auto_sync_interval_minutes는 0(끄기)~1440(24시간) 범위여야 합니다".into(),
        ));
    }
    Ok(())
}

/// 설정 업데이트
#[tauri::command]
pub async fn update_settings(
    app: AppHandle,
    mut settings: Settings,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<()> {
    validate_settings(&settings)?;
    tracing::info!(
        "Updating settings: mode={:?}, theme={:?}, semantic={}, ocr={}, ai_key={}",
        settings.search_mode,
        settings.theme,
        settings.semantic_search_enabled,
        settings.ocr_enabled,
        if settings.ai_api_key.is_some() {
            "[SET]"
        } else {
            "[NONE]"
        }
    );

    let app_data_dir = {
        let state = state.read()?;
        state.app_data_dir.clone()
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

    // 키 변경 분기:
    // - 센티넬 → 기존 키 유지 (프론트에서 편집 안 함)
    // - None/빈 문자열 → **기존 키 유지** (마스킹 UI가 input을 비우기 때문에
    //   사용자가 다른 설정만 바꿔도 빈 값이 올 수 있음. 실수 삭제 방지).
    // - 그 외 실제 문자열 → 신규 키 저장
    // 명시적 삭제는 별도 UI 이벤트(키 삭제 버튼)로 분리 예정.
    let effective_key: Option<String> = match settings.ai_api_key.as_deref() {
        Some(k) if is_masked_sentinel(k) => load_api_key(&app_data_dir),
        Some("") | None => load_api_key(&app_data_dir),
        Some(k) => Some(k.to_string()),
    };

    // settings.json에는 API 키 없이 저장
    let mut settings_for_file = settings.clone();
    settings_for_file.ai_api_key = None;
    let settings_path = get_settings_path(&app_data_dir);
    let content = serde_json::to_string_pretty(&settings_for_file)
        .map_err(|e| ApiError::SettingsSave(e.to_string()))?;

    // atomic write: 크래시 시 설정 파일 손상 방지
    let tmp_path = settings_path.with_extension("json.tmp");
    fs::write(&tmp_path, &content).map_err(|e| ApiError::SettingsSave(e.to_string()))?;
    fs::rename(&tmp_path, &settings_path).map_err(|e| ApiError::SettingsSave(e.to_string()))?;

    // API 키는 settings.json 커밋 성공 후 저장 — 역순이면 settings 실패 시
    // "디스크=새 키 / 메모리=옛 키 / 설정=옛 값" 3-way 불일치로 재시작 전까지
    // LLM이 옛 키로 호출된다. 이 순서에선 실패해도 키만 옛 상태(재시도로 회복).
    let api_key_for_cache = effective_key.clone();
    save_api_key(&app_data_dir, effective_key.as_deref())
        .map_err(|e| ApiError::SettingsSave(format!("credentials save failed: {}", e)))?;

    // 인메모리 캐시 갱신 (API 키 포함)
    settings.ai_api_key = api_key_for_cache;
    {
        let container = state.read()?;
        container.update_settings_cache(settings.clone());
    }

    // 전역 formula OCR 토글 — kordoc 사이드카 호출 시 --formula-ocr 플래그 전파용
    crate::parsers::kordoc::set_formula_ocr_enabled(settings.formula_ocr_enabled);

    // 클라우드/네트워크 본문 인덱싱 스킵 토글 동기화 (parse_file 진입에서 즉시 반영)
    crate::utils::cloud_detect::set_skip_enabled(settings.skip_cloud_body_indexing);
    // 시스템 폴더 추가 허용 토글 동기화 (validate_watch_path 즉시 반영)
    crate::constants::set_allow_system_folders(settings.allow_system_folders);

    tracing::info!("Settings saved to {:?}", settings_path);

    // 시맨틱 검색 활성화 시 모델이 없으면 백그라운드 다운로드 시작
    if settings.semantic_search_enabled {
        let models_dir = app_data_dir.join("models");
        let e5_model_int8 = models_dir
            .join("kosimcse-roberta-multitask")
            .join("model_int8.onnx");
        let e5_model = models_dir
            .join("kosimcse-roberta-multitask")
            .join("model.onnx");
        let e5_model_data = models_dir
            .join("kosimcse-roberta-multitask")
            .join("model.onnx.data");
        // INT8 모델 또는 F32 모델(+data) 중 하나라도 있으면 OK
        let embedder_available =
            e5_model_int8.exists() || (e5_model.exists() && e5_model_data.exists());
        if !embedder_available {
            let download_models_dir = models_dir.clone();
            let download_app = app.clone();
            tauri::async_runtime::spawn(async move {
                let _ = download_app.emit("model-download-status", "downloading");
                match tokio::task::spawn_blocking(move || {
                    model_downloader::ensure_models(&download_models_dir)
                })
                .await
                {
                    Ok(Ok(_)) => {
                        let _ = download_app.emit("model-download-status", "completed");
                        // 다운로드 완료 → VectorIndex OnceCell pre-init
                        // (WatchManager 벡터 삭제 경로 활성화, 재시작 없이 사용 가능)
                        if let Some(state) = download_app.try_state::<RwLock<AppContainer>>() {
                            if let Ok(container) = state.read() {
                                if let Err(e) = container.get_vector_index() {
                                    tracing::warn!("VectorIndex pre-init 실패(다운로드 후): {}", e);
                                }
                            }
                        }
                    }
                    _ => {
                        let _ = download_app.emit("model-download-status", "failed");
                    }
                }
            });
        } else {
            // 이미 모델이 있으나 VectorIndex OnceCell이 아직 비어있다면 지금 init
            // (사용자가 앱 설치 후 수동으로 models 폴더에 파일 배치한 케이스)
            if let Ok(container) = state.read() {
                if let Err(e) = container.get_vector_index() {
                    tracing::debug!("VectorIndex pre-init skip: {}", e);
                }
            }
        }
    }

    // OCR 활성화 시 모델이 없으면 백그라운드 다운로드 시작
    if settings.ocr_enabled {
        let models_dir = app_data_dir.join("models");
        let ocr_dir = models_dir.join("paddleocr");
        let det_exists = ocr_dir.join("det.onnx").exists();
        let rec_exists = ocr_dir.join("rec.onnx").exists();
        if !det_exists || !rec_exists {
            let download_app = app.clone();
            tauri::async_runtime::spawn(async move {
                let _ = download_app.emit("model-download-status", "downloading-ocr");
                match tokio::task::spawn_blocking(move || {
                    model_downloader::ensure_ocr_models(&models_dir)
                })
                .await
                {
                    Ok(Ok(_)) => {
                        let _ = download_app.emit("model-download-status", "completed-ocr");
                    }
                    _ => {
                        let _ = download_app.emit("model-download-status", "failed-ocr");
                    }
                }
            });
        }

        // pdfium 도 함께 준비 — 런타임 OCR 토글에서도 스캔/이미지 PDF 래스터화가
        // 즉시 동작하도록. (기존엔 앱 시작 setup 에서만 받아, 런타임에 켜면 재시작
        // 전까지 스캔 PDF 인식이 안 됐다.) best-effort — 실패해도 기존 경로는 무손상.
        let pdfium_models_dir = app_data_dir.join("models");
        let pdfium_lib = pdfium_models_dir
            .join("pdfium")
            .join(model_downloader::pdfium_lib_filename());
        if !pdfium_lib.exists() {
            tauri::async_runtime::spawn(async move {
                let _ = tokio::task::spawn_blocking(move || {
                    model_downloader::ensure_pdfium(&pdfium_models_dir)
                })
                .await;
            });
        }

        // PP-DocLayout 레이아웃 모델 — 레이아웃 분석 토글이 켜졌을 때만 받는다(선택·실험).
        // 끄면 모델 파일을 제거해, 다음 OCR 엔진 초기화(재시작)부터 레이아웃 분석이 꺼진다.
        let layout_models_dir = app_data_dir.join("models");
        let layout_path = layout_models_dir.join("paddleocr").join("layout.onnx");
        if settings.ocr_layout_enabled {
            if !layout_path.exists() {
                tauri::async_runtime::spawn(async move {
                    let _ = tokio::task::spawn_blocking(move || {
                        model_downloader::ensure_layout_model(&layout_models_dir)
                    })
                    .await;
                });
            }
        } else {
            let _ = std::fs::remove_file(&layout_path);
        }
    }

    Ok(())
}
