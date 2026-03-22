//! PaddleOCR Recognition (SVTR) — 텍스트 인식 + CTC 디코딩

use image::RgbImage;
use ort::session::Session;
use ort::value::Value;
use std::sync::Mutex;

/// Recognition 파라미터
const REC_HEIGHT: u32 = 48;
const REC_MAX_WIDTH: u32 = 320;
const REC_BATCH_SIZE: usize = 6;

/// 인식 결과
#[derive(Debug, Clone)]
pub struct RecognitionResult {
    pub text: String,
    pub confidence: f32,
}

/// 여러 crop 이미지를 배치로 인식
pub fn recognize_batch(
    session: &Mutex<Session>,
    crops: &[RgbImage],
    dictionary: &[String],
) -> Result<Vec<RecognitionResult>, super::OcrError> {
    let mut results = Vec::with_capacity(crops.len());

    // 배치 단위 처리
    for batch_start in (0..crops.len()).step_by(REC_BATCH_SIZE) {
        let batch_end = (batch_start + REC_BATCH_SIZE).min(crops.len());
        let batch = &crops[batch_start..batch_end];
        let batch_results = recognize_single_batch(session, batch, dictionary)?;
        results.extend(batch_results);
    }

    Ok(results)
}

/// 단일 배치 인식
fn recognize_single_batch(
    session: &Mutex<Session>,
    crops: &[RgbImage],
    dictionary: &[String],
) -> Result<Vec<RecognitionResult>, super::OcrError> {
    if crops.is_empty() {
        return Ok(vec![]);
    }

    let batch_size = crops.len();

    // 최대 너비 계산 (배치 내 동적 패딩)
    let max_wh_ratio: f32 = crops
        .iter()
        .map(|c| c.width() as f32 / c.height().max(1) as f32)
        .fold(0.0f32, f32::max);

    let target_w = ((REC_HEIGHT as f32 * max_wh_ratio).ceil() as u32)
        .clamp(REC_HEIGHT, REC_MAX_WIDTH);

    // 전처리: resize + normalize + pad
    let pixels_per_image = 3 * REC_HEIGHT as usize * target_w as usize;
    let mut tensor = vec![0.0f32; batch_size * pixels_per_image];

    for (bi, crop) in crops.iter().enumerate() {
        let resized = resize_rec(crop, target_w);
        let offset = bi * pixels_per_image;
        let hw = REC_HEIGHT as usize * target_w as usize;

        for y in 0..REC_HEIGHT as usize {
            for x in 0..resized.width() as usize {
                let pixel = resized.get_pixel(x as u32, y as u32);
                let idx = y * target_w as usize + x;
                tensor[offset + idx] = (pixel[0] as f32 / 255.0 - 0.5) / 0.5;
                tensor[offset + hw + idx] = (pixel[1] as f32 / 255.0 - 0.5) / 0.5;
                tensor[offset + 2 * hw + idx] = (pixel[2] as f32 / 255.0 - 0.5) / 0.5;
            }
            // 나머지 (resized.width()..target_w)는 이미 0.0 (0-pad 효과)
        }
    }

    // ort 추론
    let shape = [batch_size as i64, 3, REC_HEIGHT as i64, target_w as i64];
    let input = Value::from_array((shape, tensor.clone()))
        .map_err(|e| super::OcrError::Inference(e.to_string()))?;

    let (seq_len, num_classes, flat_data) = {
        let mut sess = session.lock().unwrap_or_else(|p| p.into_inner());
        let outputs = sess.run(ort::inputs!["x" => input])
            .map_err(|e| super::OcrError::Inference(e.to_string()))?;

        let output_names: Vec<String> = outputs.keys().map(|k| k.to_string()).collect();
        let output = output_names
            .first()
            .and_then(|name| outputs.get(name.as_str()))
            .ok_or_else(|| super::OcrError::Inference("No recognition output".into()))?;

        let (out_shape, out_data) = output
            .try_extract_tensor::<f32>()
            .map_err(|e| super::OcrError::Inference(e.to_string()))?;

        if out_shape.len() != 3 {
            return Err(super::OcrError::Inference(format!(
                "Unexpected recognition output dims: {}", out_shape.len()
            )));
        }
        let sl = out_shape.get(1).map(|&d| d as usize).unwrap_or(0);
        let nc = out_shape.get(2).map(|&d| d as usize).unwrap_or(0);
        (sl, nc, out_data.to_vec())
    };

    // CTC 디코딩
    let mut results = Vec::with_capacity(batch_size);
    for bi in 0..batch_size {
        let offset = bi * seq_len * num_classes;
        if offset + seq_len * num_classes > flat_data.len() {
            break;
        }
        let logits = &flat_data[offset..offset + seq_len * num_classes];
        let result = ctc_decode(logits, seq_len, num_classes, dictionary);
        results.push(result);
    }

    Ok(results)
}

/// CTC Greedy 디코딩
fn ctc_decode(
    logits: &[f32],
    seq_len: usize,
    num_classes: usize,
    dictionary: &[String],
) -> RecognitionResult {
    let mut chars = Vec::new();
    let mut scores = Vec::new();
    let mut prev_idx: i64 = -1;

    for t in 0..seq_len {
        let row = &logits[t * num_classes..(t + 1) * num_classes];

        // argmax
        let (max_idx, &max_val) = row
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or((0, &0.0));

        let idx = max_idx as i64;

        // blank(0) 제거 + 연속 중복 제거
        if idx != 0 && idx != prev_idx {
            // dictionary는 1-indexed (idx=1 → dictionary[0])
            let dict_idx = (idx - 1) as usize;
            if dict_idx < dictionary.len() {
                chars.push(dictionary[dict_idx].clone());
                // softmax 근사 (max값을 신뢰도로 사용)
                scores.push(max_val);
            }
        }
        prev_idx = idx;
    }

    let text = chars.join("");
    let confidence = if scores.is_empty() {
        0.0
    } else {
        scores.iter().sum::<f32>() / scores.len() as f32
    };

    RecognitionResult { text, confidence }
}

/// Recognition용 리사이즈 (높이 48 고정, 종횡비 유지)
fn resize_rec(crop: &RgbImage, max_width: u32) -> RgbImage {
    let (w, h) = (crop.width(), crop.height());
    if h == 0 {
        return RgbImage::new(1, REC_HEIGHT);
    }

    let ratio = w as f32 / h as f32;
    let target_w = (REC_HEIGHT as f32 * ratio).ceil() as u32;
    let target_w = target_w.clamp(1, max_width);

    image::imageops::resize(crop, target_w, REC_HEIGHT, image::imageops::FilterType::Lanczos3)
}
