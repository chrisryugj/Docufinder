use super::fts::FtsResult;
use super::vector::VectorResult;

/// Reciprocal Rank Fusion으로 하이브리드 검색 결과 병합
pub fn merge_results(
    fts_results: &[FtsResult],
    vector_results: &[VectorResult],
    k: f32, // RRF 상수, 보통 60
) -> Vec<HybridResult> {
    use std::collections::HashMap;

    let mut scores: HashMap<i64, f32> = HashMap::new();

    // FTS 결과 점수 계산
    for (rank, result) in fts_results.iter().enumerate() {
        let rrf_score = 1.0 / (k + rank as f32 + 1.0);
        *scores.entry(result.chunk_id).or_insert(0.0) += rrf_score;
    }

    // 벡터 검색 결과 점수 계산
    for (rank, result) in vector_results.iter().enumerate() {
        let rrf_score = 1.0 / (k + rank as f32 + 1.0);
        *scores.entry(result.chunk_id).or_insert(0.0) += rrf_score;
    }

    // 점수순 정렬
    let mut results: Vec<HybridResult> = scores
        .into_iter()
        .map(|(chunk_id, score)| HybridResult { chunk_id, score })
        .collect();

    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    results
}

#[derive(Debug, Clone)]
pub struct HybridResult {
    pub chunk_id: i64,
    pub score: f32,
}
