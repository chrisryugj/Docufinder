//! 텍스트 임베딩 모듈 (시맨틱 검색 빌드 시 활성화)
//!
//! 현재 빌드 의존성 문제로 비활성화됨

use thiserror::Error;

pub const EMBEDDING_DIM: usize = 384;

#[derive(Error, Debug)]
pub enum EmbedderError {
    #[error("Semantic search not available")]
    NotAvailable,
}

/// 텍스트 임베딩 생성기 (stub)
pub struct Embedder;

impl Embedder {
    pub fn new(_model_path: &std::path::Path, _tokenizer_path: &std::path::Path) -> Result<Self, EmbedderError> {
        Err(EmbedderError::NotAvailable)
    }

    pub fn embed(&self, _text: &str, _is_query: bool) -> Result<Vec<f32>, EmbedderError> {
        Err(EmbedderError::NotAvailable)
    }

    pub fn embed_batch(&self, _texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedderError> {
        Err(EmbedderError::NotAvailable)
    }
}

unsafe impl Send for Embedder {}
unsafe impl Sync for Embedder {}
