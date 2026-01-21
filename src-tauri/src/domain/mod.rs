//! Domain Layer - 비즈니스 로직의 핵심
//!
//! 클린 아키텍처의 가장 안쪽 레이어로, 외부 의존성이 없어야 합니다.
//!
//! ## 구성요소
//! - **Entities**: 비즈니스 로직을 포함하는 도메인 객체 (File, Chunk, Folder)
//! - **Value Objects**: 불변, 동등성으로 비교되는 값 객체 (FileId, ChunkId, Embedding)
//! - **Repository Traits**: 데이터 접근 추상화 (DIP 적용)
//! - **Domain Errors**: 비즈니스 규칙 위반 에러

pub mod entities;
pub mod errors;
pub mod repositories;
pub mod value_objects;

// Re-exports for convenience
pub use entities::{Chunk, File, FileType, WatchedFolder};
pub use errors::DomainError;
pub use repositories::{
    ChunkRepository, EmbedderPort, FileRepository, VectorRepository, VectorSearchResult,
};
pub use value_objects::{ChunkId, Embedding, FileId, EMBEDDING_DIM};
