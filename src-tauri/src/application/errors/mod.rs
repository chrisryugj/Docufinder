//! Application Layer Errors
//!
//! 비즈니스 로직 실행 중 발생하는 에러

use crate::domain::DomainError;
use std::fmt;

/// Application Layer 에러
#[derive(Debug)]
pub enum AppError {
    /// 도메인 규칙 위반
    Domain(DomainError),
    /// 검색어가 비어있음
    EmptyQuery,
    /// 경로를 찾을 수 없음
    PathNotFound(String),
    /// 잘못된 경로
    InvalidPath(String),
    /// 접근 거부 (시스템 폴더)
    AccessDenied(String),
    /// 인덱싱 실패
    IndexingFailed(String),
    /// 검색 실패
    SearchFailed(String),
    /// 임베딩 실패
    EmbeddingFailed(String),
    /// 벡터 인덱스가 비어있음
    VectorIndexEmpty,
    /// 시맨틱 검색 비활성화 (모델 없음)
    SemanticSearchDisabled,
    /// AI 에러 (Gemini API)
    AiError(String),
    /// 내부 에러
    Internal(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Domain(e) => write!(f, "Domain error: {}", e),
            AppError::EmptyQuery => write!(f, "Search query is empty"),
            AppError::PathNotFound(p) => write!(f, "Path not found: {}", p),
            AppError::InvalidPath(p) => write!(f, "Invalid path: {}", p),
            AppError::AccessDenied(p) => write!(f, "Access denied: {}", p),
            AppError::IndexingFailed(e) => write!(f, "Indexing failed: {}", e),
            AppError::SearchFailed(e) => write!(f, "Search failed: {}", e),
            AppError::EmbeddingFailed(e) => write!(f, "Embedding failed: {}", e),
            AppError::VectorIndexEmpty => write!(f, "Vector index is empty"),
            AppError::SemanticSearchDisabled => {
                write!(f, "Semantic search is disabled (model not found)")
            }
            AppError::AiError(e) => write!(f, "AI error: {}", e),
            AppError::Internal(e) => write!(f, "Internal error: {}", e),
        }
    }
}

impl std::error::Error for AppError {}

impl From<DomainError> for AppError {
    fn from(e: DomainError) -> Self {
        AppError::Domain(e)
    }
}

/// Application 결과 타입
pub type AppResult<T> = Result<T, AppError>;
