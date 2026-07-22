//! 모델 자동 다운로드 모듈
//!
//! ONNX Runtime과 임베딩 모델을 자동으로 다운로드합니다.
//! SHA-256 무결성 검증을 수행합니다.

use sha2::{Digest, Sha256};
use std::fs::{self, File};
#[cfg(target_os = "windows")]
use std::io;
use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

// ============================================================================
// 모델 URL 및 SHA-256 해시 (무결성 검증용)
// ============================================================================

/// 플랫폼별 ONNX Runtime 동적 라이브러리 파일명.
/// `ORT_DYLIB_PATH` 환경변수와 번들/다운로드 경로에 일관 사용된다.
pub fn dylib_filename() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "onnxruntime.dll"
    }
    #[cfg(target_os = "macos")]
    {
        "libonnxruntime.dylib"
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        "libonnxruntime.so"
    }
}

/// 플랫폼별 pdfium 동적 라이브러리 파일명.
/// bblanchon/pdfium-binaries 아카이브 내부 명칭 및 `PDFIUM_DYLIB_PATH` 경로에 일관 사용된다.
pub fn pdfium_lib_filename() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "pdfium.dll"
    }
    #[cfg(target_os = "macos")]
    {
        "libpdfium.dylib"
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        "libpdfium.so"
    }
}

// ort 2.0.0-rc.11 은 ONNX Runtime **>= 1.23.x** 를 요구한다.
// v1.20.1 을 쓰던 구 빌드에서 업그레이드하면 DLL 버전 불일치로 부팅 단계 panic 발생.
// URL 갱신 시 반드시 ZIP/DLL SHA-256 을 함께 갱신해야 한다.
#[cfg(target_os = "windows")]
const ONNX_RUNTIME_URL: &str = "https://github.com/microsoft/onnxruntime/releases/download/v1.23.0/onnxruntime-win-x64-1.23.0.zip";
// KoSimCSE-roberta-multitask (HuggingFace) — INT8 동적 양자화 모델
const KOSIMCSE_MODEL_URL: &str =
    "https://huggingface.co/chrisryugj/kosimcse-roberta-multitask-onnx/resolve/main/model_int8.onnx";
const KOSIMCSE_TOKENIZER_URL: &str =
    "https://huggingface.co/chrisryugj/kosimcse-roberta-multitask-onnx/resolve/main/tokenizer.json";
// F32 원본 URL (하위 호환용, 현재 미사용)
#[allow(dead_code)]
const KOSIMCSE_MODEL_F32_URL: &str =
    "https://huggingface.co/chrisryugj/kosimcse-roberta-multitask-onnx/resolve/main/model.onnx";
#[allow(dead_code)]
const KOSIMCSE_MODEL_DATA_URL: &str = "https://huggingface.co/chrisryugj/kosimcse-roberta-multitask-onnx/resolve/main/model.onnx.data";

// SHA-256 해시 (무결성 검증)
// 주의: 모델 버전 업데이트 시 해시값도 업데이트 필요
// INT8 양자화 모델 SHA-256
const KOSIMCSE_MODEL_SHA256: &str =
    "877e43d3f3a2ee09a58c08a0d1720f99b3496962e92569c5846299f862ac0f33";
// F32 원본 해시 (하위 호환용, 현재 미사용)
#[allow(dead_code)]
const KOSIMCSE_MODEL_F32_SHA256: &str =
    "a1e12d33caecc60aa192fa1bb56a5a7a4d817486e7420e38662acc6e1c357b5d";
#[allow(dead_code)]
const KOSIMCSE_MODEL_DATA_SHA256: &str =
    "98691c75a2129885f4a9da144749d0a97c36d2c7a0d425559463046eadb2de9f";
const KOSIMCSE_TOKENIZER_SHA256: &str =
    "d607daae73f6a05440b09833097b34c3f6eea3a53d6ab010a6c0c07081f0a5ab";
// ONNX Runtime ZIP SHA-256 (v1.23.0 win-x64, Microsoft 공식 릴리스 78,078,377 바이트)
#[cfg(target_os = "windows")]
const ONNX_RUNTIME_ZIP_SHA256: &str =
    "72c23470310ec79a7d42d27fe9d257e6c98540c73fa5a1db1f67f538c6c16f2f";

// 추출된 onnxruntime.dll SHA-256 (v1.23.0 win-x64, 14,197,760 바이트, 빌드 1.23.20250925.2.be835ef)
// ZIP 해시만 검증하면 추출 중 손상/다른 소스 교체를 못 잡으므로 DLL 본체도 검증한다.
#[cfg(target_os = "windows")]
const ONNX_RUNTIME_DLL_SHA256: &str =
    "b4b7f9aed3cf6b04000f595bddcbdf12e87214bc401d1b81beadae3dbf28d2bd";

// PaddleOCR ONNX 모델 (Hugging Face: monkt/paddleocr-onnx)
const OCR_DET_URL: &str =
    "https://huggingface.co/monkt/paddleocr-onnx/resolve/main/detection/v3/det.onnx";
const OCR_REC_KO_URL: &str =
    "https://huggingface.co/monkt/paddleocr-onnx/resolve/main/languages/korean/rec.onnx";
const OCR_DICT_KO_URL: &str =
    "https://huggingface.co/monkt/paddleocr-onnx/resolve/main/languages/korean/dict.txt";
const OCR_DET_SHA256: &str = "ee40e80071ba3a320d4efda75f3e22047a7d049e9bf7bcaaf9daea23fc21b935";
const OCR_REC_KO_SHA256: &str = "322f140154c820fcb83c3d24cfe42c9ec70dd1a1834163306a7338136e4f1eaa";
const OCR_DICT_KO_SHA256: &str = "a88071c68c01707489baa79ebe0405b7beb5cca229f4fc94cc3ef992328802d7";

// PP-DocLayout-M — 문서 레이아웃 검출 ONNX (GreatV/oar-ocr v0.3.0 릴리스, Apache-2.0, 22.4MB).
// 스캔/이미지 OCR 텍스트 영역을 제목·표·머리글/바닥글 등으로 분류. 선택적 모델(부재 시 미로드).
const OCR_LAYOUT_URL: &str =
    "https://github.com/GreatV/oar-ocr/releases/download/v0.3.0/pp-doclayout-m.onnx";
const OCR_LAYOUT_SHA256: &str = "8e458bfc919bbf7a35be9802485b5cd30151cb356364cfad09911d2ee1fc1f76";

// 다운로드 설정
const CONNECT_TIMEOUT_SECS: u64 = 30;
const READ_TIMEOUT_SECS: u64 = 600; // 10분 (대용량 모델)
const MAX_FILE_SIZE: u64 = 600 * 1024 * 1024; // 600MB 상한

/// 오프라인(망분리) 배포용 강제 스위치.
///
/// `DOCUFINDER_OFFLINE=1`(또는 `true`) 이면 모든 런타임 네트워크 다운로드를 즉시 차단한다.
/// 정상 배포는 필수 자산(onnxruntime·paddleocr·pdfium)을 MSI 에 번들하므로 `seed_bundled_models`
/// 가 먼저 채워 다운로드 경로 자체를 타지 않지만, seed 실패 같은 엣지에서 네트워크를 건드리면
/// 내부망 EDR(ZombieZERO 등) 이 "실행 중 외부 바이너리 다운로드" 를 dropper 로 오탐해 프로세스를
/// 격리한다. 이 스위치는 그 마지막 네트워크 시도까지 원천 봉쇄한다. 호출부는 반환된 Err 를
/// graceful 처리(기능만 비활성, 앱은 계속 부팅)하므로 부작용이 없다.
fn offline_mode() -> bool {
    matches!(
        std::env::var("DOCUFINDER_OFFLINE").ok().as_deref(),
        Some("1") | Some("true") | Some("TRUE")
    )
}

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
    let model_dir = models_dir.join("kosimcse-roberta-multitask");
    fs::create_dir_all(&model_dir).map_err(|e| format!("디렉토리 생성 실패: {}", e))?;

    let model_int8_path = model_dir.join("model_int8.onnx");
    let model_f32_path = model_dir.join("model.onnx");
    let tokenizer_path = model_dir.join("tokenizer.json");

    let mut result = DownloadResult {
        onnx_runtime_downloaded: false,
        model_downloaded: false,
        model_data_downloaded: false,
        tokenizer_downloaded: false,
    };

    // ONNX Runtime DLL — 존재해도 SHA-256 을 검증해 구버전(v1.20.1 등) 잔재를 강제로 교체.
    // 기존엔 존재 여부만 확인했기 때문에 ort 크레이트 업그레이드 후에도 구 DLL 을 계속 로드해
    // 부팅 단계에서 "ort 2.x is not compatible ... got '1.20.1'" panic 이 발생했다.
    // macOS/Linux: 다운로드 미지원 — 번들 dylib 사용 (seed_bundled_models 가 처리).
    #[cfg(target_os = "windows")]
    {
        let dll_path = model_dir.join(dylib_filename());
        if needs_dll_replacement(&dll_path) {
            if dll_path.exists() {
                tracing::warn!(
                    "ONNX Runtime DLL 해시 불일치 감지 — 구버전 제거 후 재다운로드합니다: {}",
                    dll_path.display()
                );
                let _ = fs::remove_file(&dll_path);
            } else {
                tracing::info!("ONNX Runtime 다운로드 중...");
            }
            download_onnx_runtime(&model_dir)?;
            result.onnx_runtime_downloaded = true;
            tracing::info!("ONNX Runtime 다운로드 완료");
        }
    }

    // INT8 양자화 모델 다운로드 (SHA-256 검증)
    if !model_int8_path.exists() {
        tracing::info!("임베딩 모델 다운로드 중 (model_int8.onnx, ~106MB)...");
        download_file_verified(KOSIMCSE_MODEL_URL, &model_int8_path, KOSIMCSE_MODEL_SHA256)?;
        result.model_downloaded = true;
        tracing::info!("임베딩 모델(INT8) 다운로드 및 검증 완료");
    } else {
        verify_existing_file(&model_int8_path, KOSIMCSE_MODEL_SHA256, "임베딩 모델(INT8)")?;
    }

    // F32 → INT8 마이그레이션: INT8 다운로드 성공 후 F32 원본 삭제 (RAM 절약)
    if model_int8_path.exists() && model_f32_path.exists() {
        let model_data_path = model_dir.join("model.onnx.data");
        tracing::info!("F32 원본 모델 삭제 중 (INT8로 교체 완료)...");
        let _ = fs::remove_file(&model_f32_path);
        let _ = fs::remove_file(&model_data_path);
        tracing::info!("F32 모델 삭제 완료 (~840MB 절약)");
    }

    // 토크나이저 다운로드 (SHA-256 검증)
    if !tokenizer_path.exists() {
        tracing::info!("토크나이저 다운로드 중...");
        download_file_verified(
            KOSIMCSE_TOKENIZER_URL,
            &tokenizer_path,
            KOSIMCSE_TOKENIZER_SHA256,
        )?;
        result.tokenizer_downloaded = true;
        tracing::info!("토크나이저 다운로드 및 검증 완료");
    } else {
        verify_existing_file(&tokenizer_path, KOSIMCSE_TOKENIZER_SHA256, "토크나이저")?;
    }

    Ok(result)
}

/// 기존 DLL 이 없거나 SHA-256 이 기대값과 다르면 true.
/// 구버전 ONNX Runtime 이 디스크에 남아 ort::init 시 panic 을 유발하는 경로를 차단한다.
#[cfg(target_os = "windows")]
fn needs_dll_replacement(dll_path: &Path) -> bool {
    if !dll_path.exists() {
        return true;
    }
    match compute_sha256(dll_path) {
        Ok(hash) if hash == ONNX_RUNTIME_DLL_SHA256 => false,
        Ok(hash) => {
            tracing::warn!(
                "ONNX Runtime DLL SHA 불일치: 기대 {}, 실제 {}",
                ONNX_RUNTIME_DLL_SHA256,
                hash
            );
            true
        }
        Err(e) => {
            tracing::warn!("ONNX Runtime DLL SHA 계산 실패 (교체 진행): {}", e);
            true
        }
    }
}

/// setup() 에서 sync 로 호출해 ONNX Runtime DLL 을 먼저 준비한다.
/// 정상이면 즉시 반환, 구버전/손상이면 삭제 후 새 버전 다운로드(~14MB).
/// 실패 시 Err 를 반환하지만 app 은 계속 부팅된다(시맨틱/OCR 기능 비활성).
#[cfg(target_os = "windows")]
pub fn ensure_onnx_runtime_dll(models_dir: &Path) -> Result<(), String> {
    let model_dir = models_dir.join("kosimcse-roberta-multitask");
    fs::create_dir_all(&model_dir).map_err(|e| format!("디렉토리 생성 실패: {}", e))?;
    let dll_path = model_dir.join(dylib_filename());

    if !needs_dll_replacement(&dll_path) {
        return Ok(());
    }

    if dll_path.exists() {
        tracing::warn!(
            "구버전 ONNX Runtime DLL 감지 → 삭제 후 v1.23.0 다운로드 진행: {}",
            dll_path.display()
        );
        let _ = fs::remove_file(&dll_path);
    }
    download_onnx_runtime(&model_dir)
}

/// macOS/Linux: 자동 다운로드 미지원 — 번들 dylib 검증만.
/// `setup-macos-resources.sh` 가 빌드 시점에 `resources/onnxruntime/<DYLIB>` 를 채워두고,
/// `seed_bundled_models` 가 첫 실행에 `models/kosimcse-roberta-multitask/` 로 복사한다.
#[cfg(not(target_os = "windows"))]
pub fn ensure_onnx_runtime_dll(models_dir: &Path) -> Result<(), String> {
    let dylib_path = models_dir
        .join("kosimcse-roberta-multitask")
        .join(dylib_filename());
    if dylib_path.exists() {
        Ok(())
    } else {
        Err(format!(
            "ONNX Runtime dylib 미발견: {} — 번들 리소스에 포함되었는지 확인하세요.",
            dylib_path.display()
        ))
    }
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
    } else if !OCR_DET_SHA256.is_empty() {
        verify_existing_file(&det_path, OCR_DET_SHA256, "OCR Detection")?;
    }

    if !rec_path.exists() {
        tracing::info!("OCR Recognition (한국어) 모델 다운로드 중...");
        download_file_optional_hash(OCR_REC_KO_URL, &rec_path, OCR_REC_KO_SHA256)?;
        rec_downloaded = true;
        tracing::info!("OCR Recognition 모델 다운로드 완료");
    } else if !OCR_REC_KO_SHA256.is_empty() {
        verify_existing_file(&rec_path, OCR_REC_KO_SHA256, "OCR Recognition")?;
    }

    if !dict_path.exists() {
        tracing::info!("OCR 한국어 사전 다운로드 중...");
        download_file_optional_hash(OCR_DICT_KO_URL, &dict_path, OCR_DICT_KO_SHA256)?;
        dict_downloaded = true;
        tracing::info!("OCR 한국어 사전 다운로드 완료");
    } else if !OCR_DICT_KO_SHA256.is_empty() {
        verify_existing_file(&dict_path, OCR_DICT_KO_SHA256, "OCR Dictionary")?;
    }

    Ok((det_downloaded, rec_downloaded, dict_downloaded))
}

/// PP-DocLayout 레이아웃 모델(선택) 다운로드. OCR 모델과 같은 `paddleocr/` 아래 `layout.onnx`
/// 로 둔다. 없으면 다운로드, 있으면 SHA 검증. 이 모델은 `OcrEngine` 이 지연 로드하며 부재 시
/// OCR 은 레이아웃 분석 없이 동작한다(기능만 off). 새로 받았으면 `true`.
pub fn ensure_layout_model(models_dir: &Path) -> Result<bool, String> {
    let ocr_dir = models_dir.join("paddleocr");
    fs::create_dir_all(&ocr_dir).map_err(|e| format!("OCR 디렉토리 생성 실패: {}", e))?;
    let layout_path = ocr_dir.join("layout.onnx");

    if layout_path.exists() {
        if !OCR_LAYOUT_SHA256.is_empty() {
            verify_existing_file(&layout_path, OCR_LAYOUT_SHA256, "OCR Layout")?;
        }
        return Ok(false);
    }

    tracing::info!("PP-DocLayout 레이아웃 모델 다운로드 중...");
    download_file_optional_hash(OCR_LAYOUT_URL, &layout_path, OCR_LAYOUT_SHA256)?;
    tracing::info!("PP-DocLayout 레이아웃 모델 다운로드 완료");
    Ok(true)
}

/// 스캔/이미지 PDF 페이지 래스터화용 pdfium 동적 라이브러리를 확인하고 없으면 다운로드.
///
/// bblanchon/pdfium-binaries 의 플랫폼별 `.tgz` 를 받아 내부 dylib 한 개만
/// `models/pdfium/<파일명>` 으로 추출한다. `PDFIUM_DOWNLOAD_SHA256` 이 채워져 있으면 tgz
/// 무결성을 검증하고, 빈 문자열이면 스킵한다. URL 이 빈 미지원 플랫폼이면 `Ok(false)` 로 조용히 스킵.
///
/// 반환값: 이번 호출에서 새로 다운로드/추출했으면 true. 실패해도 앱은 계속 동작하며 스캔
/// PDF 래스터화 fallback 만 비활성된다(호출부에서 warn 처리).
///
/// ⚠️ mac-arm64 만 실제 다운로드·추출까지 검증됨. 나머지 플랫폼은 URL·SHA 만 채워둔 미검증 상태.
pub fn ensure_pdfium(models_dir: &Path) -> Result<bool, String> {
    let url = crate::constants::PDFIUM_DOWNLOAD_URL;
    let expected_hash = crate::constants::PDFIUM_DOWNLOAD_SHA256;

    if url.is_empty() {
        tracing::debug!("pdfium 다운로드 미지원 플랫폼 — 스캔 PDF 래스터화 fallback 비활성");
        return Ok(false);
    }

    let pdfium_dir = models_dir.join("pdfium");
    fs::create_dir_all(&pdfium_dir).map_err(|e| format!("pdfium 디렉토리 생성 실패: {}", e))?;
    let lib_path = pdfium_dir.join(pdfium_lib_filename());
    if lib_path.exists() {
        return Ok(false); // 이미 존재
    }

    tracing::info!("pdfium 동적 라이브러리 다운로드 중 (스캔 PDF OCR)...");

    // .tgz 아카이브를 임시 파일로 받는다
    let tgz_path = pdfium_dir.join("pdfium.tgz.tmp");
    download_file_with_timeout(url, &tgz_path)?;

    // 선택적 SHA-256 검증 (아카이브 본체)
    if !expected_hash.is_empty() {
        let actual = compute_sha256(&tgz_path)?;
        if actual != expected_hash {
            let _ = fs::remove_file(&tgz_path);
            return Err(format!(
                "pdfium 아카이브 무결성 검증 실패!\n예상: {}\n실제: {}",
                expected_hash, actual
            ));
        }
        tracing::info!("pdfium 아카이브 SHA-256 검증 성공");
    }

    // gzip+tar 에서 dylib 한 개만 추출
    let extracted = extract_pdfium_from_tgz(&tgz_path, pdfium_lib_filename(), &lib_path);
    let _ = fs::remove_file(&tgz_path);
    extracted?;

    tracing::info!("pdfium 준비 완료: {}", lib_path.display());
    Ok(true)
}

/// pdfium `.tgz`(gzip+tar) 에서 파일명이 `lib_filename` 로 끝나는 엔트리 하나를 `dest` 로 추출.
///
/// tar 헤더(512바이트 블록)를 직접 스캔한다 — pdfium 아카이브의 경로는 짧아 ustar name 필드
/// (100바이트) 안에 들어가므로 GNU long-name/PAX 확장을 다룰 필요가 없다. tar 크레이트 의존성을
/// 추가하지 않고 이미 있는 flate2(gzip)만으로 처리한다.
fn extract_pdfium_from_tgz(tgz_path: &Path, lib_filename: &str, dest: &Path) -> Result<(), String> {
    use flate2::read::GzDecoder;

    let file = File::open(tgz_path).map_err(|e| format!("tgz 열기 실패: {}", e))?;

    // gzip 해제된 tar 전체를 메모리로 (pdfium tar ≈ 8MB, 상한 이내)
    const MAX_TAR_SIZE: u64 = 64 * 1024 * 1024;
    let mut tar = Vec::new();
    GzDecoder::new(file)
        .take(MAX_TAR_SIZE)
        .read_to_end(&mut tar)
        .map_err(|e| format!("gzip 해제 실패: {}", e))?;

    let mut pos = 0usize;
    while pos + 512 <= tar.len() {
        let header = &tar[pos..pos + 512];
        // 이름 첫 바이트가 0 이면 아카이브 종료(zero-block)
        if header[0] == 0 {
            break;
        }
        // name: 0..100 (NUL 종료), size(octal ASCII): 124..136
        let name_end = header[..100].iter().position(|&b| b == 0).unwrap_or(100);
        let name = String::from_utf8_lossy(&header[..name_end]);
        let size = parse_tar_octal(&header[124..136]);
        let data_start = pos + 512;
        let data_end = data_start + size as usize;
        if data_end > tar.len() {
            break; // 손상된 아카이브
        }
        if name.ends_with(lib_filename) {
            // 원자적 쓰기: 임시 파일 → rename(같은 디렉토리라 원자적). 디스크풀·중단으로 부분
            // 파일이 최종 경로에 남아 bind_pdfium 이 손상된 dylib 를 로드하는 것을 방지한다.
            let tmp = dest.with_extension("part");
            fs::write(&tmp, &tar[data_start..data_end])
                .map_err(|e| format!("pdfium 추출 쓰기 실패: {}", e))?;
            // 실행 권한 부여 (dylib 로드 자체엔 불필요하나 관례상)
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755));
            }
            fs::rename(&tmp, dest).map_err(|e| {
                let _ = fs::remove_file(&tmp);
                format!("pdfium 최종 배치(rename) 실패: {}", e)
            })?;
            return Ok(());
        }
        // 다음 헤더: 512(헤더) + 데이터(512 정렬)
        pos = data_start + (size.div_ceil(512) * 512) as usize;
    }

    Err(format!(
        "pdfium 아카이브에서 {} 를 찾을 수 없습니다",
        lib_filename
    ))
}

/// tar 헤더의 8진수 ASCII 필드(공백/NUL 패딩)를 u64 로 파싱.
fn parse_tar_octal(bytes: &[u8]) -> u64 {
    let s = String::from_utf8_lossy(bytes);
    let trimmed = s.trim_matches(|c| c == ' ' || c == '\0');
    u64::from_str_radix(trimmed, 8).unwrap_or(0)
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
    if offline_mode() {
        return Err(format!(
            "오프라인 모드(DOCUFINDER_OFFLINE) — 네트워크 다운로드 차단됨: {}",
            url
        ));
    }
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

/// ONNX Runtime ZIP 다운로드 및 압축 해제 (Windows 전용)
#[cfg(target_os = "windows")]
fn download_onnx_runtime(dest_dir: &Path) -> Result<(), String> {
    if offline_mode() {
        return Err("오프라인 모드(DOCUFINDER_OFFLINE) — ONNX Runtime 다운로드 차단됨".to_string());
    }
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

    // 추출된 DLL 무결성 검증 — ZIP 해시만 믿으면 추출 과정의 손상/중간 변조를 못 잡는다.
    let dll_path = dest_dir.join(dll_name);
    let actual_dll_hash = compute_sha256(&dll_path)?;
    if actual_dll_hash != ONNX_RUNTIME_DLL_SHA256 {
        let _ = fs::remove_file(&dll_path);
        return Err(format!(
            "ONNX Runtime DLL 무결성 검증 실패!\n예상: {}\n실제: {}",
            ONNX_RUNTIME_DLL_SHA256, actual_dll_hash
        ));
    }
    tracing::info!("ONNX Runtime DLL SHA-256 검증 성공 (v1.23.0)");

    Ok(())
}

/// 번들된 리소스(ONNX Runtime DLL + PaddleOCR 3종)를 APPDATA/models/ 로 복사한다.
/// MSI 설치본은 이 4개를 함께 배포하므로 사용자가 huggingface/github 다운로드를 못 해도
/// 첫 실행 즉시 시맨틱·OCR 가능해진다. 실패해도 panic 하지 않고 기존 다운로드 경로로 자연 fallback.
pub fn seed_bundled_models(resource_dir: &Path, models_dir: &Path) {
    let bundled_root = resource_dir.join("resources");

    // ONNX Runtime dylib → models/kosimcse-roberta-multitask/<DYLIB>
    let dll_dest_dir = models_dir.join("kosimcse-roberta-multitask");
    if let Err(e) = fs::create_dir_all(&dll_dest_dir) {
        tracing::warn!("seed: ONNX Runtime 디렉토리 생성 실패: {}", e);
    } else {
        let lib = dylib_filename();
        // Windows 만 SHA-256 본체 해시를 검증 — macOS dylib 해시는 미고정.
        #[cfg(target_os = "windows")]
        let expected_hash = ONNX_RUNTIME_DLL_SHA256;
        #[cfg(not(target_os = "windows"))]
        let expected_hash = "";

        seed_one(
            &bundled_root.join("onnxruntime").join(lib),
            &dll_dest_dir.join(lib),
            expected_hash,
            "ONNX Runtime dylib",
        );
    }

    // PaddleOCR 3종 → models/paddleocr/*
    let ocr_dest_dir = models_dir.join("paddleocr");
    if let Err(e) = fs::create_dir_all(&ocr_dest_dir) {
        tracing::warn!("seed: OCR 디렉토리 생성 실패: {}", e);
        return;
    }
    for (name, hash, label) in [
        ("det.onnx", OCR_DET_SHA256, "OCR Detection"),
        ("rec.onnx", OCR_REC_KO_SHA256, "OCR Recognition (ko)"),
        ("dict.txt", OCR_DICT_KO_SHA256, "OCR Dictionary (ko)"),
    ] {
        seed_one(
            &bundled_root.join("paddleocr").join(name),
            &ocr_dest_dir.join(name),
            hash,
            label,
        );
    }

    // pdfium dylib → models/pdfium/<lib> (스캔 PDF 래스터화 fallback).
    // 번들(resources/pdfium/)에 있으면 복사, 없으면 no-op → ensure_pdfium 다운로드 fallback.
    let pdfium_dest_dir = models_dir.join("pdfium");
    if let Err(e) = fs::create_dir_all(&pdfium_dest_dir) {
        tracing::warn!("seed: pdfium 디렉토리 생성 실패: {}", e);
    } else {
        let lib = pdfium_lib_filename();
        seed_one(
            &bundled_root.join("pdfium").join(lib),
            &pdfium_dest_dir.join(lib),
            "", // pdfium dylib 해시 미고정
            "pdfium dylib",
        );
    }
}

/// 번들 파일 1개를 dest 로 복사. 이미 동일 해시면 skip, 해시 다르면 덮어씀.
/// 번들 원본이 없거나 복사 실패해도 warn 만 남기고 진행 — 다운로드 fallback 이 따라온다.
fn seed_one(src: &Path, dest: &Path, expected_hash: &str, label: &str) {
    if !src.exists() {
        tracing::debug!(
            "seed: 번들 {} 미발견({:?}) → 다운로드 fallback 사용",
            label,
            src.display()
        );
        return;
    }

    if dest.exists() {
        if let Ok(h) = compute_sha256(dest) {
            if h == expected_hash {
                return; // 이미 최신
            }
        }
        let _ = fs::remove_file(dest);
    }

    match fs::copy(src, dest) {
        Ok(bytes) => tracing::info!(
            "seed: 번들 {} 적용 완료 ({} bytes) → {}",
            label,
            bytes,
            dest.display()
        ),
        Err(e) => tracing::warn!("seed: 번들 {} 복사 실패: {}", label, e),
    }
}
