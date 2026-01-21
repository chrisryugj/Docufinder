//! Infrastructure Layer - 외부 시스템 어댑터
//!
//! 클린 아키텍처의 가장 바깥쪽 레이어로, Domain Layer의 추상화를 구현합니다.
//!
//! ## 구성요소
//! - **persistence**: SQLite 리포지토리 구현체 (SqliteFileRepository, SqliteChunkRepository)
//! - **vector**: usearch 벡터 인덱스 어댑터 (UsearchVectorRepository)
//! - **embedding**: ONNX 임베딩 어댑터 (OnnxEmbedderAdapter)

pub mod embedding;
pub mod persistence;
pub mod vector;

// Re-exports
pub use embedding::OnnxEmbedderAdapter;
pub use persistence::{SqliteChunkRepository, SqliteFileRepository};
pub use vector::UsearchVectorRepository;
