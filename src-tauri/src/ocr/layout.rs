//! PP-DocLayout-M — 문서 레이아웃 검출 (ONNX, GreatV/oar-ocr v0.3.0, Apache-2.0)
//!
//! 스캔/이미지 OCR 로 얻은 텍스트 영역을 레이아웃 종류(제목·표·머리글/바닥글·페이지번호
//! 등)로 분류하기 위한 검출기. 모델(`layout.onnx`)이 없으면 로드되지 않으며, OCR 은
//! 레이아웃 분석 없이 동작한다(Phase 0). 전처리·후처리 규약은 oar-ocr 참조 구현과 동일.
//!
//! - 전처리: RGB → 640×640 stretch(CatmullRom) → /255 (mean 0·std 1) → NCHW f32
//! - 입력: `image`[1,3,640,640] + `scale_factor`[1,2]=[640/h, 640/w]
//! - 출력: `[N,6]` = `[class_id, score, x1, y1, x2, y2]` — PaddleDetection multiclass_nms 가
//!   ONNX 그래프에 내장돼 이미 원본 이미지 좌표로 디코드·NMS 된 박스가 나온다(수동 anchor
//!   decode/NMS 불필요).

use super::{BBox, OcrError, RegionKind};
use image::DynamicImage;
use ort::session::Session;
use ort::value::Value;
use std::sync::Mutex;

/// 모델 고정 입력 크기 (PP-DocLayout-M).
const INPUT_SIZE: u32 = 640;
/// 검출 점수 하한 (PaddleX draw_threshold 기본값).
const SCORE_THRESHOLD: f32 = 0.5;
/// 페이지당 최대 영역 수 (PaddleX max_elements).
const MAX_ELEMENTS: usize = 100;

/// 레이아웃 영역 — 축 정렬 bbox(원본 이미지 좌표) + 분류 + 점수.
#[derive(Debug, Clone, Copy)]
pub struct LayoutRegion {
    pub bbox: BBox,
    pub kind: RegionKind,
    pub score: f32,
}

/// PP-DocLayout-M 23-class 인덱스 → docufinder RegionKind 매핑.
///
/// 클래스 순서는 oar-ocr `pp_doclayout_m()`(= PaddleX PP-DocLayout -s/-m/-l 계열)의
/// 고정 순서. 머리글/바닥글/페이지번호만 노이즈 종류로 귀속하고(본문 제외 대상), 나머지
/// 콘텐츠성 클래스는 보수적으로 Text/그에 준하는 종류로 매핑한다.
fn class_to_kind(class_id: usize) -> RegionKind {
    match class_id {
        0 => RegionKind::Title,      // paragraph_title
        1 => RegionKind::Figure,     // image
        2 => RegionKind::Text,       // text
        3 => RegionKind::PageNumber, // number
        4 => RegionKind::Text,       // abstract
        5 => RegionKind::Text,       // content
        6 => RegionKind::Caption,    // figure_title
        7 => RegionKind::Text,       // formula
        8 => RegionKind::Table,      // table
        9 => RegionKind::Caption,    // table_title
        10 => RegionKind::Text,      // reference
        11 => RegionKind::Title,     // doc_title
        12 => RegionKind::Text,      // footnote (콘텐츠 — 보수적으로 Text)
        13 => RegionKind::Header,    // header
        14 => RegionKind::Text,      // algorithm
        15 => RegionKind::Footer,    // footer
        16 => RegionKind::Figure,    // seal
        17 => RegionKind::Caption,   // chart_title
        18 => RegionKind::Figure,    // chart
        19 => RegionKind::Text,      // formula_number
        20 => RegionKind::Header,    // header_image
        21 => RegionKind::Footer,    // footer_image
        22 => RegionKind::Text,      // aside_text
        _ => RegionKind::Text,
    }
}

const NUM_CLASSES: usize = 23;

/// 이미지 → 레이아웃 영역 목록. 모델 세션은 지연 로드된 PP-DocLayout-M.
pub fn analyze(
    session: &Mutex<Session>,
    image: &DynamicImage,
) -> Result<Vec<LayoutRegion>, OcrError> {
    let orig_w = image.width() as f32;
    let orig_h = image.height() as f32;
    if orig_w < 1.0 || orig_h < 1.0 {
        return Ok(vec![]);
    }

    // 전처리: 640×640 stretch(CatmullRom) → /255 → NCHW(R,G,B 평면).
    let resized = image
        .resize_exact(
            INPUT_SIZE,
            INPUT_SIZE,
            image::imageops::FilterType::CatmullRom,
        )
        .to_rgb8();
    let plane = (INPUT_SIZE * INPUT_SIZE) as usize;
    let mut chw = vec![0.0f32; 3 * plane];
    for (i, px) in resized.pixels().enumerate() {
        chw[i] = px[0] as f32 / 255.0;
        chw[plane + i] = px[1] as f32 / 255.0;
        chw[2 * plane + i] = px[2] as f32 / 255.0;
    }

    let image_val = Value::from_array(([1i64, 3, INPUT_SIZE as i64, INPUT_SIZE as i64], chw))
        .map_err(|e| OcrError::Inference(e.to_string()))?;
    // scale_factor = [scale_y, scale_x] = [640/h, 640/w] — 모델이 내부에서 박스를 원본
    // 좌표로 되돌리는 데 사용한다.
    let scale = vec![INPUT_SIZE as f32 / orig_h, INPUT_SIZE as f32 / orig_w];
    let scale_val =
        Value::from_array(([1i64, 2], scale)).map_err(|e| OcrError::Inference(e.to_string()))?;

    let rows: Vec<f32> = {
        let mut sess = session.lock().unwrap_or_else(|p| p.into_inner());
        let outputs = sess
            .run(ort::inputs!["image" => image_val, "scale_factor" => scale_val])
            .map_err(|e| OcrError::Inference(e.to_string()))?;

        // 검출 텐서 = [N,6](= 6N 원소). 다른 출력(박스 수 [1])과 원소 수로 구분 — 6 배수 중
        // 가장 큰 것을 고른다(검출 0건이면 [0,6]→len 0→아래 chunks 가 빈 결과).
        let names: Vec<String> = outputs.keys().map(|k| k.to_string()).collect();
        let mut picked: Vec<f32> = Vec::new();
        for name in &names {
            if let Some(v) = outputs.get(name.as_str()) {
                if let Ok((_shape, data)) = v.try_extract_tensor::<f32>() {
                    if data.len() % 6 == 0 && data.len() > picked.len() {
                        picked = data.to_vec();
                    }
                }
            }
        }
        picked
    };

    // 후처리: 각 행 [class_id, score, x1, y1, x2, y2] — 이미 원본 좌표.
    let mut out: Vec<LayoutRegion> = Vec::new();
    for row in rows.chunks_exact(6) {
        let class_id = row[0] as i32;
        let score = row[1];
        if class_id < 0 || class_id as usize >= NUM_CLASSES || score < SCORE_THRESHOLD {
            continue;
        }
        // clamp 후 좌표는 [0,orig] 이거나 NaN(입력이 NaN 일 때). NaN 은 비교가 항상 false 라
        // x1>x0 && y1>y0 검사에서 자연히 걸러진다(+Inf 는 clamp 로 유한값이 됨).
        let x0 = row[2].clamp(0.0, orig_w);
        let y0 = row[3].clamp(0.0, orig_h);
        let x1 = row[4].clamp(0.0, orig_w);
        let y1 = row[5].clamp(0.0, orig_h);
        if x1 > x0 && y1 > y0 {
            out.push(LayoutRegion {
                bbox: BBox { x0, y0, x1, y1 },
                kind: class_to_kind(class_id as usize),
                score,
            });
        }
    }

    // PaddleX 스타일 그리디 NMS — 동일 클래스 IoU≥0.6, 다른 클래스 IoU≥0.98 이면 제거.
    // 모델 그래프에 이미 multiclass_nms 가 있어 대개 잔여 중복은 적으나, 규약대로 한 번 더.
    let kept = paddlex_nms(&mut out);
    Ok(kept.into_iter().take(MAX_ELEMENTS).collect())
}

/// PaddleX 레이아웃 NMS(+1 픽셀 면적 규약). 점수 내림차순 그리디.
fn paddlex_nms(dets: &mut [LayoutRegion]) -> Vec<LayoutRegion> {
    dets.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut suppressed = vec![false; dets.len()];
    let mut kept = Vec::new();
    for i in 0..dets.len() {
        if suppressed[i] {
            continue;
        }
        kept.push(dets[i]);
        for j in (i + 1)..dets.len() {
            if suppressed[j] {
                continue;
            }
            let same_class = dets[i].kind == dets[j].kind;
            let thr = if same_class { 0.6 } else { 0.98 };
            if iou_plus1(&dets[i].bbox, &dets[j].bbox) >= thr {
                suppressed[j] = true;
            }
        }
    }
    kept
}

/// PaddleX IoU — 면적에 +1 픽셀 규약.
fn iou_plus1(a: &BBox, b: &BBox) -> f32 {
    let ix0 = a.x0.max(b.x0);
    let iy0 = a.y0.max(b.y0);
    let ix1 = a.x1.min(b.x1);
    let iy1 = a.y1.min(b.y1);
    let iw = (ix1 - ix0 + 1.0).max(0.0);
    let ih = (iy1 - iy0 + 1.0).max(0.0);
    let inter = iw * ih;
    let area_a = (a.x1 - a.x0 + 1.0) * (a.y1 - a.y0 + 1.0);
    let area_b = (b.x1 - b.x0 + 1.0) * (b.y1 - b.y0 + 1.0);
    let union = area_a + area_b - inter;
    if union <= 0.0 {
        0.0
    } else {
        inter / union
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ort::session::Session;

    /// 실모델 스모크 — env 로 모델·이미지·onnxruntime 를 주면 실행, 없으면 스킵(CI 무해).
    /// 포팅한 전처리·I/O·디코드·클래스맵이 실제 PP-DocLayout-M 에서 정상 동작하는지 검증.
    /// 실행: `ORT_DYLIB_PATH=<libonnxruntime> LAYOUT_MODEL=<layout.onnx> LAYOUT_IMAGE=<doc.png> \
    ///        cargo test --lib ocr::layout::tests::layout_smoke -- --nocapture`
    #[test]
    fn layout_smoke() {
        let (model, image_path) =
            match (std::env::var("LAYOUT_MODEL"), std::env::var("LAYOUT_IMAGE")) {
                (Ok(m), Ok(i)) => (m, i),
                _ => {
                    eprintln!(
                        "skip layout_smoke: set LAYOUT_MODEL + LAYOUT_IMAGE (+ ORT_DYLIB_PATH)"
                    );
                    return;
                }
            };

        let session = Session::builder()
            .unwrap()
            .with_execution_providers([ort::ep::CPU::default().build()])
            .unwrap()
            .commit_from_file(&model)
            .unwrap();
        let img = image::open(&image_path).unwrap();
        let regions = analyze(&Mutex::new(session), &img).unwrap();

        eprintln!(
            "layout regions: {} (image {}x{})",
            regions.len(),
            img.width(),
            img.height()
        );
        for r in &regions {
            eprintln!(
                "  {:?} score={:.3} bbox=({:.0},{:.0})-({:.0},{:.0})",
                r.kind, r.score, r.bbox.x0, r.bbox.y0, r.bbox.x1, r.bbox.y1
            );
        }

        let (w, h) = (img.width() as f32, img.height() as f32);
        for r in &regions {
            assert!(
                r.bbox.x0 >= 0.0 && r.bbox.x1 <= w && r.bbox.y0 >= 0.0 && r.bbox.y1 <= h,
                "box out of image bounds: {:?}",
                r.bbox
            );
            assert!(
                r.bbox.x1 > r.bbox.x0 && r.bbox.y1 > r.bbox.y0,
                "degenerate box"
            );
            assert!(r.score >= SCORE_THRESHOLD, "score below threshold");
        }
    }
}
