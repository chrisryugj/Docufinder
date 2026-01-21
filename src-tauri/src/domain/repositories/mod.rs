//! Repository Traits - 데이터 접근 추상화 (DIP)

mod file_repository;
mod chunk_repository;
mod vector_repository;
mod embedder_port;

pub use file_repository::FileRepository;
pub use chunk_repository::{ChunkRepository, FtsSearchResult};
pub use vector_repository::{VectorRepository, VectorSearchResult};
pub use embedder_port::EmbedderPort;
