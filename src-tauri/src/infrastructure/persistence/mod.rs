//! SQLite Persistence Adapters

mod file_repository;
mod chunk_repository;

pub use file_repository::SqliteFileRepository;
pub use chunk_repository::SqliteChunkRepository;
