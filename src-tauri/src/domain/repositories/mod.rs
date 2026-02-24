//! Repository Traits - 데이터 접근 추상화 (DIP)

mod chunk_repository;
mod embedder_port;
mod file_repository;
mod vector_repository;

pub use chunk_repository::{ChunkRepository, FtsSearchResult};
pub use embedder_port::EmbedderPort;
pub use file_repository::FileRepository;
pub use vector_repository::{VectorRepository, VectorSearchResult};
