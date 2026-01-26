//! 텍스트 임베딩 모듈 (e5-small ONNX)

use ndarray::Array2;
use ort::session::Session;
use ort::value::Value;
use std::path::Path;
use std::sync::Mutex;
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

    #[error("Lock failed")]
    LockFailed,
}

/// 텍스트 임베딩 생성기
///
/// Session은 &mut self를 필요로 하므로 내부 Mutex 사용
/// 토큰화는 병렬 가능, ONNX 추론만 직렬화
pub struct Embedder {
    session: Mutex<Session>,
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

        // 동적 스레드 수 감지 (최대 8개, 최소 4개)
        let num_threads = std::thread::available_parallelism()
            .map(|p| p.get().clamp(4, 8))
            .unwrap_or(4);

        tracing::debug!("Embedder using {} intra-op threads", num_threads);

        // ONNX 세션 생성 (최적화 적용)
        let session = Session::builder()
            .map_err(|e: ort::Error| EmbedderError::OrtError(e.to_string()))?
            .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
            .map_err(|e: ort::Error| EmbedderError::OrtError(e.to_string()))?
            .with_intra_threads(num_threads)
            .map_err(|e: ort::Error| EmbedderError::OrtError(e.to_string()))?
            .with_parallel_execution(true)
            .map_err(|e: ort::Error| EmbedderError::OrtError(e.to_string()))?
            .commit_from_file(model_path)
            .map_err(|e: ort::Error| EmbedderError::OrtError(e.to_string()))?;

        // Tokenizer 로드
        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| EmbedderError::TokenizerError(e.to_string()))?;

        Ok(Self {
            session: Mutex::new(session),
            tokenizer,
        })
    }

    /// 단일 텍스트 임베딩
    pub fn embed(&self, text: &str, is_query: bool) -> Result<Vec<f32>, EmbedderError> {
        let embeddings = self.embed_batch(&[self.prepare_text(text, is_query)])?;
        embeddings
            .into_iter()
            .next()
            .ok_or(EmbedderError::InvalidDimension)
    }

    /// 배치 임베딩 (불변 참조 - 락 없이 병렬 호출 가능)
    pub fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedderError> {
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

        // ONNX 추론 (Session은 &mut self 필요 → Mutex 사용)
        // e5-small INT8 모델은 input_ids, attention_mask 2개 입력만 필요
        let input_ids_value = Value::from_array((shape, input_ids_vec))
            .map_err(|e: ort::Error| EmbedderError::OrtError(e.to_string()))?;
        let attention_mask_value = Value::from_array((shape, attention_mask_vec))
            .map_err(|e: ort::Error| EmbedderError::OrtError(e.to_string()))?;

        let embeddings = {
            let mut session = self.session.lock().map_err(|_| EmbedderError::LockFailed)?;

            // 먼저 출력 이름들 수집 (borrow 충돌 방지)
            let output_names: Vec<String> = session
                .outputs()
                .iter()
                .map(|o| o.name().to_string())
                .collect();

            let outputs = session
                .run(ort::inputs![
                    "input_ids" => input_ids_value,
                    "attention_mask" => attention_mask_value,
                ])
                .map_err(|e: ort::Error| EmbedderError::OrtError(e.to_string()))?;

            // 출력에서 임베딩 추출 (모델에 따라 출력 이름이 다를 수 있음)
            let output = outputs
                .get("last_hidden_state")
                .or_else(|| outputs.get("output"))
                .or_else(|| outputs.get("sentence_embedding"))
                .or_else(|| outputs.get("token_embeddings"))
                .or_else(|| {
                    // 첫 번째 출력 사용 (fallback)
                    output_names.first().and_then(|name| outputs.get(name.as_str()))
                })
                .ok_or_else(|| {
                    EmbedderError::OrtError(format!("No embedding output found. Available: {:?}", output_names))
                })?;

            let (out_shape, out_data) = output
                .try_extract_tensor::<f32>()
                .map_err(|e: ort::Error| EmbedderError::OrtError(e.to_string()))?;

            let dims = out_shape.len();

            if dims == 2 {
                // 2D: [batch, hidden_dim] - 이미 pooling된 sentence embedding
                let hidden_dim = out_shape.get(1).map(|&d| d as usize).unwrap_or(EMBEDDING_DIM);
                let mut embeddings = Vec::with_capacity(batch_size);

                for i in 0..batch_size {
                    let mut emb = vec![0.0f32; EMBEDDING_DIM];
                    let offset = i * hidden_dim;
                    for k in 0..EMBEDDING_DIM.min(hidden_dim) {
                        if offset + k < out_data.len() {
                            emb[k] = out_data[offset + k];
                        }
                    }
                    // L2 normalize
                    let norm: f32 = emb.iter().map(|x| x * x).sum::<f32>().sqrt();
                    if norm > 0.0 {
                        for v in &mut emb {
                            *v /= norm;
                        }
                    }
                    embeddings.push(emb);
                }
                embeddings
            } else {
                // 3D: [batch, seq_len, hidden_dim] - mean pooling 필요
                let model_seq_len = out_shape.get(1).map(|&d| d as usize).unwrap_or(seq_len);
                let hidden_dim = out_shape.get(2).map(|&d| d as usize).unwrap_or(EMBEDDING_DIM);

                let mut embeddings = Vec::with_capacity(batch_size);
                for i in 0..batch_size {
                    let mut sum = vec![0.0f32; EMBEDDING_DIM];
                    let mut count = 0.0f32;

                    for j in 0..model_seq_len.min(seq_len) {
                        if j < seq_len && attention_mask[[i, j]] == 1 {
                            let offset = i * model_seq_len * hidden_dim + j * hidden_dim;
                            for k in 0..EMBEDDING_DIM.min(hidden_dim) {
                                if offset + k < out_data.len() {
                                    sum[k] += out_data[offset + k];
                                }
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
                embeddings
            }
        };

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
