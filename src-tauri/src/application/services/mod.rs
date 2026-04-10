//! Application Services - 비즈니스 로직
//!
//! Repository Trait을 통해 데이터 접근하며, 비즈니스 규칙 실행
//!
//! NOTE: Phase 2에서 Commands → Services 전환 시 활용 예정
#![allow(dead_code)]

mod folder_service;
mod index_service;
pub(crate) mod search_service;

pub use folder_service::FolderService;
pub use index_service::IndexService;
pub use search_service::SearchService;
