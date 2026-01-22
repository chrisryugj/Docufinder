//! Cross-Encoder Reranking 모듈
//!
//! RRF 병합 후 Top-K 결과에 대해 Cross-Encoder로 재정렬하여
//! 검색 정확도를 향상시킵니다.

use ndarray::Array2;
use ort::session::Session;
use ort::value::Value;
use std::path::Path;
use std::sync::Mutex;
use thiserror::Error;
use tokenizers::Tokenizer;

const MAX_LENGTH: usize = 512;

#[derive(Error, Debug)]
pub enum RerankerError {
    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Tokenizer error: {0}")]
    TokenizerError(String),

    #[error("ONNX Runtime error: {0}")]
    OrtError(String),

    #[error("Lock failed")]
    LockFailed,
}

/// Cross-Encoder Reranker
///
/// (query, document) 쌍의 관련도 점수를 계산하여
/// 검색 결과를 재정렬합니다.
pub struct Reranker {
    session: Mutex<Session>,
    tokenizer: Tokenizer,
}

impl Reranker {
    /// 새 Reranker 생성
    pub fn new(model_path: &Path, tokenizer_path: &Path) -> Result<Self, RerankerError> {
        if !model_path.exists() {
            return Err(RerankerError::ModelNotFound(
                model_path.to_string_lossy().to_string(),
            ));
        }

        if !tokenizer_path.exists() {
            return Err(RerankerError::ModelNotFound(
                tokenizer_path.to_string_lossy().to_string(),
            ));
        }

        // 동적 스레드 수 감지 (최대 8개, 최소 4개)
        let num_threads = std::thread::available_parallelism()
            .map(|p| p.get().clamp(4, 8))
            .unwrap_or(4);

        tracing::debug!("Reranker using {} intra-op threads", num_threads);

        // ONNX 세션 생성
        let session = Session::builder()
            .map_err(|e: ort::Error| RerankerError::OrtError(e.to_string()))?
            .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
            .map_err(|e: ort::Error| RerankerError::OrtError(e.to_string()))?
            .with_intra_threads(num_threads)
            .map_err(|e: ort::Error| RerankerError::OrtError(e.to_string()))?
            .with_parallel_execution(true)
            .map_err(|e: ort::Error| RerankerError::OrtError(e.to_string()))?
            .commit_from_file(model_path)
            .map_err(|e: ort::Error| RerankerError::OrtError(e.to_string()))?;

        // Tokenizer 로드
        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| RerankerError::TokenizerError(e.to_string()))?;

        Ok(Self {
            session: Mutex::new(session),
            tokenizer,
        })
    }

    /// (query, document) 쌍들의 관련도 점수 계산
    ///
    /// 반환값: 각 document의 관련도 점수 (높을수록 관련도 높음)
    pub fn score(&self, query: &str, documents: &[&str]) -> Result<Vec<f32>, RerankerError> {
        if documents.is_empty() {
            return Ok(vec![]);
        }

        // Cross-Encoder 입력 생성: [CLS] query [SEP] document [SEP]
        // tokenizer.encode_batch_with_pairs 대신 수동으로 처리
        let pairs: Vec<(String, String)> = documents
            .iter()
            .map(|doc| (query.to_string(), doc.to_string()))
            .collect();

        // 토큰화 (pairs 형식)
        let encodings = pairs
            .iter()
            .map(|(q, d)| {
                self.tokenizer
                    .encode((q.as_str(), d.as_str()), true)
                    .map_err(|e| RerankerError::TokenizerError(e.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let batch_size = encodings.len();
        let seq_len = encodings
            .iter()
            .map(|e| e.get_ids().len().min(MAX_LENGTH))
            .max()
            .unwrap_or(0);

        // 입력 텐서 생성
        let mut input_ids = Array2::<i64>::zeros((batch_size, seq_len));
        let mut attention_mask = Array2::<i64>::zeros((batch_size, seq_len));
        let mut token_type_ids = Array2::<i64>::zeros((batch_size, seq_len));

        for (i, encoding) in encodings.iter().enumerate() {
            let ids = encoding.get_ids();
            let mask = encoding.get_attention_mask();
            let type_ids = encoding.get_type_ids();
            let len = ids.len().min(seq_len);

            for j in 0..len {
                input_ids[[i, j]] = ids[j] as i64;
                attention_mask[[i, j]] = mask[j] as i64;
                token_type_ids[[i, j]] = type_ids[j] as i64;
            }
        }

        // Vec으로 변환
        let shape = [batch_size as i64, seq_len as i64];
        let input_ids_vec: Vec<i64> = input_ids.iter().copied().collect();
        let attention_mask_vec: Vec<i64> = attention_mask.iter().copied().collect();
        let token_type_ids_vec: Vec<i64> = token_type_ids.iter().copied().collect();

        // ONNX 추론
        let input_ids_value = Value::from_array((shape, input_ids_vec))
            .map_err(|e: ort::Error| RerankerError::OrtError(e.to_string()))?;
        let attention_mask_value = Value::from_array((shape, attention_mask_vec))
            .map_err(|e: ort::Error| RerankerError::OrtError(e.to_string()))?;
        let token_type_ids_value = Value::from_array((shape, token_type_ids_vec))
            .map_err(|e: ort::Error| RerankerError::OrtError(e.to_string()))?;

        let scores = {
            let mut session = self.session.lock().map_err(|_| RerankerError::LockFailed)?;
            let outputs = session
                .run(ort::inputs![
                    "input_ids" => input_ids_value,
                    "attention_mask" => attention_mask_value,
                    "token_type_ids" => token_type_ids_value,
                ])
                .map_err(|e: ort::Error| RerankerError::OrtError(e.to_string()))?;

            // 출력에서 logits 추출
            // Cross-Encoder 출력: [batch_size, 1] 또는 [batch_size]
            let output = outputs
                .get("logits")
                .ok_or_else(|| RerankerError::OrtError("No logits output".to_string()))?;

            let (_, out_data) = output
                .try_extract_tensor::<f32>()
                .map_err(|e: ort::Error| RerankerError::OrtError(e.to_string()))?;

            // 각 샘플의 점수 추출
            out_data.iter().take(batch_size).copied().collect()
        };

        Ok(scores)
    }

    /// 점수 기반 재정렬 (상위 K개의 인덱스 반환)
    ///
    /// 반환값: 점수가 높은 순서대로 정렬된 원본 인덱스
    pub fn rerank(
        &self,
        query: &str,
        documents: &[&str],
        top_k: usize,
    ) -> Result<Vec<usize>, RerankerError> {
        let scores = self.score(query, documents)?;

        // (원본 인덱스, 점수) 쌍 생성
        let mut indexed_scores: Vec<(usize, f32)> = scores
            .into_iter()
            .enumerate()
            .collect();

        // 점수 기준 내림차순 정렬
        indexed_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // 상위 K개의 인덱스 반환
        Ok(indexed_scores
            .into_iter()
            .take(top_k)
            .map(|(idx, _)| idx)
            .collect())
    }

    /// 점수 기반 재정렬 (점수와 함께 반환)
    pub fn rerank_with_scores(
        &self,
        query: &str,
        documents: &[&str],
        top_k: usize,
    ) -> Result<Vec<(usize, f32)>, RerankerError> {
        let scores = self.score(query, documents)?;

        let mut indexed_scores: Vec<(usize, f32)> = scores
            .into_iter()
            .enumerate()
            .collect();

        indexed_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        Ok(indexed_scores.into_iter().take(top_k).collect())
    }
}

unsafe impl Send for Reranker {}
unsafe impl Sync for Reranker {}

#[cfg(test)]
mod tests {
    use super::*;

    // 모델이 있을 때만 테스트 실행
    #[test]
    #[ignore = "requires model files"]
    fn test_rerank() {
        let model_path = Path::new("models/ms-marco-MiniLM-L6-v2/model.onnx");
        let tokenizer_path = Path::new("models/ms-marco-MiniLM-L6-v2/tokenizer.json");

        let reranker = Reranker::new(model_path, tokenizer_path).unwrap();

        let query = "What is the capital of France?";
        let documents = [
            "Paris is the capital of France.",
            "Berlin is the capital of Germany.",
            "France is a country in Europe.",
        ];

        let result = reranker.rerank(query, &documents, 3).unwrap();
        println!("Reranked indices: {:?}", result);

        // Paris 문서가 1위여야 함
        assert_eq!(result[0], 0);
    }
}
