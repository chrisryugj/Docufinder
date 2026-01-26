//! 모델 자동 다운로드 모듈
//!
//! ONNX Runtime과 임베딩 모델을 자동으로 다운로드합니다.
#![allow(dead_code)]

use std::fs::{self, File};
use std::io;
use std::path::Path;

const ONNX_RUNTIME_URL: &str = "https://github.com/microsoft/onnxruntime/releases/download/v1.20.1/onnxruntime-win-x64-1.20.1.zip";
const E5_MODEL_URL: &str = "https://huggingface.co/Teradata/multilingual-e5-small/resolve/main/onnx/model_int8.onnx";
const E5_TOKENIZER_URL: &str = "https://huggingface.co/Teradata/multilingual-e5-small/resolve/main/tokenizer.json";

// Cross-Encoder Reranker (ms-marco-MiniLM-L-6-v2)
const RERANKER_MODEL_URL: &str = "https://huggingface.co/Xenova/ms-marco-MiniLM-L-6-v2/resolve/main/onnx/model_quantized.onnx";
const RERANKER_TOKENIZER_URL: &str = "https://huggingface.co/Xenova/ms-marco-MiniLM-L-6-v2/resolve/main/tokenizer.json";

/// 모델 다운로드 진행률 콜백
pub type ProgressCallback = Box<dyn Fn(u64, u64, &str) + Send>;

/// 모델 다운로드 결과
#[derive(Debug)]
pub struct DownloadResult {
    pub onnx_runtime_downloaded: bool,
    pub model_downloaded: bool,
    pub tokenizer_downloaded: bool,
    pub reranker_model_downloaded: bool,
    pub reranker_tokenizer_downloaded: bool,
}

/// 필요한 모델 파일들을 확인하고 없으면 다운로드
pub fn ensure_models(models_dir: &Path) -> Result<DownloadResult, String> {
    let e5_dir = models_dir.join("multilingual-e5-small");
    fs::create_dir_all(&e5_dir).map_err(|e| format!("디렉토리 생성 실패: {}", e))?;

    let dll_path = e5_dir.join("onnxruntime.dll");
    let model_path = e5_dir.join("model.onnx");
    let tokenizer_path = e5_dir.join("tokenizer.json");

    let mut result = DownloadResult {
        onnx_runtime_downloaded: false,
        model_downloaded: false,
        tokenizer_downloaded: false,
        reranker_model_downloaded: false,
        reranker_tokenizer_downloaded: false,
    };

    // ONNX Runtime DLL 다운로드
    if !dll_path.exists() {
        tracing::info!("ONNX Runtime 다운로드 중...");
        download_onnx_runtime(&e5_dir)?;
        result.onnx_runtime_downloaded = true;
        tracing::info!("ONNX Runtime 다운로드 완료");
    }

    // E5 임베딩 모델 다운로드
    if !model_path.exists() {
        tracing::info!("E5 모델 다운로드 중...");
        download_file(E5_MODEL_URL, &model_path)?;
        result.model_downloaded = true;
        tracing::info!("E5 모델 다운로드 완료");
    }

    // E5 토크나이저 다운로드
    if !tokenizer_path.exists() {
        tracing::info!("E5 토크나이저 다운로드 중...");
        download_file(E5_TOKENIZER_URL, &tokenizer_path)?;
        result.tokenizer_downloaded = true;
        tracing::info!("E5 토크나이저 다운로드 완료");
    }

    // Cross-Encoder Reranker 모델 다운로드
    let reranker_result = ensure_reranker_model(models_dir)?;
    result.reranker_model_downloaded = reranker_result.0;
    result.reranker_tokenizer_downloaded = reranker_result.1;

    Ok(result)
}

/// Cross-Encoder Reranker 모델 다운로드
pub fn ensure_reranker_model(models_dir: &Path) -> Result<(bool, bool), String> {
    let reranker_dir = models_dir.join("ms-marco-MiniLM-L6-v2");
    fs::create_dir_all(&reranker_dir).map_err(|e| format!("Reranker 디렉토리 생성 실패: {}", e))?;

    let model_path = reranker_dir.join("model.onnx");
    let tokenizer_path = reranker_dir.join("tokenizer.json");

    let mut model_downloaded = false;
    let mut tokenizer_downloaded = false;

    // Reranker 모델 다운로드
    if !model_path.exists() {
        tracing::info!("Reranker 모델 다운로드 중...");
        download_file(RERANKER_MODEL_URL, &model_path)?;
        model_downloaded = true;
        tracing::info!("Reranker 모델 다운로드 완료");
    }

    // Reranker 토크나이저 다운로드
    if !tokenizer_path.exists() {
        tracing::info!("Reranker 토크나이저 다운로드 중...");
        download_file(RERANKER_TOKENIZER_URL, &tokenizer_path)?;
        tokenizer_downloaded = true;
        tracing::info!("Reranker 토크나이저 다운로드 완료");
    }

    Ok((model_downloaded, tokenizer_downloaded))
}

/// 파일 다운로드
fn download_file(url: &str, dest: &Path) -> Result<(), String> {
    let response = ureq::get(url)
        .call()
        .map_err(|e| format!("다운로드 실패 ({}): {}", url, e))?;

    let mut file = File::create(dest)
        .map_err(|e| format!("파일 생성 실패: {}", e))?;

    let mut reader = response.into_body().into_reader();
    io::copy(&mut reader, &mut file)
        .map_err(|e| format!("파일 쓰기 실패: {}", e))?;

    Ok(())
}

/// ONNX Runtime ZIP 다운로드 및 압축 해제
fn download_onnx_runtime(dest_dir: &Path) -> Result<(), String> {
    let response = ureq::get(ONNX_RUNTIME_URL)
        .call()
        .map_err(|e| format!("ONNX Runtime 다운로드 실패: {}", e))?;

    // 임시 파일에 저장
    let temp_path = dest_dir.join("onnxruntime_temp.zip");
    {
        let mut file = File::create(&temp_path)
            .map_err(|e| format!("임시 파일 생성 실패: {}", e))?;

        let mut reader = response.into_body().into_reader();
        io::copy(&mut reader, &mut file)
            .map_err(|e| format!("파일 쓰기 실패: {}", e))?;
    }

    // ZIP 압축 해제
    let file = File::open(&temp_path)
        .map_err(|e| format!("ZIP 파일 열기 실패: {}", e))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("ZIP 아카이브 열기 실패: {}", e))?;

    // onnxruntime.dll 찾아서 추출
    let dll_name = "onnxruntime.dll";
    let mut dll_found = false;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)
            .map_err(|e| format!("ZIP 엔트리 읽기 실패: {}", e))?;

        let name = file.name().to_string();
        if name.ends_with(dll_name) {
            let dest_path = dest_dir.join(dll_name);
            let mut dest_file = File::create(&dest_path)
                .map_err(|e| format!("DLL 파일 생성 실패: {}", e))?;

            io::copy(&mut file, &mut dest_file)
                .map_err(|e| format!("DLL 파일 쓰기 실패: {}", e))?;

            dll_found = true;
            break;
        }
    }

    // 임시 파일 삭제
    let _ = fs::remove_file(&temp_path);

    if !dll_found {
        return Err("ZIP에서 onnxruntime.dll을 찾을 수 없습니다".to_string());
    }

    Ok(())
}

/// 모델 파일 존재 여부 확인
pub fn check_models(models_dir: &Path) -> (bool, bool, bool) {
    let e5_dir = models_dir.join("multilingual-e5-small");
    (
        e5_dir.join("onnxruntime.dll").exists(),
        e5_dir.join("model.onnx").exists(),
        e5_dir.join("tokenizer.json").exists(),
    )
}

/// Reranker 모델 파일 존재 여부 확인
pub fn check_reranker_model(models_dir: &Path) -> (bool, bool) {
    let reranker_dir = models_dir.join("ms-marco-MiniLM-L6-v2");
    (
        reranker_dir.join("model.onnx").exists(),
        reranker_dir.join("tokenizer.json").exists(),
    )
}
