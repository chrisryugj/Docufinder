//! PaddleOCR ONNX 기반 OCR 엔진
//!
//! Detection (DBNet) → Crop → Recognition (SVTR) → CTC 디코딩

mod detection;
mod geometry;
mod reading_order;
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
    /// 축 정렬 바운딩 박스 (Quad 에서 유도) — 읽기순서·레이아웃 분석의 전제
    pub bbox: BBox,
    /// 레이아웃 분류 (Phase 0 는 항상 Text, Phase 1 에서 PP-DocLayout 로 채움)
    pub kind: RegionKind,
}

/// 축 정렬 바운딩 박스 (이미지 좌표계, y 아래로 증가)
#[derive(Debug, Clone, Copy)]
pub struct BBox {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl BBox {
    /// Quad(4점, 축 정렬 직사각형: 좌상→우상→우하→좌하)에서 bbox 유도
    fn from_quad(q: &geometry::Quad) -> Self {
        Self {
            x0: q.points[0].0,
            y0: q.points[0].1,
            x1: q.points[2].0,
            y1: q.points[2].1,
        }
    }
}

/// 레이아웃 영역 분류 — Phase 1(PP-DocLayout)에서 채워질 forward 선언.
/// Text 외 variant 는 아직 구성되지 않으므로 dead_code 를 허용한다.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum RegionKind {
    #[default]
    Text,
    Title,
    List,
    Table,
    Figure,
    Caption,
    Header,
    Footer,
    PageNumber,
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
            return Err(OcrError::ModelLoad(format!(
                "Detection model not found: {:?}",
                det_path
            )));
        }
        if !rec_path.exists() {
            return Err(OcrError::ModelLoad(format!(
                "Recognition model not found: {:?}",
                rec_path
            )));
        }
        if !dict_path.exists() {
            return Err(OcrError::ModelLoad(format!(
                "Dictionary not found: {:?}",
                dict_path
            )));
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
        let rec_results =
            recognition::recognize_batch(&self.rec_session, &crops, &self.dictionary)?;

        // 4. 결과 조합 — box 좌표를 OcrRegion.bbox 로 보존 (읽기순서·레이아웃의 전제).
        //    boxes[i] ↔ crops[i] ↔ rec_results[i] 는 인덱스 1:1 대응.
        let regions: Vec<OcrRegion> = rec_results
            .iter()
            .zip(boxes.iter())
            .filter(|(r, _)| !r.text.trim().is_empty())
            .map(|(r, quad)| OcrRegion {
                text: r.text.clone(),
                confidence: r.confidence,
                bbox: BBox::from_quad(quad),
                kind: RegionKind::Text,
            })
            .collect();

        // 5. 읽기순서(XY-Cut) 재정렬 — 검출 순서가 아닌 사람이 읽는 순서로 본문 조립.
        //    다단(multi-column) 스캔본에서 좌우 텍스트가 행 단위로 뒤섞이는 문제를 해소.
        let bboxes: Vec<BBox> = regions.iter().map(|r| r.bbox).collect();
        let order = reading_order::reading_order(&bboxes);
        let regions: Vec<OcrRegion> = order.iter().map(|&i| regions[i].clone()).collect();

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
