//! EmbedderPort - 임베딩 생성 포트 (Hexagonal Architecture)

use crate::domain::errors::DomainError;
use crate::domain::value_objects::Embedding;
use async_trait::async_trait;

/// 임베딩 생성 포트 (추상화)
#[async_trait]
pub trait EmbedderPort: Send + Sync {
    /// 단일 텍스트 임베딩 생성
    ///
    /// # Arguments
    /// * `text` - 임베딩할 텍스트
    /// * `is_query` - 쿼리 텍스트 여부 (프롬프트 prefix 결정)
    async fn embed(&self, text: &str, is_query: bool) -> Result<Embedding, DomainError>;

    /// 배치 텍스트 임베딩 생성
    ///
    /// # Arguments
    /// * `texts` - 임베딩할 텍스트 목록
    /// * `is_query` - 쿼리 텍스트 여부
    async fn embed_batch(
        &self,
        texts: &[String],
        is_query: bool,
    ) -> Result<Vec<Embedding>, DomainError>;

    /// 임베딩 모델 사용 가능 여부
    fn is_available(&self) -> bool;

    /// 모델 차원 반환
    fn dimension(&self) -> usize;

    /// 최대 배치 크기 반환
    fn max_batch_size(&self) -> usize;
}
