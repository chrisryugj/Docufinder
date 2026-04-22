//! PDF 수식 OCR 모델 관리 커맨드
//!
//! kordoc 사이드카 CLI (`check-formula-models --status-only`) 를 호출해
//! 수식 모델(MFD + MFR + tokenizer, 총 ~155MB) 의 상태를 확인하거나,
//! 없으면 HuggingFace 에서 다운로드 + SHA-256 검증을 트리거한다.
//!
//! 모델은 `~/.cache/kordoc/models/pix2text/` 에 저장되어 kordoc 사이드카와
//! 공유된다 (사용자 홈 캐시 → Docufinder 재설치 후에도 유지).

use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use std::process::Stdio;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

use crate::error::{ApiError, ApiResult};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModelStatus {
    pub name: String,
    pub filename: String,
    #[serde(rename = "sizeMb")]
    pub size_mb: u64,
    pub exists: bool,
    pub verified: bool,
    pub path: String,
    #[serde(rename = "invalidReason", default)]
    pub invalid_reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FormulaModelsStatus {
    #[serde(rename = "modelsDir")]
    pub models_dir: String,
    #[serde(rename = "allReady")]
    pub all_ready: bool,
    pub models: Vec<ModelStatus>,
}

/// kordoc 사이드카의 `check-formula-models --status-only` 를 호출해 상태만 확인.
#[tauri::command]
pub async fn get_formula_models_status() -> ApiResult<FormulaModelsStatus> {
    tokio::task::spawn_blocking(run_check_status_only)
        .await
        .map_err(|e| ApiError::CommandFailed(format!("spawn_blocking: {}", e)))?
}

fn run_check_status_only() -> ApiResult<FormulaModelsStatus> {
    let cli = crate::parsers::kordoc::find_kordoc_cli_public()
        .ok_or_else(|| ApiError::CommandFailed("kordoc CLI 찾을 수 없음".to_string()))?;
    let node = crate::parsers::kordoc::which_node_public()
        .ok_or_else(|| ApiError::CommandFailed("Node.js 없음".to_string()))?;

    let mut cmd = std::process::Command::new(node);
    cmd.arg(cli.to_string_lossy().as_ref())
        .arg("check-formula-models")
        .arg("--status-only")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    let output = cmd
        .output()
        .map_err(|e| ApiError::CommandFailed(format!("kordoc 실행 실패: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ApiError::CommandFailed(format!(
            "kordoc check-formula-models 실패: {}",
            stderr
        )));
    }

    let stdout = String::from_utf8(output.stdout)
        .map_err(|e| ApiError::CommandFailed(format!("응답 UTF-8 디코딩 실패: {}", e)))?;
    serde_json::from_str::<FormulaModelsStatus>(&stdout)
        .map_err(|e| ApiError::CommandFailed(format!("응답 JSON 파싱 실패: {}", e)))
}

/// 수식 모델 다운로드 트리거 — 진행률 이벤트를 `formula-model-progress` 로 emit.
///
/// 최종 반환 전까지 stderr 를 줄 단위로 읽어 프론트에 push.
/// kordoc 의 stderr 포맷 예:
///   `[kordoc-formula] Pix2Text MFD 42% (18.5/44.2MB)`
///   `[kordoc-formula] Pix2Text MFD 준비 완료`
///   `[kordoc-formula] Pix2Text MFR encoder 이미 존재 (skip)`
#[tauri::command]
pub async fn download_formula_models(app: AppHandle) -> ApiResult<()> {
    tokio::task::spawn_blocking(move || run_download(app))
        .await
        .map_err(|e| ApiError::CommandFailed(format!("spawn_blocking: {}", e)))?
}

fn run_download(app: AppHandle) -> ApiResult<()> {
    let cli = crate::parsers::kordoc::find_kordoc_cli_public()
        .ok_or_else(|| ApiError::CommandFailed("kordoc CLI 찾을 수 없음".to_string()))?;
    let node = crate::parsers::kordoc::which_node_public()
        .ok_or_else(|| ApiError::CommandFailed("Node.js 없음".to_string()))?;

    let mut cmd = std::process::Command::new(node);
    cmd.arg(cli.to_string_lossy().as_ref())
        .arg("check-formula-models")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| ApiError::CommandFailed(format!("kordoc 실행 실패: {}", e)))?;

    let stderr = child.stderr.take().unwrap();
    let reader = BufReader::new(stderr);
    // 진행률 파싱 + emit. 완료 시 loop 종료.
    for line in reader.split(b'\n').flatten() {
        let text = String::from_utf8_lossy(&line).trim().to_string();
        if text.is_empty() {
            continue;
        }
        let _ = app.emit("formula-model-progress", text);
    }

    let status = child
        .wait()
        .map_err(|e| ApiError::CommandFailed(format!("kordoc 대기 실패: {}", e)))?;

    // 모든 stderr drain 후 잠깐 대기 (프론트가 마지막 progress emit 수신 여유)
    std::thread::sleep(Duration::from_millis(50));

    if !status.success() {
        return Err(ApiError::CommandFailed(format!(
            "수식 모델 다운로드 실패 (exit {:?})",
            status.code()
        )));
    }

    Ok(())
}
