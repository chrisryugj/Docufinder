//! 모델 자동 다운로드 모듈
//!
//! ONNX Runtime과 임베딩 모델을 자동으로 다운로드합니다.
//! SHA-256 무결성 검증을 수행합니다.
#![allow(dead_code)]

use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;
use std::time::Duration;

// ============================================================================
// 모델 URL 및 SHA-256 해시 (무결성 검증용)
// ============================================================================

const ONNX_RUNTIME_URL: &str = "https://github.com/microsoft/onnxruntime/releases/download/v1.20.1/onnxruntime-win-x64-1.20.1.zip";
// KoSimCSE-roberta-multitask (HuggingFace) — INT8 동적 양자화 모델
const E5_MODEL_URL: &str =
    "https://huggingface.co/chrisryugj/kosimcse-roberta-multitask-onnx/resolve/main/model_int8.onnx";
const E5_TOKENIZER_URL: &str =
    "https://huggingface.co/chrisryugj/kosimcse-roberta-multitask-onnx/resolve/main/tokenizer.json";
// F32 원본 URL (하위 호환용, 현재 미사용)
#[allow(dead_code)]
const E5_MODEL_F32_URL: &str =
    "https://huggingface.co/chrisryugj/kosimcse-roberta-multitask-onnx/resolve/main/model.onnx";
#[allow(dead_code)]
const E5_MODEL_DATA_URL: &str = "https://huggingface.co/chrisryugj/kosimcse-roberta-multitask-onnx/resolve/main/model.onnx.data";

// SHA-256 해시 (무결성 검증)
// 주의: 모델 버전 업데이트 시 해시값도 업데이트 필요
// INT8 양자화 모델 SHA-256
const E5_MODEL_SHA256: &str = "877e43d3f3a2ee09a58c08a0d1720f99b3496962e92569c5846299f862ac0f33";
// F32 원본 해시 (하위 호환용, 현재 미사용)
#[allow(dead_code)]
const E5_MODEL_F32_SHA256: &str =
    "a1e12d33caecc60aa192fa1bb56a5a7a4d817486e7420e38662acc6e1c357b5d";
#[allow(dead_code)]
const E5_MODEL_DATA_SHA256: &str =
    "98691c75a2129885f4a9da144749d0a97c36d2c7a0d425559463046eadb2de9f";
const E5_TOKENIZER_SHA256: &str =
    "d607daae73f6a05440b09833097b34c3f6eea3a53d6ab010a6c0c07081f0a5ab";
// ONNX Runtime ZIP SHA-256 (v1.20.1 win-x64)
const ONNX_RUNTIME_ZIP_SHA256: &str =
    "78d447051e48bd2e1e778bba378bec4ece11191c9e538cf7b2c4a4565e8f5581";

// PaddleOCR ONNX 모델 (Hugging Face: monkt/paddleocr-onnx)
const OCR_DET_URL: &str =
    "https://huggingface.co/monkt/paddleocr-onnx/resolve/main/detection/v3/det.onnx";
const OCR_REC_KO_URL: &str =
    "https://huggingface.co/monkt/paddleocr-onnx/resolve/main/languages/korean/rec.onnx";
const OCR_DICT_KO_URL: &str =
    "https://huggingface.co/monkt/paddleocr-onnx/resolve/main/languages/korean/dict.txt";
// SHA-256: 최초 다운로드 후 compute_sha256()으로 계산하여 채울 것
// 현재는 빈 문자열 → 검증 스킵
const OCR_DET_SHA256: &str = "";
const OCR_REC_KO_SHA256: &str = "";
const OCR_DICT_KO_SHA256: &str = "";

// 다운로드 설정
const CONNECT_TIMEOUT_SECS: u64 = 30;
const READ_TIMEOUT_SECS: u64 = 600; // 10분 (대용량 모델)
const MAX_FILE_SIZE: u64 = 600 * 1024 * 1024; // 600MB 상한

/// 모델 다운로드 진행률 콜백
pub type ProgressCallback = Box<dyn Fn(u64, u64, &str) + Send>;

/// 모델 다운로드 결과
#[derive(Debug)]
pub struct DownloadResult {
    pub onnx_runtime_downloaded: bool,
    pub model_downloaded: bool,
    pub model_data_downloaded: bool,
    pub tokenizer_downloaded: bool,
}

/// 필요한 모델 파일들을 확인하고 없으면 다운로드
pub fn ensure_models(models_dir: &Path) -> Result<DownloadResult, String> {
    let e5_dir = models_dir.join("kosimcse-roberta-multitask");
    fs::create_dir_all(&e5_dir).map_err(|e| format!("디렉토리 생성 실패: {}", e))?;

    let dll_path = e5_dir.join("onnxruntime.dll");
    let model_int8_path = e5_dir.join("model_int8.onnx");
    let model_f32_path = e5_dir.join("model.onnx");
    let tokenizer_path = e5_dir.join("tokenizer.json");

    let mut result = DownloadResult {
        onnx_runtime_downloaded: false,
        model_downloaded: false,
        model_data_downloaded: false,
        tokenizer_downloaded: false,
    };

    // ONNX Runtime DLL 다운로드
    if !dll_path.exists() {
        tracing::info!("ONNX Runtime 다운로드 중...");
        download_onnx_runtime(&e5_dir)?;
        result.onnx_runtime_downloaded = true;
        tracing::info!("ONNX Runtime 다운로드 완료");
    }

    // INT8 양자화 모델 다운로드 (SHA-256 검증)
    if !model_int8_path.exists() {
        tracing::info!("임베딩 모델 다운로드 중 (model_int8.onnx, ~106MB)...");
        download_file_verified(E5_MODEL_URL, &model_int8_path, E5_MODEL_SHA256)?;
        result.model_downloaded = true;
        tracing::info!("임베딩 모델(INT8) 다운로드 및 검증 완료");
    } else {
        verify_existing_file(&model_int8_path, E5_MODEL_SHA256, "임베딩 모델(INT8)")?;
    }

    // F32 → INT8 마이그레이션: INT8 다운로드 성공 후 F32 원본 삭제 (RAM 절약)
    if model_int8_path.exists() && model_f32_path.exists() {
        let model_data_path = e5_dir.join("model.onnx.data");
        tracing::info!("F32 원본 모델 삭제 중 (INT8로 교체 완료)...");
        let _ = fs::remove_file(&model_f32_path);
        let _ = fs::remove_file(&model_data_path);
        tracing::info!("F32 모델 삭제 완료 (~840MB 절약)");
    }

    // 토크나이저 다운로드 (SHA-256 검증)
    if !tokenizer_path.exists() {
        tracing::info!("토크나이저 다운로드 중...");
        download_file_verified(E5_TOKENIZER_URL, &tokenizer_path, E5_TOKENIZER_SHA256)?;
        result.tokenizer_downloaded = true;
        tracing::info!("토크나이저 다운로드 및 검증 완료");
    } else {
        verify_existing_file(&tokenizer_path, E5_TOKENIZER_SHA256, "토크나이저")?;
    }

    Ok(result)
}

/// PaddleOCR 모델 다운로드 (Detection + Korean Recognition + Dictionary)
pub fn ensure_ocr_models(models_dir: &Path) -> Result<(bool, bool, bool), String> {
    let ocr_dir = models_dir.join("paddleocr");
    fs::create_dir_all(&ocr_dir).map_err(|e| format!("OCR 디렉토리 생성 실패: {}", e))?;

    let det_path = ocr_dir.join("det.onnx");
    let rec_path = ocr_dir.join("rec.onnx");
    let dict_path = ocr_dir.join("dict.txt");

    let mut det_downloaded = false;
    let mut rec_downloaded = false;
    let mut dict_downloaded = false;

    if !det_path.exists() {
        tracing::info!("OCR Detection 모델 다운로드 중...");
        download_file_optional_hash(OCR_DET_URL, &det_path, OCR_DET_SHA256)?;
        det_downloaded = true;
        tracing::info!("OCR Detection 모델 다운로드 완료");
    }

    if !rec_path.exists() {
        tracing::info!("OCR Recognition (한국어) 모델 다운로드 중...");
        download_file_optional_hash(OCR_REC_KO_URL, &rec_path, OCR_REC_KO_SHA256)?;
        rec_downloaded = true;
        tracing::info!("OCR Recognition 모델 다운로드 완료");
    }

    if !dict_path.exists() {
        tracing::info!("OCR 한국어 사전 다운로드 중...");
        download_file_optional_hash(OCR_DICT_KO_URL, &dict_path, OCR_DICT_KO_SHA256)?;
        dict_downloaded = true;
        tracing::info!("OCR 한국어 사전 다운로드 완료");
    }

    Ok((det_downloaded, rec_downloaded, dict_downloaded))
}

/// SHA-256 해시가 비어있으면 검증 스킵, 있으면 검증
fn download_file_optional_hash(url: &str, dest: &Path, expected_hash: &str) -> Result<(), String> {
    if expected_hash.is_empty() {
        // 해시 미설정 → 직접 다운로드 (검증 스킵)
        // ⚠️ 보안 경고: 배포 빌드 전 반드시 SHA-256 해시를 채울 것
        #[cfg(not(debug_assertions))]
        tracing::error!(
            "⚠️ SECURITY: {} 의 SHA-256 해시가 미설정 — 무결성 검증 없이 다운로드합니다. 배포 전 반드시 해시를 설정하세요!",
            dest.display()
        );
        download_file_with_timeout(url, dest)?;
        // 다운로드 후 해시 출력 (해시 상수에 채울 값)
        if let Ok(hash) = compute_sha256(dest) {
            tracing::warn!(
                "SHA-256 해시 미설정 파일 다운로드 완료. 이 해시를 model_downloader.rs 상수에 채우세요:\n  파일: {}\n  SHA-256: {}",
                dest.display(),
                hash
            );
        }
        Ok(())
    } else {
        download_file_verified(url, dest, expected_hash)
    }
}

/// SHA-256 검증 포함 파일 다운로드
fn download_file_verified(url: &str, dest: &Path, expected_hash: &str) -> Result<(), String> {
    // 임시 파일에 다운로드
    let temp_path = dest.with_extension("tmp");

    // 다운로드
    download_file_with_timeout(url, &temp_path)?;

    // SHA-256 검증
    let actual_hash = compute_sha256(&temp_path)?;
    if actual_hash != expected_hash {
        // 검증 실패 - 파일 삭제
        let _ = fs::remove_file(&temp_path);
        return Err(format!(
            "무결성 검증 실패!\n예상: {}\n실제: {}\n\n파일이 변조되었거나 손상되었습니다. 보안을 위해 다운로드를 차단합니다.",
            expected_hash, actual_hash
        ));
    }

    // 검증 성공 - 최종 위치로 이동
    fs::rename(&temp_path, dest).map_err(|e| format!("파일 이동 실패: {}", e))?;

    tracing::info!("SHA-256 검증 성공: {}", dest.display());
    Ok(())
}

/// 기존 파일 무결성 검증 (선택적)
fn verify_existing_file(path: &Path, expected_hash: &str, name: &str) -> Result<(), String> {
    let actual_hash = compute_sha256(path)?;
    if actual_hash != expected_hash {
        tracing::warn!(
            "{} 무결성 불일치 - 예상: {}, 실제: {}. 파일을 삭제하고 재다운로드합니다.",
            name,
            expected_hash,
            actual_hash
        );
        // 손상된 파일 삭제 (다음 실행 시 재다운로드)
        let _ = fs::remove_file(path);
        return Err(format!(
            "{} 파일이 손상되었습니다. 앱을 재시작하여 다시 다운로드하세요.",
            name
        ));
    }
    Ok(())
}

/// SHA-256 해시 계산
fn compute_sha256(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| format!("파일 열기 실패: {}", e))?;

    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = file
            .read(&mut buffer)
            .map_err(|e| format!("파일 읽기 실패: {}", e))?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    let hash = hasher.finalize();
    Ok(format!("{:x}", hash))
}

/// 타임아웃 + 크기 제한 포함 파일 다운로드
fn download_file_with_timeout(url: &str, dest: &Path) -> Result<(), String> {
    let config = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(CONNECT_TIMEOUT_SECS)))
        .timeout_recv_body(Some(Duration::from_secs(READ_TIMEOUT_SECS)))
        .build();
    let agent = ureq::Agent::new_with_config(config);

    let response = agent
        .get(url)
        .call()
        .map_err(|e| format!("다운로드 실패 ({}): {}", url, e))?;

    // Content-Length 확인 (크기 제한)
    if let Some(content_length) = response.headers().get("Content-Length") {
        if let Ok(size) = content_length.to_str().unwrap_or("0").parse::<u64>() {
            if size > MAX_FILE_SIZE {
                return Err(format!(
                    "파일 크기 초과: {} bytes (최대 {} bytes)",
                    size, MAX_FILE_SIZE
                ));
            }
        }
    }

    let mut file = File::create(dest).map_err(|e| format!("파일 생성 실패: {}", e))?;

    let mut reader = response.into_body().into_reader();
    let mut total_bytes: u64 = 0;
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = reader
            .read(&mut buffer)
            .map_err(|e| format!("읽기 실패: {}", e))?;
        if bytes_read == 0 {
            break;
        }

        total_bytes += bytes_read as u64;
        if total_bytes > MAX_FILE_SIZE {
            let _ = fs::remove_file(dest);
            return Err(format!("다운로드 중 크기 제한 초과: {} bytes", total_bytes));
        }

        file.write_all(&buffer[..bytes_read])
            .map_err(|e| format!("파일 쓰기 실패: {}", e))?;
    }

    Ok(())
}

/// ONNX Runtime ZIP 다운로드 및 압축 해제
fn download_onnx_runtime(dest_dir: &Path) -> Result<(), String> {
    let config = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(CONNECT_TIMEOUT_SECS)))
        .timeout_recv_body(Some(Duration::from_secs(READ_TIMEOUT_SECS)))
        .build();
    let agent = ureq::Agent::new_with_config(config);

    let response = agent
        .get(ONNX_RUNTIME_URL)
        .call()
        .map_err(|e| format!("ONNX Runtime 다운로드 실패: {}", e))?;

    // 임시 파일에 저장
    let temp_path = dest_dir.join("onnxruntime_temp.zip");
    {
        let mut file =
            File::create(&temp_path).map_err(|e| format!("임시 파일 생성 실패: {}", e))?;

        let mut reader = response.into_body().into_reader().take(MAX_FILE_SIZE);
        io::copy(&mut reader, &mut file).map_err(|e| format!("파일 쓰기 실패: {}", e))?;
    }

    // SHA-256 무결성 검증
    let actual_hash = compute_sha256(&temp_path)?;
    if actual_hash != ONNX_RUNTIME_ZIP_SHA256 {
        let _ = fs::remove_file(&temp_path);
        return Err(format!(
            "ONNX Runtime ZIP 무결성 검증 실패!\n예상: {}\n실제: {}\n\n파일이 변조되었거나 손상되었습니다.",
            ONNX_RUNTIME_ZIP_SHA256, actual_hash
        ));
    }
    tracing::info!("ONNX Runtime ZIP SHA-256 검증 성공");

    // ZIP 압축 해제
    let file = File::open(&temp_path).map_err(|e| format!("ZIP 파일 열기 실패: {}", e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("ZIP 아카이브 열기 실패: {}", e))?;

    // onnxruntime.dll 찾아서 추출
    let dll_name = "onnxruntime.dll";
    let mut dll_found = false;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("ZIP 엔트리 읽기 실패: {}", e))?;

        let name = file.name().to_string();
        if name.ends_with(dll_name) {
            let dest_path = dest_dir.join(dll_name);
            let mut dest_file =
                File::create(&dest_path).map_err(|e| format!("DLL 파일 생성 실패: {}", e))?;

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
    let e5_dir = models_dir.join("kosimcse-roberta-multitask");
    let model_exists =
        e5_dir.join("model_int8.onnx").exists() || e5_dir.join("model.onnx").exists();
    (
        e5_dir.join("onnxruntime.dll").exists(),
        model_exists,
        e5_dir.join("tokenizer.json").exists(),
    )
}

