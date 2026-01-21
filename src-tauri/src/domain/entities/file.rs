//! File Entity - 인덱싱된 파일을 나타내는 도메인 엔티티

use crate::domain::errors::DomainError;
use crate::domain::value_objects::FileId;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// 지원되는 파일 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileType {
    Hwpx,
    Docx,
    Xlsx,
    Pdf,
    Txt,
    Unknown,
}

impl FileType {
    /// 확장자로부터 FileType 결정
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "hwpx" | "hwp" => Self::Hwpx,
            "docx" | "doc" => Self::Docx,
            "xlsx" | "xls" => Self::Xlsx,
            "pdf" => Self::Pdf,
            "txt" | "md" | "log" => Self::Txt,
            _ => Self::Unknown,
        }
    }

    /// 지원되는 파일 타입인지 확인
    pub fn is_supported(&self) -> bool {
        !matches!(self, Self::Unknown)
    }

    /// 확장자 문자열 반환
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Hwpx => "hwpx",
            Self::Docx => "docx",
            Self::Xlsx => "xlsx",
            Self::Pdf => "pdf",
            Self::Txt => "txt",
            Self::Unknown => "unknown",
        }
    }
}

/// 파일 엔티티 (비즈니스 로직 포함)
#[derive(Debug, Clone)]
pub struct File {
    id: FileId,
    path: String,
    name: String,
    file_type: FileType,
    size: i64,
    modified_at: i64,
    fts_indexed_at: Option<i64>,
    vector_indexed_at: Option<i64>,
}

impl File {
    /// 새 파일 엔티티 생성 (도메인 규칙 검증 포함)
    pub fn new(
        path: String,
        name: String,
        file_type: FileType,
        size: i64,
        modified_at: i64,
    ) -> Result<Self, DomainError> {
        // 도메인 규칙 검증
        if path.is_empty() {
            return Err(DomainError::InvalidPath {
                path: "빈 경로".to_string(),
            });
        }

        if name.is_empty() {
            return Err(DomainError::ValidationError {
                field: "name".to_string(),
                reason: "파일명이 비어있음".to_string(),
            });
        }

        if size < 0 {
            return Err(DomainError::InvalidFileSize { size });
        }

        if !file_type.is_supported() {
            return Err(DomainError::UnsupportedFileType {
                extension: file_type.as_str().to_string(),
            });
        }

        Ok(Self {
            id: FileId::new(0), // DB 저장 전까지 0
            path,
            name,
            file_type,
            size,
            modified_at,
            fts_indexed_at: None,
            vector_indexed_at: None,
        })
    }

    /// 경로로부터 파일 엔티티 생성
    pub fn from_path(path: &Path, modified_at: i64, size: i64) -> Result<Self, DomainError> {
        let path_str = path.to_string_lossy().to_string();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_default();
        let file_type = FileType::from_extension(&ext);

        Self::new(path_str, name, file_type, size, modified_at)
    }

    /// DB에서 로드할 때 사용 (모든 필드 지정)
    pub fn reconstitute(
        id: FileId,
        path: String,
        name: String,
        file_type: FileType,
        size: i64,
        modified_at: i64,
        fts_indexed_at: Option<i64>,
        vector_indexed_at: Option<i64>,
    ) -> Self {
        Self {
            id,
            path,
            name,
            file_type,
            size,
            modified_at,
            fts_indexed_at,
            vector_indexed_at,
        }
    }

    // === Getters ===

    pub fn id(&self) -> FileId {
        self.id
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn file_type(&self) -> FileType {
        self.file_type
    }

    pub fn size(&self) -> i64 {
        self.size
    }

    pub fn modified_at(&self) -> i64 {
        self.modified_at
    }

    pub fn fts_indexed_at(&self) -> Option<i64> {
        self.fts_indexed_at
    }

    pub fn vector_indexed_at(&self) -> Option<i64> {
        self.vector_indexed_at
    }

    // === 비즈니스 로직 ===

    /// ID 설정 (DB 저장 후)
    pub fn set_id(&mut self, id: FileId) {
        self.id = id;
    }

    /// FTS 인덱싱 완료 표시
    pub fn mark_fts_indexed(&mut self, timestamp: i64) {
        self.fts_indexed_at = Some(timestamp);
    }

    /// 벡터 인덱싱 완료 표시
    pub fn mark_vector_indexed(&mut self, timestamp: i64) {
        self.vector_indexed_at = Some(timestamp);
    }

    /// FTS 인덱싱이 필요한지 확인
    pub fn needs_fts_indexing(&self) -> bool {
        self.fts_indexed_at.is_none()
    }

    /// 벡터 인덱싱이 필요한지 확인 (FTS 인덱싱 완료 후에만)
    pub fn needs_vector_indexing(&self) -> bool {
        self.fts_indexed_at.is_some() && self.vector_indexed_at.is_none()
    }

    /// 재인덱싱이 필요한지 확인 (파일 수정 시간 비교)
    pub fn needs_reindex(&self, current_modified_at: i64) -> bool {
        self.modified_at < current_modified_at
    }

    /// 완전히 인덱싱되었는지 확인
    pub fn is_fully_indexed(&self) -> bool {
        self.fts_indexed_at.is_some() && self.vector_indexed_at.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_creation() {
        let file = File::new(
            "/path/to/file.docx".to_string(),
            "file.docx".to_string(),
            FileType::Docx,
            1024,
            1234567890,
        )
        .unwrap();

        assert_eq!(file.name(), "file.docx");
        assert_eq!(file.file_type(), FileType::Docx);
        assert!(file.needs_fts_indexing());
        assert!(!file.needs_vector_indexing()); // FTS 먼저 해야 함
    }

    #[test]
    fn test_file_validation() {
        // 빈 경로
        assert!(File::new(
            "".to_string(),
            "file.docx".to_string(),
            FileType::Docx,
            1024,
            0
        )
        .is_err());

        // 음수 크기
        assert!(File::new(
            "/path".to_string(),
            "file.docx".to_string(),
            FileType::Docx,
            -1,
            0
        )
        .is_err());

        // 지원하지 않는 파일 타입
        assert!(File::new(
            "/path".to_string(),
            "file.xyz".to_string(),
            FileType::Unknown,
            1024,
            0
        )
        .is_err());
    }

    #[test]
    fn test_indexing_state() {
        let mut file = File::new(
            "/path/to/file.docx".to_string(),
            "file.docx".to_string(),
            FileType::Docx,
            1024,
            1234567890,
        )
        .unwrap();

        // 초기 상태
        assert!(file.needs_fts_indexing());
        assert!(!file.needs_vector_indexing());
        assert!(!file.is_fully_indexed());

        // FTS 인덱싱 완료
        file.mark_fts_indexed(1234567891);
        assert!(!file.needs_fts_indexing());
        assert!(file.needs_vector_indexing());
        assert!(!file.is_fully_indexed());

        // 벡터 인덱싱 완료
        file.mark_vector_indexed(1234567892);
        assert!(!file.needs_fts_indexing());
        assert!(!file.needs_vector_indexing());
        assert!(file.is_fully_indexed());
    }

    #[test]
    fn test_file_type_from_extension() {
        assert_eq!(FileType::from_extension("hwpx"), FileType::Hwpx);
        assert_eq!(FileType::from_extension("DOCX"), FileType::Docx);
        assert_eq!(FileType::from_extension("pdf"), FileType::Pdf);
        assert_eq!(FileType::from_extension("xyz"), FileType::Unknown);
    }
}
