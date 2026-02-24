//! VectorRepository Trait - 벡터 인덱스 접근 추상화

use crate::domain::errors::DomainError;
use crate::domain::value_objects::{ChunkId, Embedding};
use async_trait::async_trait;

/// 벡터 검색 결과
#[derive(Debug, Clone)]
pub struct VectorSearchResult {
    pub chunk_id: ChunkId,
    pub score: f32, // 코사인 유사도 (0.0 ~ 1.0)
}

/// 벡터 리포지토리 트레이트
#[async_trait]
pub trait VectorRepository: Send + Sync {
    /// 벡터 추가
    async fn add(&self, chunk_id: ChunkId, embedding: Embedding) -> Result<(), DomainError>;

    /// 벡터 배치 추가 (성능 최적화)
    async fn add_batch(&self, items: &[(ChunkId, Embedding)]) -> Result<(), DomainError>;

    /// 벡터 삭제
    async fn remove(&self, chunk_id: ChunkId) -> Result<(), DomainError>;

    /// 여러 벡터 삭제
    async fn remove_batch(&self, chunk_ids: &[ChunkId]) -> Result<(), DomainError>;

    /// 벡터 검색
    async fn search(
        &self,
        query_embedding: &Embedding,
        limit: usize,
    ) -> Result<Vec<VectorSearchResult>, DomainError>;

    /// 인덱스 크기 (벡터 수)
    fn size(&self) -> usize;

    /// 인덱스 저장
    async fn save(&self) -> Result<(), DomainError>;

    /// 인덱스 로드
    async fn load(&self) -> Result<(), DomainError>;

    /// 벡터 존재 여부 확인
    fn contains(&self, chunk_id: ChunkId) -> bool;

    /// 인덱스 초기화 (모든 벡터 삭제)
    async fn clear(&self) -> Result<(), DomainError>;
}
