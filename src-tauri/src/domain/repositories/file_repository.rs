//! FileRepository Trait - 파일 데이터 접근 추상화

use crate::domain::entities::File;
use crate::domain::errors::DomainError;
use crate::domain::value_objects::FileId;
use async_trait::async_trait;

/// 파일 리포지토리 트레이트
#[async_trait]
pub trait FileRepository: Send + Sync {
    /// 파일 저장 (INSERT or UPDATE)
    async fn save(&self, file: &mut File) -> Result<FileId, DomainError>;

    /// ID로 파일 조회
    async fn find_by_id(&self, id: FileId) -> Result<Option<File>, DomainError>;

    /// 경로로 파일 조회
    async fn find_by_path(&self, path: &str) -> Result<Option<File>, DomainError>;

    /// 폴더 내 모든 파일 조회
    async fn find_in_folder(&self, folder_path: &str) -> Result<Vec<File>, DomainError>;

    /// 벡터 인덱싱 대기 중인 파일 조회
    async fn find_pending_vector_files(&self, limit: usize) -> Result<Vec<File>, DomainError>;

    /// 파일 삭제
    async fn delete(&self, id: FileId) -> Result<(), DomainError>;

    /// 경로로 파일 삭제
    async fn delete_by_path(&self, path: &str) -> Result<(), DomainError>;

    /// 폴더 내 모든 파일 삭제
    async fn delete_in_folder(&self, folder_path: &str) -> Result<usize, DomainError>;

    /// 전체 파일 수
    async fn count(&self) -> Result<usize, DomainError>;

    /// FTS 인덱싱 완료 표시
    async fn mark_fts_indexed(&self, id: FileId, timestamp: i64) -> Result<(), DomainError>;

    /// 벡터 인덱싱 완료 표시
    async fn mark_vector_indexed(&self, id: FileId, timestamp: i64) -> Result<(), DomainError>;

    /// 파일 존재 여부 확인
    async fn exists(&self, path: &str) -> Result<bool, DomainError>;
}
