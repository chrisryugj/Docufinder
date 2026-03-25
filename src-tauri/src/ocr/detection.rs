//! PaddleOCR Detection (DBNet) — 텍스트 영역 검출

use super::geometry::{self, Quad};
use image::DynamicImage;
use ort::session::Session;
use ort::value::Value;
use std::sync::Mutex;

/// Detection 파라미터
const LIMIT_SIDE_LEN: u32 = 960;
const BINARY_THRESH: f32 = 0.3;
const BOX_THRESH: f32 = 0.5;
const MARGIN_RATIO: f32 = 0.1;
const MIN_COMPONENT_AREA: usize = 16;

/// Detection 전처리: 이미지 → 정규화된 텐서
pub fn preprocess(image: &DynamicImage) -> (Vec<f32>, u32, u32, f32, f32) {
    let rgb = image.to_rgb8();
    let (orig_w, orig_h) = (rgb.width(), rgb.height());

    // Resize: 장변을 LIMIT_SIDE_LEN으로 제한, 32배수로 올림
    let ratio = if orig_w.max(orig_h) > LIMIT_SIDE_LEN {
        LIMIT_SIDE_LEN as f32 / orig_w.max(orig_h) as f32
    } else {
        1.0
    };

    let new_w = ((orig_w as f32 * ratio / 32.0).ceil() * 32.0).max(32.0) as u32;
    let new_h = ((orig_h as f32 * ratio / 32.0).ceil() * 32.0).max(32.0) as u32;

    let resized =
        image::imageops::resize(&rgb, new_w, new_h, image::imageops::FilterType::Lanczos3);

    // Normalize: (x/255 - 0.5) / 0.5 → [-1, 1] 범위
    // HWC → CHW
    let pixels = new_h as usize * new_w as usize;
    let mut tensor = vec![0.0f32; 3 * pixels];

    for y in 0..new_h as usize {
        for x in 0..new_w as usize {
            let pixel = resized.get_pixel(x as u32, y as u32);
            let idx = y * new_w as usize + x;
            // RGB 채널 (PaddleOCR는 BGR이지만, 정규화 mean/std가 동일하므로 RGB도 OK)
            tensor[idx] = (pixel[0] as f32 / 255.0 - 0.5) / 0.5; // R → C0
            tensor[pixels + idx] = (pixel[1] as f32 / 255.0 - 0.5) / 0.5; // G → C1
            tensor[2 * pixels + idx] = (pixel[2] as f32 / 255.0 - 0.5) / 0.5; // B → C2
        }
    }

    let scale_x = new_w as f32 / orig_w as f32;
    let scale_y = new_h as f32 / orig_h as f32;

    (tensor, new_w, new_h, scale_x, scale_y)
}

/// Detection 추론 + 후처리 → 텍스트 바운딩 박스 목록
pub fn detect(
    session: &Mutex<Session>,
    image: &DynamicImage,
) -> Result<Vec<Quad>, super::OcrError> {
    let (orig_w, orig_h) = (image.width(), image.height());
    let (tensor, new_w, new_h, scale_x, scale_y) = preprocess(image);

    // ort 추론
    let shape = [1i64, 3, new_h as i64, new_w as i64];
    let input = Value::from_array((shape, tensor.clone()))
        .map_err(|e| super::OcrError::Inference(e.to_string()))?;

    let prob_map = {
        let mut sess = session.lock().unwrap_or_else(|p| p.into_inner());
        let outputs = sess
            .run(ort::inputs!["x" => input])
            .map_err(|e| super::OcrError::Inference(e.to_string()))?;

        let output_names: Vec<String> = outputs.keys().map(|k| k.to_string()).collect();
        let output = output_names
            .first()
            .and_then(|name| outputs.get(name.as_str()))
            .ok_or_else(|| super::OcrError::Inference("No detection output".into()))?;

        let (_out_shape, prob_data) = output
            .try_extract_tensor::<f32>()
            .map_err(|e| super::OcrError::Inference(e.to_string()))?;
        prob_data.to_vec()
    };

    // 후처리: DBPostProcess
    let boxes = postprocess(
        &prob_map,
        new_w as usize,
        new_h as usize,
        scale_x,
        scale_y,
        orig_w as usize,
        orig_h as usize,
    );

    Ok(boxes)
}

/// DBPostProcess: 확률 맵 → 바운딩 박스
fn postprocess(
    prob_map: &[f32],
    map_w: usize,
    map_h: usize,
    scale_x: f32,
    scale_y: f32,
    orig_w: usize,
    orig_h: usize,
) -> Vec<Quad> {
    // 1. 이진화
    let binary: Vec<u8> = prob_map
        .iter()
        .map(|&v| if v > BINARY_THRESH { 1 } else { 0 })
        .collect();

    // 2. 연결 컴포넌트 검출
    let components = geometry::find_connected_components(&binary, map_w, map_h, MIN_COMPONENT_AREA);

    // 3. 각 컴포넌트 → 바운딩 박스 + 스코어 필터링
    let mut boxes = Vec::new();
    for comp in &components {
        let mut quad = geometry::bounding_box_with_margin(comp, MARGIN_RATIO, map_w, map_h);

        // 신뢰도 계산
        let score = geometry::compute_box_score(prob_map, map_w, &quad, 1.0, 1.0);
        if score < BOX_THRESH {
            continue;
        }
        quad.score = score;

        // 원본 이미지 좌표로 스케일링
        for p in &mut quad.points {
            p.0 /= scale_x;
            p.1 /= scale_y;
            p.0 = p.0.clamp(0.0, orig_w as f32 - 1.0);
            p.1 = p.1.clamp(0.0, orig_h as f32 - 1.0);
        }

        // 너무 작은 박스 필터링
        let w = quad.points[1].0 - quad.points[0].0;
        let h = quad.points[2].1 - quad.points[0].1;
        if w < 3.0 || h < 3.0 {
            continue;
        }

        boxes.push(quad);
    }

    // 4. 읽기 순서로 정렬
    geometry::sort_boxes_reading_order(&mut boxes);

    boxes
}
