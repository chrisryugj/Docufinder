//! SQLite Persistence Adapters

mod chunk_repository;
mod file_repository;

// NOTE: Phase 2에서 Clean Architecture 전환 시 사용 예정
#[allow(unused_imports)]
pub use chunk_repository::SqliteChunkRepository;
#[allow(unused_imports)]
pub use file_repository::SqliteFileRepository;
