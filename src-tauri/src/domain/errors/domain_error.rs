//! Domain Error - 도메인 레이어의 에러 타입

use thiserror::Error;

/// 도메인 레이어 에러
#[derive(Error, Debug)]
pub enum DomainError {
    // === 파일 관련 ===
    #[error("유효하지 않은 파일 경로: {path}")]
    InvalidPath { path: String },

    #[error("파일을 찾을 수 없음: {path}")]
    FileNotFound { path: String },

    #[error("지원하지 않는 파일 형식: {extension}")]
    UnsupportedFileType { extension: String },

    #[error("파일 크기가 유효하지 않음: {size}")]
    InvalidFileSize { size: i64 },

    #[error("접근이 거부된 경로: {path}")]
    ForbiddenPath { path: String },

    // === 청크 관련 ===
    #[error("빈 청크는 허용되지 않음")]
    EmptyChunk,

    #[error("유효하지 않은 청크 범위: start={start}, end={end}")]
    InvalidChunkRange { start: usize, end: usize },

    #[error("청크 인덱스가 범위를 벗어남: {index}")]
    ChunkIndexOutOfBounds { index: usize },

    // === 임베딩 관련 ===
    #[error("유효하지 않은 임베딩 차원: 기대={expected}, 실제={actual}")]
    InvalidEmbeddingDimension { expected: usize, actual: usize },

    #[error("임베딩 생성 실패: {reason}")]
    EmbeddingFailed { reason: String },

    // === 폴더 관련 ===
    #[error("폴더를 찾을 수 없음: {path}")]
    FolderNotFound { path: String },

    #[error("이미 등록된 폴더: {path}")]
    FolderAlreadyExists { path: String },

    // === 리포지토리 관련 ===
    #[error("리포지토리 에러: {message}")]
    RepositoryError { message: String },

    #[error("데이터를 찾을 수 없음: {entity} (id={id})")]
    NotFound { entity: String, id: String },

    // === 검색 관련 ===
    #[error("검색 쿼리가 비어있음")]
    EmptySearchQuery,

    #[error("검색 결과 없음")]
    NoSearchResults,

    // === 벡터 인덱스 관련 ===
    #[error("벡터 인덱스 에러 ({operation}): {reason}")]
    VectorIndexError { operation: String, reason: String },

    // === 임베딩 어댑터 관련 ===
    #[error("임베딩 에러: {reason}")]
    EmbeddingError { reason: String },

    // === 일반 ===
    #[error("도메인 규칙 위반: {rule}")]
    BusinessRuleViolation { rule: String },

    #[error("유효성 검증 실패: {field} - {reason}")]
    ValidationError { field: String, reason: String },
}

impl DomainError {
    /// 리포지토리 에러 생성 헬퍼
    pub fn repository(message: impl Into<String>) -> Self {
        Self::RepositoryError {
            message: message.into(),
        }
    }

    /// NotFound 에러 생성 헬퍼
    pub fn not_found(entity: impl Into<String>, id: impl Into<String>) -> Self {
        Self::NotFound {
            entity: entity.into(),
            id: id.into(),
        }
    }

    /// 비즈니스 규칙 위반 에러 생성 헬퍼
    pub fn business_rule(rule: impl Into<String>) -> Self {
        Self::BusinessRuleViolation { rule: rule.into() }
    }

    /// 유효성 검증 에러 생성 헬퍼
    pub fn validation(field: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::ValidationError {
            field: field.into(),
            reason: reason.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = DomainError::InvalidPath {
            path: "/invalid/path".to_string(),
        };
        assert!(err.to_string().contains("/invalid/path"));
    }

    #[test]
    fn test_helper_methods() {
        let err = DomainError::repository("DB 연결 실패");
        assert!(matches!(err, DomainError::RepositoryError { .. }));

        let err = DomainError::not_found("File", "123");
        assert!(matches!(err, DomainError::NotFound { .. }));
    }
}
