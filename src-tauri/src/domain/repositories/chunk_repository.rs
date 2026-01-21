//! ChunkRepository Trait - 청크 데이터 접근 추상화

use crate::domain::entities::Chunk;
use crate::domain::errors::DomainError;
use crate::domain::value_objects::{ChunkId, FileId};
use async_trait::async_trait;

/// FTS 검색 결과
#[derive(Debug, Clone)]
pub struct FtsSearchResult {
    pub chunk_id: ChunkId,
    pub file_id: FileId,
    pub content: String,
    pub score: f32,
    pub highlight_ranges: Vec<(usize, usize)>,
}

/// 청크 리포지토리 트레이트
#[async_trait]
pub trait ChunkRepository: Send + Sync {
    /// 청크 저장
    async fn save(&self, chunk: &mut Chunk) -> Result<ChunkId, DomainError>;

    /// 청크 배치 저장 (성능 최적화)
    async fn save_batch(&self, chunks: &mut [Chunk]) -> Result<Vec<ChunkId>, DomainError>;

    /// ID로 청크 조회
    async fn find_by_id(&self, id: ChunkId) -> Result<Option<Chunk>, DomainError>;

    /// 여러 ID로 청크 조회
    async fn find_by_ids(&self, ids: &[ChunkId]) -> Result<Vec<Chunk>, DomainError>;

    /// 파일의 모든 청크 조회
    async fn find_by_file_id(&self, file_id: FileId) -> Result<Vec<Chunk>, DomainError>;

    /// 청크 삭제
    async fn delete(&self, id: ChunkId) -> Result<(), DomainError>;

    /// 파일의 모든 청크 삭제
    async fn delete_by_file_id(&self, file_id: FileId) -> Result<usize, DomainError>;

    /// 폴더 내 모든 청크 삭제
    async fn delete_in_folder(&self, folder_path: &str) -> Result<usize, DomainError>;

    /// FTS 검색
    async fn search_fts(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<FtsSearchResult>, DomainError>;

    /// 전체 청크 수
    async fn count(&self) -> Result<usize, DomainError>;

    /// 파일의 청크 수
    async fn count_by_file_id(&self, file_id: FileId) -> Result<usize, DomainError>;
}
