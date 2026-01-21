//! Chunk Entity - 파일의 텍스트 청크를 나타내는 도메인 엔티티

use crate::domain::errors::DomainError;
use crate::domain::value_objects::{ChunkId, FileId};

/// 청크 크기 제한
pub const MIN_CHUNK_SIZE: usize = 10;
pub const MAX_CHUNK_SIZE: usize = 2000;

/// 청크 엔티티 (비즈니스 로직 포함)
#[derive(Debug, Clone)]
pub struct Chunk {
    id: ChunkId,
    file_id: FileId,
    chunk_index: usize,
    content: String,
    start_offset: usize,
    end_offset: usize,
    page_number: Option<usize>,
    location_hint: Option<String>,
}

impl Chunk {
    /// 새 청크 엔티티 생성 (도메인 규칙 검증 포함)
    pub fn new(
        file_id: FileId,
        chunk_index: usize,
        content: String,
        start_offset: usize,
        end_offset: usize,
        page_number: Option<usize>,
        location_hint: Option<String>,
    ) -> Result<Self, DomainError> {
        // 도메인 규칙 검증
        if content.is_empty() {
            return Err(DomainError::EmptyChunk);
        }

        if end_offset <= start_offset {
            return Err(DomainError::InvalidChunkRange {
                start: start_offset,
                end: end_offset,
            });
        }

        Ok(Self {
            id: ChunkId::new(0), // DB 저장 전까지 0
            file_id,
            chunk_index,
            content,
            start_offset,
            end_offset,
            page_number,
            location_hint,
        })
    }

    /// DB에서 로드할 때 사용 (모든 필드 지정)
    pub fn reconstitute(
        id: ChunkId,
        file_id: FileId,
        chunk_index: usize,
        content: String,
        start_offset: usize,
        end_offset: usize,
        page_number: Option<usize>,
        location_hint: Option<String>,
    ) -> Self {
        Self {
            id,
            file_id,
            chunk_index,
            content,
            start_offset,
            end_offset,
            page_number,
            location_hint,
        }
    }

    // === Getters ===

    pub fn id(&self) -> ChunkId {
        self.id
    }

    pub fn file_id(&self) -> FileId {
        self.file_id
    }

    pub fn chunk_index(&self) -> usize {
        self.chunk_index
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn start_offset(&self) -> usize {
        self.start_offset
    }

    pub fn end_offset(&self) -> usize {
        self.end_offset
    }

    pub fn page_number(&self) -> Option<usize> {
        self.page_number
    }

    pub fn location_hint(&self) -> Option<&str> {
        self.location_hint.as_deref()
    }

    // === 비즈니스 로직 ===

    /// ID 설정 (DB 저장 후)
    pub fn set_id(&mut self, id: ChunkId) {
        self.id = id;
    }

    /// 청크 길이 반환
    pub fn len(&self) -> usize {
        self.content.len()
    }

    /// 빈 청크인지 확인
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// 유효한 크기인지 확인 (10-2000자)
    pub fn is_valid_size(&self) -> bool {
        let len = self.content.len();
        len >= MIN_CHUNK_SIZE && len <= MAX_CHUNK_SIZE
    }

    /// 청크에 텍스트가 포함되어 있는지 확인
    pub fn contains(&self, text: &str) -> bool {
        self.content.to_lowercase().contains(&text.to_lowercase())
    }

    /// 텍스트에서 매칭 위치 찾기
    pub fn find_matches(&self, query: &str) -> Vec<(usize, usize)> {
        let content_lower = self.content.to_lowercase();
        let query_lower = query.to_lowercase();
        let mut matches = Vec::new();
        let mut start = 0;

        while let Some(pos) = content_lower[start..].find(&query_lower) {
            let actual_pos = start + pos;
            matches.push((actual_pos, actual_pos + query.len()));
            start = actual_pos + 1;
        }

        matches
    }

    /// 미리보기 텍스트 생성 (최대 길이 지정)
    pub fn preview(&self, max_len: usize) -> String {
        if self.content.len() <= max_len {
            self.content.clone()
        } else {
            format!("{}...", &self.content[..max_len])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_creation() {
        let chunk = Chunk::new(
            FileId::new(1),
            0,
            "Hello World".to_string(),
            0,
            11,
            Some(1),
            Some("Section 1".to_string()),
        )
        .unwrap();

        assert_eq!(chunk.content(), "Hello World");
        assert_eq!(chunk.chunk_index(), 0);
        assert_eq!(chunk.page_number(), Some(1));
    }

    #[test]
    fn test_chunk_validation() {
        // 빈 내용
        assert!(Chunk::new(FileId::new(1), 0, "".to_string(), 0, 1, None, None).is_err());

        // 잘못된 오프셋 범위
        assert!(
            Chunk::new(FileId::new(1), 0, "Hello".to_string(), 10, 5, None, None).is_err()
        );
    }

    #[test]
    fn test_chunk_contains() {
        let chunk = Chunk::new(
            FileId::new(1),
            0,
            "Hello World".to_string(),
            0,
            11,
            None,
            None,
        )
        .unwrap();

        assert!(chunk.contains("hello")); // 대소문자 무시
        assert!(chunk.contains("World"));
        assert!(!chunk.contains("foo"));
    }

    #[test]
    fn test_find_matches() {
        let chunk = Chunk::new(
            FileId::new(1),
            0,
            "hello world, hello everyone".to_string(),
            0,
            27,
            None,
            None,
        )
        .unwrap();

        let matches = chunk.find_matches("hello");
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0], (0, 5));
        assert_eq!(matches[1], (13, 18));
    }

    #[test]
    fn test_preview() {
        let chunk = Chunk::new(
            FileId::new(1),
            0,
            "This is a very long text that needs to be truncated".to_string(),
            0,
            51,
            None,
            None,
        )
        .unwrap();

        let preview = chunk.preview(20);
        assert!(preview.ends_with("..."));
        assert!(preview.len() <= 23); // 20 + "..."
    }
}
