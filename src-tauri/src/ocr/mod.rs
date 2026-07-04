//! PaddleOCR ONNX 기반 OCR 엔진
//!
//! Detection (DBNet) → Crop → Recognition (SVTR) → CTC 디코딩

mod detection;
mod geometry;
mod layout;
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

/// 레이아웃 영역 분류(PP-DocLayout). 대부분의 variant 는 `layout::class_to_kind` 가 구성하며,
/// `List` 는 현재 어떤 클래스에도 매핑되지 않아 dead_code 를 허용한다.
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
    /// 선택적 PP-DocLayout 레이아웃 세션 — `layout.onnx` 가 있을 때만 로드(없으면 Phase 0).
    layout_session: Option<Mutex<Session>>,
    dictionary: Vec<String>,
}

impl OcrEngine {
    /// 모델 디렉토리에서 OcrEngine 초기화
    ///
    /// models_dir에 det.onnx, rec.onnx, dict.txt가 존재해야 함.
    /// `layout_enabled` 가 false 면 layout.onnx 가 디스크에 있어도 로드하지 않는다 —
    /// 레이아웃 분석의 활성 여부는 파일 존재가 아니라 설정으로만 결정된다(다운로드 레이스로
    /// 남은 잔여 파일이 있어도 "off = 기존 동작" 보장이 유지되도록).
    pub fn new(models_dir: &Path, layout_enabled: bool) -> Result<Self, OcrError> {
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

        // 선택적 레이아웃 모델 — 설정이 켜져 있고 파일이 있을 때만 로드(없으면 Phase 0).
        let layout_session = Self::try_load_layout(models_dir, num_threads, layout_enabled);

        tracing::info!(
            "OCR engine initialized: det={:?}, rec={:?}, dict={} chars, layout={}",
            det_path.file_name(),
            rec_path.file_name(),
            dictionary.len(),
            layout_session.is_some()
        );

        Ok(Self {
            det_session: Mutex::new(det_session),
            rec_session: Mutex::new(rec_session),
            layout_session,
            dictionary,
        })
    }

    /// 선택적 레이아웃 모델(`layout.onnx`)을 로드. 없거나 로드 실패면 `None` — OCR 은
    /// 레이아웃 분석 없이 계속 동작한다(기능만 off, 크래시·에러 전파 금지).
    fn try_load_layout(
        models_dir: &Path,
        num_threads: usize,
        layout_enabled: bool,
    ) -> Option<Mutex<Session>> {
        let layout_path = models_dir.join("layout.onnx");
        if !layout_enabled || !layout_path.exists() {
            return None;
        }
        let build = Session::builder()
            .and_then(|b| b.with_execution_providers([ort::ep::CPU::default().build()]))
            .and_then(|b| {
                b.with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
            })
            .and_then(|b| b.with_intra_threads(num_threads))
            .and_then(|b| b.commit_from_file(&layout_path));
        match build {
            Ok(s) => {
                tracing::info!("OCR layout model loaded: {:?}", layout_path.file_name());
                Some(Mutex::new(s))
            }
            Err(e) => {
                tracing::warn!("layout 모델 로드 실패, 레이아웃 분석 생략: {}", e);
                None
            }
        }
    }

    /// 텍스트 영역에 레이아웃 종류를 귀속한다. 레이아웃 세션이 없으면(기본) regions 를
    /// 그대로 반환 — 모든 kind 는 Text 로 유지되어 기존 동작과 완전히 동일하다.
    fn classify_regions(
        &self,
        mut regions: Vec<OcrRegion>,
        image: &DynamicImage,
    ) -> Vec<OcrRegion> {
        let session = match &self.layout_session {
            Some(s) => s,
            None => return regions,
        };
        let layout_regions = match layout::analyze(session, image) {
            Ok(lr) if !lr.is_empty() => lr,
            Ok(_) => return regions,
            Err(e) => {
                tracing::debug!("layout 분석 실패, 분류 생략: {}", e);
                return regions;
            }
        };
        for region in &mut regions {
            // 텍스트 박스가 가장 많이 포함되는 레이아웃 영역의 종류를 귀속(containment 기준).
            let mut best_kind = RegionKind::Text;
            let mut best_overlap = 0.30_f32; // 텍스트 박스 면적 대비 최소 겹침
            for lr in &layout_regions {
                let c = containment(&region.bbox, &lr.bbox);
                if c > best_overlap {
                    best_overlap = c;
                    best_kind = lr.kind;
                }
            }
            region.kind = best_kind;
        }
        regions
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

        // 6. 레이아웃 분류(선택) — 레이아웃 모델이 있으면 각 영역의 kind 를 채운다.
        //    없으면 regions 그대로(모든 kind = Text) → 아래 필터가 무동작.
        let regions = self.classify_regions(regions, image);

        // 본문 텍스트는 머리글/바닥글/페이지번호를 제외해 검색 노이즈를 줄인다.
        // (해당 kind 는 레이아웃 분류가 있을 때만 생기므로 모델 부재 시 기존과 동일.)
        //  regions 자체엔 전부 보존(미리보기 오버레이·별도 소비용).
        let text = regions
            .iter()
            .filter(|r| {
                !matches!(
                    r.kind,
                    RegionKind::Header | RegionKind::Footer | RegionKind::PageNumber
                )
            })
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

/// 텍스트 박스가 레이아웃 영역에 포함되는 비율 = 교집합 면적 / 텍스트 박스 면적.
/// 텍스트 박스는 작고 레이아웃 영역은 크므로 IoU 대신 containment 로 귀속 판정한다.
fn containment(inner: &BBox, outer: &BBox) -> f32 {
    let ix0 = inner.x0.max(outer.x0);
    let iy0 = inner.y0.max(outer.y0);
    let ix1 = inner.x1.min(outer.x1);
    let iy1 = inner.y1.min(outer.y1);
    let inter = (ix1 - ix0).max(0.0) * (iy1 - iy0).max(0.0);
    let inner_area = (inner.x1 - inner.x0).max(0.0) * (inner.y1 - inner.y0).max(0.0);
    if inner_area <= 0.0 {
        0.0
    } else {
        inter / inner_area
    }
}
