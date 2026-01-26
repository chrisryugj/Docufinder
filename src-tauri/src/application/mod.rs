//! Application Layer - 유스케이스와 비즈니스 로직
//!
//! Domain Layer와 Infrastructure Layer 사이를 조율합니다.
//!
//! ## 구성요소
//! - **container**: DI 컨테이너 (AppContainer)
//! - **dto**: 데이터 전송 객체 (SearchQuery, SearchResponse 등)
//! - **services**: 비즈니스 로직 서비스 (SearchService, IndexService 등)
//! - **errors**: 애플리케이션 레벨 에러

pub mod container;
pub mod dto;
pub mod errors;
pub mod services;
