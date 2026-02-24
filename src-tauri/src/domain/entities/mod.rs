//! Domain Entities - 비즈니스 로직을 포함하는 도메인 객체

mod chunk;
mod file;
mod folder;

// NOTE: Phase 2에서 Clean Architecture 전환 시 사용 예정
#[allow(unused_imports)]
pub use chunk::Chunk;
#[allow(unused_imports)]
pub use file::{File, FileType};
#[allow(unused_imports)]
pub use folder::WatchedFolder;
