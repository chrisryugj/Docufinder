//! API 에러 타입 정의
//!
//! 모든 Tauri 커맨드에서 일관된 에러 타입 사용

use serde::Serialize;
use thiserror::Error;

/// API 에러 타입
///
/// 프론트엔드에서 `code` 필드로 에러 종류를 구분할 수 있음
#[derive(Debug, Error, Serialize)]
#[serde(tag = "code", content = "message")]
pub enum ApiError {
    // ============ 입력 검증 ============
    /// 입력값 검증 실패
    #[error("{0}")]
    Validation(String),

    // ============ 파일 시스템 ============
    /// 경로를 찾을 수 없음
    #[error("경로를 찾을 수 없습니다: {0}")]
    PathNotFound(String),

    /// 접근 거부 (시스템 폴더 등)
    #[error("접근 거부: {0}")]
    AccessDenied(String),

    /// 잘못된 경로
    #[error("잘못된 경로: {0}")]
    InvalidPath(String),

    // ============ 데이터베이스 ============
    /// DB 연결 실패
    #[error("DB 연결 실패: {0}")]
    DatabaseConnection(String),

    /// DB 쿼리 실패
    #[error("DB 쿼리 실패: {0}")]
    DatabaseQuery(String),

    // ============ 인덱싱 ============
    /// 인덱싱 실패
    #[error("인덱싱 실패: {0}")]
    IndexingFailed(String),

    /// 인덱싱 취소됨
    #[error("인덱싱이 취소되었습니다")]
    IndexingCancelled,

    // ============ 검색 ============
    /// 검색 실패
    #[error("검색 실패: {0}")]
    SearchFailed(String),

    /// 임베딩 생성 실패
    #[error("임베딩 생성 실패: {0}")]
    EmbeddingFailed(String),

    /// 벡터 인덱스 비어있음
    #[error("시맨틱 검색 인덱스가 비어 있습니다. 폴더를 인덱싱해주세요.")]
    VectorIndexEmpty,

    /// 벡터 인덱스 손상됨
    #[error("시맨틱 검색 매핑이 손상되었습니다. 인덱스를 삭제하고 다시 인덱싱해주세요.")]
    VectorIndexCorrupted,

    /// 시맨틱 검색 비활성화
    #[error("시맨틱 검색 모델이 설치되지 않았습니다")]
    SemanticSearchDisabled,

    // ============ 설정 ============
    /// 설정 로드 실패
    #[error("설정 로드 실패: {0}")]
    SettingsLoad(String),

    /// 설정 저장 실패
    #[error("설정 저장 실패: {0}")]
    SettingsSave(String),

    // ============ 내부 에러 ============
    /// 락 획득 실패
    #[error("내부 오류: {0}")]
    LockFailed(String),

    /// 태스크 조인 실패
    #[error("작업 처리 중 오류: {0}")]
    TaskJoinError(String),

    /// 모델을 찾을 수 없음
    #[error("모델을 찾을 수 없습니다: {0}")]
    ModelNotFound(String),

    // ============ AI ============
    /// AI 처리 실패
    #[error("AI 처리 실패: {0}")]
    AiError(String),

    // ============ 서브프로세스/커맨드 ============
    /// 사이드카/커맨드 실행 실패
    #[error("커맨드 실행 실패: {0}")]
    CommandFailed(String),
}

/// API 결과 타입 별칭
pub type ApiResult<T> = Result<T, ApiError>;

// ============ From 구현 ============

impl From<rusqlite::Error> for ApiError {
    fn from(e: rusqlite::Error) -> Self {
        // DB 에러 상세는 로그에만 기록 (사용자에게 스키마 정보 노출 방지)
        tracing::error!("Database error: {}", e);
        ApiError::DatabaseQuery("데이터베이스 처리 중 오류가 발생했습니다".to_string())
    }
}

impl<T> From<std::sync::PoisonError<T>> for ApiError {
    fn from(_e: std::sync::PoisonError<T>) -> Self {
        ApiError::LockFailed("내부 동기화 오류가 발생했습니다".to_string())
    }
}

impl From<std::io::Error> for ApiError {
    fn from(e: std::io::Error) -> Self {
        ApiError::IndexingFailed(format!("파일 처리 오류: {}", e))
    }
}

impl From<tokio::task::JoinError> for ApiError {
    fn from(e: tokio::task::JoinError) -> Self {
        ApiError::TaskJoinError(e.to_string())
    }
}

impl From<crate::application::errors::AppError> for ApiError {
    fn from(e: crate::application::errors::AppError) -> Self {
        use crate::application::errors::AppError;
        match e {
            AppError::Domain(d) => ApiError::IndexingFailed(d.to_string()),
            AppError::EmptyQuery => ApiError::SearchFailed("검색어가 비어있습니다".to_string()),
            AppError::PathNotFound(p) => ApiError::PathNotFound(p),
            AppError::InvalidPath(p) => ApiError::InvalidPath(p),
            AppError::AccessDenied(p) => ApiError::AccessDenied(p),
            AppError::IndexingFailed(e) => ApiError::IndexingFailed(e),
            AppError::SearchFailed(e) => ApiError::SearchFailed(e),
            AppError::EmbeddingFailed(e) => ApiError::EmbeddingFailed(e),
            AppError::VectorIndexEmpty => ApiError::VectorIndexEmpty,
            AppError::SemanticSearchDisabled => ApiError::SemanticSearchDisabled,
            AppError::AiError(e) => ApiError::AiError(e),
            AppError::Internal(e) => ApiError::LockFailed(e),
        }
    }
}
