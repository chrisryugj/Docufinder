//! ChunkId Value Object - 청크 식별자의 타입 안전성 보장

use serde::{Deserialize, Serialize};

/// 청크 ID (타입 안전한 래퍼)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChunkId(i64);

impl ChunkId {
    /// 새 ChunkId 생성
    pub fn new(id: i64) -> Self {
        Self(id)
    }

    /// 내부 값 반환
    pub fn value(&self) -> i64 {
        self.0
    }

    /// 유효한 ID인지 확인 (0보다 큰 경우)
    pub fn is_valid(&self) -> bool {
        self.0 > 0
    }
}

impl From<i64> for ChunkId {
    fn from(id: i64) -> Self {
        Self::new(id)
    }
}

impl From<ChunkId> for i64 {
    fn from(id: ChunkId) -> Self {
        id.0
    }
}

impl std::fmt::Display for ChunkId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ChunkId({})", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_id_equality() {
        let id1 = ChunkId::new(1);
        let id2 = ChunkId::new(1);
        let id3 = ChunkId::new(2);

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_chunk_id_validity() {
        assert!(ChunkId::new(1).is_valid());
        assert!(!ChunkId::new(0).is_valid());
        assert!(!ChunkId::new(-1).is_valid());
    }
}
