//! ONNX 기반 텍스트 임베딩 모듈
//!
//! multilingual-e5-small 모델을 사용하여 텍스트를 384차원 벡터로 변환

use ndarray::{Array1, Array2, Axis};
use ort::{inputs, GraphOptimizationLevel, Session};
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;
use tokenizers::Tokenizer;

/// 임베딩 차원 (e5-small)
pub const EMBEDDING_DIM: usize = 384;

/// 최대 토큰 길이
const MAX_LENGTH: usize = 512;

/// 배치 처리 최대 크기
const MAX_BATCH_SIZE: usize = 32;

#[derive(Error, Debug)]
pub enum EmbedderError {
    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Tokenizer error: {0}")]
    TokenizerError(String),

    #[error("ONNX error: {0}")]
    OnnxError(String),

    #[error("Shape error: {0}")]
    ShapeError(String),
}

/// 텍스트 임베딩 생성기
pub struct Embedder {
    session: Session,
    tokenizer: Tokenizer,
}

impl Embedder {
    /// 새 임베더 생성
    ///
    /// # Arguments
    /// * `model_path` - ONNX 모델 파일 경로
    /// * `tokenizer_path` - tokenizer.json 파일 경로
    pub fn new(model_path: &Path, tokenizer_path: &Path) -> Result<Self, EmbedderError> {
        // 파일 존재 확인
        if !model_path.exists() {
            return Err(EmbedderError::ModelNotFound(
                model_path.to_string_lossy().to_string(),
            ));
        }
        if !tokenizer_path.exists() {
            return Err(EmbedderError::ModelNotFound(
                tokenizer_path.to_string_lossy().to_string(),
            ));
        }

        // ONNX 세션 생성
        let session = Session::builder()
            .map_err(|e| EmbedderError::OnnxError(e.to_string()))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| EmbedderError::OnnxError(e.to_string()))?
            .with_intra_threads(4)
            .map_err(|e| EmbedderError::OnnxError(e.to_string()))?
            .commit_from_file(model_path)
            .map_err(|e| EmbedderError::OnnxError(e.to_string()))?;

        // 토크나이저 로드
        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| EmbedderError::TokenizerError(e.to_string()))?;

        tracing::info!(
            "Embedder initialized: model={:?}, tokenizer={:?}",
            model_path,
            tokenizer_path
        );

        Ok(Self { session, tokenizer })
    }

    /// 단일 텍스트 임베딩 생성
    ///
    /// # Arguments
    /// * `text` - 입력 텍스트
    /// * `is_query` - true면 "query: " 프리픽스, false면 "passage: " 프리픽스
    pub fn embed(&self, text: &str, is_query: bool) -> Result<Vec<f32>, EmbedderError> {
        // e5 모델은 프리픽스 필요
        let prefixed = if is_query {
            format!("query: {}", text)
        } else {
            format!("passage: {}", text)
        };

        // 토큰화
        let encoding = self
            .tokenizer
            .encode(prefixed, true)
            .map_err(|e| EmbedderError::TokenizerError(e.to_string()))?;

        let input_ids: Vec<i64> = encoding
            .get_ids()
            .iter()
            .take(MAX_LENGTH)
            .map(|&id| id as i64)
            .collect();

        let attention_mask: Vec<i64> = encoding
            .get_attention_mask()
            .iter()
            .take(MAX_LENGTH)
            .map(|&m| m as i64)
            .collect();

        let seq_len = input_ids.len();

        // ONNX 입력 텐서 생성
        let input_ids_array = Array2::from_shape_vec((1, seq_len), input_ids)
            .map_err(|e| EmbedderError::ShapeError(e.to_string()))?;

        let attention_mask_array = Array2::from_shape_vec((1, seq_len), attention_mask.clone())
            .map_err(|e| EmbedderError::ShapeError(e.to_string()))?;

        let token_type_ids_array = Array2::zeros((1, seq_len));

        // 추론 실행
        let outputs = self
            .session
            .run(inputs! {
                "input_ids" => input_ids_array,
                "attention_mask" => attention_mask_array,
                "token_type_ids" => token_type_ids_array,
            }?)
            .map_err(|e| EmbedderError::OnnxError(e.to_string()))?;

        // last_hidden_state 추출 (shape: [1, seq_len, 384])
        let output_tensor = outputs
            .get("last_hidden_state")
            .ok_or_else(|| EmbedderError::OnnxError("Missing last_hidden_state output".into()))?;

        let embeddings = output_tensor
            .try_extract_tensor::<f32>()
            .map_err(|e| EmbedderError::OnnxError(e.to_string()))?;

        // Mean pooling with attention mask
        let pooled = self.mean_pooling(&embeddings.view(), &attention_mask)?;

        // L2 정규화
        Ok(self.normalize(&pooled))
    }

    /// 배치 임베딩 생성 (인덱싱용)
    ///
    /// # Arguments
    /// * `texts` - 입력 텍스트 목록
    pub fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedderError> {
        let mut results = Vec::with_capacity(texts.len());

        // 배치 크기 제한
        for chunk in texts.chunks(MAX_BATCH_SIZE) {
            for text in chunk {
                let embedding = self.embed(text, false)?;
                results.push(embedding);
            }
        }

        Ok(results)
    }

    /// Mean pooling with attention mask
    fn mean_pooling(
        &self,
        embeddings: &ndarray::ArrayViewD<'_, f32>,
        attention_mask: &[i64],
    ) -> Result<Vec<f32>, EmbedderError> {
        // embeddings shape: [1, seq_len, hidden_size]
        let shape = embeddings.shape();
        if shape.len() != 3 {
            return Err(EmbedderError::ShapeError(format!(
                "Expected 3D tensor, got {:?}",
                shape
            )));
        }

        let seq_len = shape[1];
        let hidden_size = shape[2];

        let mut pooled = vec![0.0f32; hidden_size];
        let mut mask_sum = 0.0f32;

        for i in 0..seq_len {
            let mask = attention_mask.get(i).copied().unwrap_or(0) as f32;
            mask_sum += mask;

            for j in 0..hidden_size {
                let idx = vec![0, i, j];
                pooled[j] += embeddings[idx.as_slice()] * mask;
            }
        }

        if mask_sum > 0.0 {
            for val in &mut pooled {
                *val /= mask_sum;
            }
        }

        Ok(pooled)
    }

    /// L2 정규화
    fn normalize(&self, vec: &[f32]) -> Vec<f32> {
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            vec.iter().map(|x| x / norm).collect()
        } else {
            vec.to_vec()
        }
    }
}

// Thread-safe wrapper
unsafe impl Send for Embedder {}
unsafe impl Sync for Embedder {}
