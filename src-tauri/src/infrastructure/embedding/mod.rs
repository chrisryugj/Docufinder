//! OnnxEmbedderAdapter - EmbedderPort trait의 ONNX 구현체

use crate::domain::errors::DomainError;
use crate::domain::repositories::EmbedderPort;
use crate::domain::value_objects::{Embedding, EMBEDDING_DIM};
use crate::embedder::Embedder;
use async_trait::async_trait;
use std::path::Path;

/// ONNX 기반 임베딩 어댑터
///
/// 기존 Embedder를 EmbedderPort trait으로 감싸는 어댑터 패턴
/// Embedder가 &self로 호출 가능하므로 락 불필요
pub struct OnnxEmbedderAdapter {
    embedder: Embedder,
}

impl OnnxEmbedderAdapter {
    /// 새 어댑터 생성
    pub fn new(model_path: &Path, tokenizer_path: &Path) -> Result<Self, DomainError> {
        let embedder = Embedder::new(model_path, tokenizer_path).map_err(|e| {
            DomainError::EmbeddingError {
                reason: e.to_string(),
            }
        })?;

        Ok(Self { embedder })
    }

    /// 기존 Embedder로부터 생성 (마이그레이션용)
    pub fn from_embedder(embedder: Embedder) -> Self {
        Self { embedder }
    }
}

#[async_trait]
impl EmbedderPort for OnnxEmbedderAdapter {
    async fn embed(&self, text: &str, is_query: bool) -> Result<Embedding, DomainError> {
        let vector = self.embedder.embed(text, is_query).map_err(|e| DomainError::EmbeddingError {
            reason: e.to_string(),
        })?;

        Embedding::new(vector)
    }

    async fn embed_batch(
        &self,
        texts: &[String],
        _is_query: bool,
    ) -> Result<Vec<Embedding>, DomainError> {
        if texts.is_empty() {
            return Ok(vec![]);
        }

        // KoSimCSE는 접두사 불필요
        let prepared: Vec<String> = texts.iter().map(|t| t.to_string()).collect();

        let vectors = self.embedder.embed_batch(&prepared).map_err(|e| DomainError::EmbeddingError {
            reason: e.to_string(),
        })?;

        vectors.into_iter().map(Embedding::new).collect()
    }

    fn is_available(&self) -> bool {
        true // 락이 없으므로 항상 사용 가능
    }

    fn dimension(&self) -> usize {
        EMBEDDING_DIM
    }

    fn max_batch_size(&self) -> usize {
        128 // 배치 크기 증가
    }
}
