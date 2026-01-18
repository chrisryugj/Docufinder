//! 텍스트 임베딩 모듈 (e5-small ONNX)

use ndarray::Array2;
use ort::session::Session;
use ort::value::Value;
use std::path::Path;
use thiserror::Error;
use tokenizers::Tokenizer;

pub const EMBEDDING_DIM: usize = 384;
const MAX_LENGTH: usize = 512;

#[derive(Error, Debug)]
pub enum EmbedderError {
    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Tokenizer error: {0}")]
    TokenizerError(String),

    #[error("ONNX Runtime error: {0}")]
    OrtError(String),

    #[error("Invalid embedding dimension")]
    InvalidDimension,
}

/// 텍스트 임베딩 생성기
pub struct Embedder {
    session: Session,
    tokenizer: Tokenizer,
}

impl Embedder {
    /// 새 Embedder 생성
    pub fn new(model_path: &Path, tokenizer_path: &Path) -> Result<Self, EmbedderError> {
        // 모델 파일 확인
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
            .map_err(|e: ort::Error| EmbedderError::OrtError(e.to_string()))?
            .with_intra_threads(4)
            .map_err(|e: ort::Error| EmbedderError::OrtError(e.to_string()))?
            .commit_from_file(model_path)
            .map_err(|e: ort::Error| EmbedderError::OrtError(e.to_string()))?;

        // Tokenizer 로드
        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| EmbedderError::TokenizerError(e.to_string()))?;

        Ok(Self { session, tokenizer })
    }

    /// 단일 텍스트 임베딩
    pub fn embed(&mut self, text: &str, is_query: bool) -> Result<Vec<f32>, EmbedderError> {
        let embeddings = self.embed_batch(&[self.prepare_text(text, is_query)])?;
        embeddings
            .into_iter()
            .next()
            .ok_or(EmbedderError::InvalidDimension)
    }

    /// 배치 임베딩
    pub fn embed_batch(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedderError> {
        if texts.is_empty() {
            return Ok(vec![]);
        }

        // 토큰화
        let encodings = self
            .tokenizer
            .encode_batch(texts.to_vec(), true)
            .map_err(|e| EmbedderError::TokenizerError(e.to_string()))?;

        let batch_size = encodings.len();
        let seq_len = encodings
            .iter()
            .map(|e| e.get_ids().len().min(MAX_LENGTH))
            .max()
            .unwrap_or(0);

        // 입력 텐서 생성 (owned arrays)
        let mut input_ids = Array2::<i64>::zeros((batch_size, seq_len));
        let mut attention_mask = Array2::<i64>::zeros((batch_size, seq_len));
        let token_type_ids = Array2::<i64>::zeros((batch_size, seq_len));

        for (i, encoding) in encodings.iter().enumerate() {
            let ids = encoding.get_ids();
            let mask = encoding.get_attention_mask();
            let len = ids.len().min(seq_len);

            for j in 0..len {
                input_ids[[i, j]] = ids[j] as i64;
                attention_mask[[i, j]] = mask[j] as i64;
            }
        }

        // 입력 데이터를 Vec으로 변환
        let shape = [batch_size as i64, seq_len as i64];
        let input_ids_vec: Vec<i64> = input_ids.iter().copied().collect();
        let attention_mask_vec: Vec<i64> = attention_mask.iter().copied().collect();
        let token_type_ids_vec: Vec<i64> = token_type_ids.iter().copied().collect();

        // ONNX 추론
        let input_ids_value = Value::from_array((shape, input_ids_vec))
            .map_err(|e: ort::Error| EmbedderError::OrtError(e.to_string()))?;
        let attention_mask_value = Value::from_array((shape, attention_mask_vec))
            .map_err(|e: ort::Error| EmbedderError::OrtError(e.to_string()))?;
        let token_type_ids_value = Value::from_array((shape, token_type_ids_vec))
            .map_err(|e: ort::Error| EmbedderError::OrtError(e.to_string()))?;

        let outputs = self
            .session
            .run(ort::inputs![
                "input_ids" => input_ids_value,
                "attention_mask" => attention_mask_value,
                "token_type_ids" => token_type_ids_value,
            ])
            .map_err(|e: ort::Error| EmbedderError::OrtError(e.to_string()))?;

        // 출력에서 임베딩 추출
        let output = outputs
            .get("last_hidden_state")
            .ok_or_else(|| EmbedderError::OrtError("No last_hidden_state output".to_string()))?;

        let (out_shape, out_data) = output
            .try_extract_tensor::<f32>()
            .map_err(|e: ort::Error| EmbedderError::OrtError(e.to_string()))?;

        // shape: [batch, seq_len, hidden_dim]
        // Shape implements Deref<Target=[i64]>
        let hidden_dim = out_shape.get(2).map(|&d| d as usize).unwrap_or(EMBEDDING_DIM);

        // Mean pooling with attention mask
        let mut embeddings = Vec::with_capacity(batch_size);
        for i in 0..batch_size {
            let mut sum = vec![0.0f32; EMBEDDING_DIM];
            let mut count = 0.0f32;

            for j in 0..seq_len {
                if attention_mask[[i, j]] == 1 {
                    let offset = i * seq_len * hidden_dim + j * hidden_dim;
                    for k in 0..EMBEDDING_DIM.min(hidden_dim) {
                        sum[k] += out_data[offset + k];
                    }
                    count += 1.0;
                }
            }

            // Average
            if count > 0.0 {
                for v in &mut sum {
                    *v /= count;
                }
            }

            // L2 normalize
            let norm: f32 = sum.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 0.0 {
                for v in &mut sum {
                    *v /= norm;
                }
            }

            embeddings.push(sum);
        }

        Ok(embeddings)
    }

    /// e5 모델용 텍스트 전처리
    fn prepare_text(&self, text: &str, is_query: bool) -> String {
        if is_query {
            format!("query: {}", text)
        } else {
            format!("passage: {}", text)
        }
    }
}

unsafe impl Send for Embedder {}
unsafe impl Sync for Embedder {}
