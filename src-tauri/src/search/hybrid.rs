use super::fts::FtsResult;
use super::vector::VectorResult;

/// Reciprocal Rank Fusion으로 하이브리드 검색 결과 병합
pub fn merge_results(
    fts_results: &[FtsResult],
    vector_results: &[VectorResult],
    k: f32, // RRF 상수, 보통 60
) -> Vec<HybridResult> {
    use std::collections::HashMap;

    let mut scores: HashMap<i64, f32> =
        HashMap::with_capacity(fts_results.len() + vector_results.len());

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

    // total_cmp: partial_cmp+unwrap_or(Equal) 은 NaN 섞이면 전이성 위반 (Rust 1.81+ smallsort panic)
    results.sort_by(|a, b| b.score.total_cmp(&a.score));

    results
}

#[derive(Debug, Clone)]
pub struct HybridResult {
    pub chunk_id: i64,
    pub score: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_fts(chunk_ids: &[i64]) -> Vec<FtsResult> {
        chunk_ids
            .iter()
            .enumerate()
            .map(|(i, &id)| FtsResult {
                chunk_id: id,
                file_path: format!("file_{}.txt", i),
                file_name: format!("file_{}.txt", i),
                chunk_index: 0,
                content: String::new(),
                score: 1.0 - (i as f64 * 0.1),
                start_offset: 0,
                end_offset: 100,
                page_number: None,
                page_end: None,
                location_hint: None,
                snippet: String::new(),
                modified_at: None,
            })
            .collect()
    }

    fn make_vec(chunk_ids: &[i64]) -> Vec<VectorResult> {
        chunk_ids
            .iter()
            .map(|&id| VectorResult {
                chunk_id: id,
                score: 0.9,
            })
            .collect()
    }

    #[test]
    fn test_rrf_basic_merge() {
        let fts = make_fts(&[1, 2, 3]);
        let vec = make_vec(&[2, 4, 1]);

        let results = merge_results(&fts, &vec, 60.0);

        // chunk 1, 2는 양쪽 다 등장 → 높은 점수
        // chunk 3, 4는 한쪽만 → 낮은 점수
        let ids: Vec<i64> = results.iter().map(|r| r.chunk_id).collect();
        assert!(ids.contains(&1));
        assert!(ids.contains(&2));
        assert!(ids.contains(&3));
        assert!(ids.contains(&4));

        // 양쪽 모두 등장한 1, 2가 상위 2개
        let top2: Vec<i64> = results[..2].iter().map(|r| r.chunk_id).collect();
        assert!(top2.contains(&1) || top2.contains(&2));
    }

    #[test]
    fn test_rrf_score_formula() {
        let fts = make_fts(&[10]);
        let vec = make_vec(&[10]);

        let results = merge_results(&fts, &vec, 60.0);

        // rank 0 in both: 1/(60+0+1) + 1/(60+0+1) = 2/61
        let expected = 2.0 / 61.0;
        assert!((results[0].score - expected).abs() < 1e-6);
    }

    #[test]
    fn test_rrf_descending_order() {
        let fts = make_fts(&[1, 2, 3, 4, 5]);
        let vec = make_vec(&[5, 4, 3, 2, 1]);

        let results = merge_results(&fts, &vec, 60.0);

        // 점수 내림차순 정렬 확인
        for w in results.windows(2) {
            assert!(w[0].score >= w[1].score, "결과가 내림차순이 아님");
        }
    }

    #[test]
    fn test_rrf_empty_fts() {
        let fts: Vec<FtsResult> = vec![];
        let vec = make_vec(&[1, 2, 3]);

        let results = merge_results(&fts, &vec, 60.0);

        assert_eq!(results.len(), 3);
        // 벡터만 있을 때 점수: 1/(60+rank+1)
        let expected_first = 1.0 / 61.0;
        assert!((results[0].score - expected_first).abs() < 1e-6);
    }

    #[test]
    fn test_rrf_empty_vector() {
        let fts = make_fts(&[1, 2, 3]);
        let vec: Vec<VectorResult> = vec![];

        let results = merge_results(&fts, &vec, 60.0);

        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_rrf_both_empty() {
        let fts: Vec<FtsResult> = vec![];
        let vec: Vec<VectorResult> = vec![];

        let results = merge_results(&fts, &vec, 60.0);

        assert!(results.is_empty());
    }

    #[test]
    fn test_rrf_single_result() {
        let fts = make_fts(&[42]);
        let vec: Vec<VectorResult> = vec![];

        let results = merge_results(&fts, &vec, 60.0);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].chunk_id, 42);
    }

    #[test]
    fn test_rrf_duplicate_chunk_in_same_source() {
        // FTS에서 같은 chunk_id가 여러 번 등장하면 점수 누적
        let fts = make_fts(&[1, 1]);
        let vec: Vec<VectorResult> = vec![];

        let results = merge_results(&fts, &vec, 60.0);

        // chunk 1: rank0 + rank1 = 1/61 + 1/62
        assert_eq!(results.len(), 1);
        let expected = 1.0 / 61.0 + 1.0 / 62.0;
        assert!((results[0].score - expected).abs() < 1e-6);
    }

    #[test]
    fn test_rrf_k_parameter_effect() {
        let fts = make_fts(&[1, 2]);
        let vec: Vec<VectorResult> = vec![];

        // k=1: 순위 차이가 점수에 큰 영향
        let results_k1 = merge_results(&fts, &vec, 1.0);
        let gap_k1 = results_k1[0].score - results_k1[1].score;

        // k=100: 순위 차이가 점수에 작은 영향
        let results_k100 = merge_results(&fts, &vec, 100.0);
        let gap_k100 = results_k100[0].score - results_k100[1].score;

        // k가 클수록 순위 차이의 점수 영향이 줄어듦
        assert!(gap_k1 > gap_k100);
    }
}
