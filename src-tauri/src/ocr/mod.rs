//! PaddleOCR ONNX 기반 OCR 엔진
//!
//! Detection (DBNet) → Crop → Recognition (SVTR) → CTC 디코딩

mod detection;
mod geometry;
mod recognition;

use image::DynamicImage;
use ort::session::Session;
use std::path::Path;
use std::sync::Mutex;

pub use recognition::RecognitionResult;

/// OCR 에러
#[derive(Debug, thiserror::Error)]
pub enum OcrError {
    #[error("Model load error: {0}")]
    ModelLoad(String),
    #[error("Image load error: {0}")]
    ImageLoad(String),
    #[error("Inference error: {0}")]
    Inference(String),
}

/// OCR 결과
#[derive(Debug, Clone)]
pub struct OcrResult {
    /// 전체 텍스트 (읽기 순서로 결합)
    pub text: String,
    /// 각 텍스트 영역별 결과
    pub regions: Vec<OcrRegion>,
    /// 평균 신뢰도
    pub confidence: f32,
}

#[derive(Debug, Clone)]
pub struct OcrRegion {
    pub text: String,
    pub confidence: f32,
}

/// PaddleOCR ONNX 엔진
pub struct OcrEngine {
    det_session: Mutex<Session>,
    rec_session: Mutex<Session>,
    dictionary: Vec<String>,
}

impl OcrEngine {
    /// 모델 디렉토리에서 OcrEngine 초기화
    ///
    /// models_dir에 det.onnx, rec.onnx, dict.txt가 존재해야 함
    pub fn new(models_dir: &Path) -> Result<Self, OcrError> {
        let det_path = models_dir.join("det.onnx");
        let rec_path = models_dir.join("rec.onnx");
        let dict_path = models_dir.join("dict.txt");

        if !det_path.exists() {
            return Err(OcrError::ModelLoad(format!("Detection model not found: {:?}", det_path)));
        }
        if !rec_path.exists() {
            return Err(OcrError::ModelLoad(format!("Recognition model not found: {:?}", rec_path)));
        }
        if !dict_path.exists() {
            return Err(OcrError::ModelLoad(format!("Dictionary not found: {:?}", dict_path)));
        }

        // ort 세션 생성 (embedder/mod.rs 패턴)
        let num_threads = std::thread::available_parallelism()
            .map(|n| n.get().clamp(2, 4))
            .unwrap_or(2);

        let det_session = Session::builder()
            .map_err(|e| OcrError::ModelLoad(e.to_string()))?
            .with_execution_providers([ort::ep::CPU::default().build()])
            .map_err(|e| OcrError::ModelLoad(e.to_string()))?
            .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
            .map_err(|e| OcrError::ModelLoad(e.to_string()))?
            .with_intra_threads(num_threads)
            .map_err(|e| OcrError::ModelLoad(e.to_string()))?
            .commit_from_file(&det_path)
            .map_err(|e| OcrError::ModelLoad(format!("Detection session: {}", e)))?;

        let rec_session = Session::builder()
            .map_err(|e| OcrError::ModelLoad(e.to_string()))?
            .with_execution_providers([ort::ep::CPU::default().build()])
            .map_err(|e| OcrError::ModelLoad(e.to_string()))?
            .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
            .map_err(|e| OcrError::ModelLoad(e.to_string()))?
            .with_intra_threads(num_threads)
            .map_err(|e| OcrError::ModelLoad(e.to_string()))?
            .commit_from_file(&rec_path)
            .map_err(|e| OcrError::ModelLoad(format!("Recognition session: {}", e)))?;

        // 사전 로드
        let dict_content = std::fs::read_to_string(&dict_path)
            .map_err(|e| OcrError::ModelLoad(format!("Dictionary read: {}", e)))?;
        let dictionary: Vec<String> = dict_content.lines().map(|l| l.to_string()).collect();

        tracing::info!(
            "OCR engine initialized: det={:?}, rec={:?}, dict={} chars",
            det_path.file_name(),
            rec_path.file_name(),
            dictionary.len()
        );

        Ok(Self {
            det_session: Mutex::new(det_session),
            rec_session: Mutex::new(rec_session),
            dictionary,
        })
    }

    /// 이미지 파일에서 텍스트 추출
    pub fn recognize_file(&self, image_path: &Path) -> Result<OcrResult, OcrError> {
        let image = image::open(image_path)
            .map_err(|e| OcrError::ImageLoad(format!("{}: {}", image_path.display(), e)))?;
        self.recognize_image(&image)
    }

    /// DynamicImage에서 텍스트 추출 (PDF 렌더링용)
    pub fn recognize_image(&self, image: &DynamicImage) -> Result<OcrResult, OcrError> {
        // 1. Detection: 텍스트 영역 검출
        let boxes = detection::detect(&self.det_session, image)?;

        if boxes.is_empty() {
            return Ok(OcrResult {
                text: String::new(),
                regions: vec![],
                confidence: 0.0,
            });
        }

        // 2. Crop: 각 영역을 잘라내기
        let crops: Vec<image::RgbImage> = boxes
            .iter()
            .map(|quad| geometry::crop_quad(image, quad))
            .collect();

        // 3. Recognition: 각 crop에서 텍스트 인식
        let rec_results = recognition::recognize_batch(
            &self.rec_session,
            &crops,
            &self.dictionary,
        )?;

        // 4. 결과 조합
        let regions: Vec<OcrRegion> = rec_results
            .iter()
            .filter(|r| !r.text.trim().is_empty())
            .map(|r| OcrRegion {
                text: r.text.clone(),
                confidence: r.confidence,
            })
            .collect();

        let text = regions
            .iter()
            .map(|r| r.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        let avg_confidence = if regions.is_empty() {
            0.0
        } else {
            regions.iter().map(|r| r.confidence).sum::<f32>() / regions.len() as f32
        };

        Ok(OcrResult {
            text,
            regions,
            confidence: avg_confidence,
        })
    }
}
