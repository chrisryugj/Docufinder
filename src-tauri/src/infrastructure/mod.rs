//! Infrastructure Layer - 외부 시스템 어댑터
//!
//! 클린 아키텍처의 가장 바깥쪽 레이어로, Domain Layer의 추상화를 구현합니다.
//!
//! ## 구성요소
//! - **persistence**: SQLite 리포지토리 구현체 (SqliteFileRepository, SqliteChunkRepository)
//! - **vector**: usearch 벡터 인덱스 어댑터 (UsearchVectorRepository)
//! - **embedding**: ONNX 임베딩 어댑터 (OnnxEmbedderAdapter)
//!
//! NOTE: Phase 2에서 Clean Architecture 전환 시 활용 예정
#![allow(dead_code)]

pub mod embedding;
pub mod persistence;
pub mod vector;
