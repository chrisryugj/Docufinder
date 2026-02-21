//! Embedding Value Object - 벡터 임베딩의 타입 안전성 보장

use crate::domain::errors::DomainError;

/// 임베딩 차원 (KoSimCSE-roberta-multitask: 768)
pub const EMBEDDING_DIM: usize = 768;

/// 임베딩 벡터 (불변, 검증된)
#[derive(Debug, Clone)]
pub struct Embedding {
    vector: Vec<f32>,
}

impl Embedding {
    /// 새 Embedding 생성 (차원 검증)
    pub fn new(vector: Vec<f32>) -> Result<Self, DomainError> {
        if vector.len() != EMBEDDING_DIM {
            return Err(DomainError::InvalidEmbeddingDimension {
                expected: EMBEDDING_DIM,
                actual: vector.len(),
            });
        }
        Ok(Self { vector })
    }

    /// 내부 벡터 참조 반환
    pub fn as_slice(&self) -> &[f32] {
        &self.vector
    }

    /// 내부 벡터 소유권 반환
    pub fn into_vec(self) -> Vec<f32> {
        self.vector
    }

    /// 차원 반환
    pub fn dimension(&self) -> usize {
        self.vector.len()
    }

    /// 코사인 유사도 계산
    pub fn cosine_similarity(&self, other: &Embedding) -> f32 {
        let dot: f32 = self
            .vector
            .iter()
            .zip(&other.vector)
            .map(|(a, b)| a * b)
            .sum();

        let norm_a = self.vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b = other.vector.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        dot / (norm_a * norm_b)
    }

    /// L2 정규화
    pub fn normalize(&mut self) {
        let norm: f32 = self.vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut self.vector {
                *x /= norm;
            }
        }
    }

    /// 제로 벡터 여부 확인
    pub fn is_zero(&self) -> bool {
        self.vector.iter().all(|&x| x == 0.0)
    }
}

impl PartialEq for Embedding {
    fn eq(&self, other: &Self) -> bool {
        self.vector == other.vector
    }
}

impl TryFrom<Vec<f32>> for Embedding {
    type Error = DomainError;

    fn try_from(vector: Vec<f32>) -> Result<Self, Self::Error> {
        Embedding::new(vector)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_creation() {
        let vec = vec![0.1; EMBEDDING_DIM];
        let embedding = Embedding::new(vec).unwrap();
        assert_eq!(embedding.dimension(), EMBEDDING_DIM);
    }

    #[test]
    fn test_embedding_invalid_dimension() {
        let vec = vec![0.1; 100]; // 잘못된 차원
        let result = Embedding::new(vec);
        assert!(result.is_err());
    }

    #[test]
    fn test_cosine_similarity() {
        let vec1 = vec![1.0, 0.0, 0.0];
        let vec2 = vec![1.0, 0.0, 0.0];
        let vec3 = vec![0.0, 1.0, 0.0];

        // 테스트용으로 차원 검증 우회
        let e1 = Embedding { vector: vec1 };
        let e2 = Embedding { vector: vec2 };
        let e3 = Embedding { vector: vec3 };

        assert!((e1.cosine_similarity(&e2) - 1.0).abs() < 0.001);
        assert!(e1.cosine_similarity(&e3).abs() < 0.001);
    }

    #[test]
    fn test_normalize() {
        let mut embedding = Embedding {
            vector: vec![3.0, 4.0],
        };
        embedding.normalize();

        let expected_norm: f32 = embedding
            .vector
            .iter()
            .map(|x| x * x)
            .sum::<f32>()
            .sqrt();
        assert!((expected_norm - 1.0).abs() < 0.001);
    }
}
