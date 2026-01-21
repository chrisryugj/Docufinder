//! Value Objects - 불변, 동등성으로 비교되는 도메인 객체

mod file_id;
mod chunk_id;
mod embedding;

pub use file_id::FileId;
pub use chunk_id::ChunkId;
pub use embedding::{Embedding, EMBEDDING_DIM};
