//! SQLite Persistence Adapters

mod file_repository;
mod chunk_repository;

// NOTE: Phase 2에서 Clean Architecture 전환 시 사용 예정
#[allow(unused_imports)]
pub use file_repository::SqliteFileRepository;
#[allow(unused_imports)]
pub use chunk_repository::SqliteChunkRepository;
