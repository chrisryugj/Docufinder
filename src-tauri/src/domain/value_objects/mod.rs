//! Value Objects - 불변, 동등성으로 비교되는 도메인 객체

mod chunk_id;
mod embedding;
mod file_id;

pub use chunk_id::ChunkId;
pub use embedding::{Embedding, EMBEDDING_DIM};
pub use file_id::FileId;
