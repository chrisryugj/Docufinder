//! TextRank 추출적 요약
//!
//! TF-IDF 기반 문장 유사도 → PageRank 반복 → 상위 N 문장 추출.
//! 추가 모델 불필요, 100% 오프라인.

use std::collections::HashMap;

/// 최소 문장 길이 (TextRank용)
const MIN_SENTENCE_LEN: usize = 15;

/// 문장 종결 문자
const SENTENCE_DELIMITERS: &[char] = &['.', '!', '?', '。', '！', '？'];

/// PageRank damping factor
const DAMPING: f32 = 0.85;

/// 수렴 조건
const EPSILON: f32 = 1e-6;

/// 최대 반복 횟수
const MAX_ITERATIONS: usize = 100;

/// TextRank 요약 최대 입력 문장 수 (성능 제한)
const MAX_INPUT_SENTENCES: usize = 200;

/// 랭크가 매겨진 문장
#[derive(Debug, Clone)]
pub struct RankedSentence {
    /// 문장 텍스트
    pub text: String,
    /// TextRank 스코어
    pub score: f32,
    /// 원본 텍스트 내 문장 순서 (0-based)
    pub original_index: usize,
}

/// 전체 문장 수 카운트 (요약 없이 문장 분리만 수행)
pub fn count_sentences(text: &str) -> usize {
    split_sentences_all(text).len()
}

/// 텍스트를 문장으로 분리 (TextRank용, 개수 제한 없음)
fn split_sentences_all(text: &str) -> Vec<(String, usize)> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return vec![];
    }

    let mut sentences = Vec::new();
    let mut current_start = 0;
    let chars: Vec<char> = trimmed.chars().collect();
    let mut byte_pos = 0;
    let mut char_idx = 0;

    while char_idx < chars.len() {
        let c = chars[char_idx];
        let char_len = c.len_utf8();

        let is_delimiter = SENTENCE_DELIMITERS.contains(&c);
        let is_newline = c == '\n';

        if is_delimiter || is_newline {
            let is_sentence_end =
                is_newline || char_idx + 1 >= chars.len() || chars[char_idx + 1].is_whitespace();

            if is_sentence_end {
                let sentence_end = byte_pos + char_len;
                let sentence_text = &trimmed[current_start..sentence_end];
                let sentence_trimmed = sentence_text.trim();

                if sentence_trimmed.len() >= MIN_SENTENCE_LEN {
                    sentences.push((sentence_trimmed.to_string(), sentences.len()));

                    if sentences.len() >= MAX_INPUT_SENTENCES {
                        return sentences;
                    }
                }

                byte_pos += char_len;
                char_idx += 1;
                while char_idx < chars.len() && chars[char_idx].is_whitespace() {
                    byte_pos += chars[char_idx].len_utf8();
                    char_idx += 1;
                }
                current_start = byte_pos;
                continue;
            }
        }

        byte_pos += char_len;
        char_idx += 1;
    }

    // 마지막 문장
    if current_start < trimmed.len() {
        let sentence_text = &trimmed[current_start..];
        let sentence_trimmed = sentence_text.trim();
        if sentence_trimmed.len() >= MIN_SENTENCE_LEN && sentences.len() < MAX_INPUT_SENTENCES {
            sentences.push((sentence_trimmed.to_string(), sentences.len()));
        }
    }

    // 문장 0개면 전체를 하나로
    if sentences.is_empty() && trimmed.len() >= MIN_SENTENCE_LEN {
        sentences.push((trimmed.to_string(), 0));
    }

    sentences
}

/// 음절 바이그램 기반 TF-IDF 벡터 생성
///
/// 한국어는 형태소 분석 없이도 음절 바이그램으로 어느 정도 의미 단위를 잡을 수 있음.
fn build_tfidf_vectors(sentences: &[String]) -> Vec<HashMap<String, f32>> {
    if sentences.is_empty() {
        return vec![];
    }

    let n = sentences.len() as f32;

    // 1) 각 문장의 바이그램 TF 계산
    let sentence_bigrams: Vec<HashMap<String, f32>> = sentences
        .iter()
        .map(|s| {
            let mut tf: HashMap<String, f32> = HashMap::new();
            let chars: Vec<char> = s.chars().filter(|c| !c.is_whitespace()).collect();
            if chars.len() < 2 {
                // 단일 문자면 유니그램
                for c in &chars {
                    *tf.entry(c.to_string()).or_default() += 1.0;
                }
            } else {
                for pair in chars.windows(2) {
                    let bigram: String = pair.iter().collect();
                    *tf.entry(bigram).or_default() += 1.0;
                }
            }
            // TF 정규화
            let max_tf = tf.values().cloned().fold(0.0_f32, f32::max);
            if max_tf > 0.0 {
                for v in tf.values_mut() {
                    *v /= max_tf;
                }
            }
            tf
        })
        .collect();

    // 2) DF (문서 빈도) 계산
    let mut df: HashMap<String, f32> = HashMap::new();
    for bigrams in &sentence_bigrams {
        for key in bigrams.keys() {
            *df.entry(key.clone()).or_default() += 1.0;
        }
    }

    // 3) TF-IDF = TF * log(N / DF)
    sentence_bigrams
        .into_iter()
        .map(|tf_map| {
            tf_map
                .into_iter()
                .map(|(term, tf)| {
                    let idf = (n / df.get(&term).copied().unwrap_or(1.0)).ln() + 1.0;
                    (term, tf * idf)
                })
                .collect()
        })
        .collect()
}

/// 희소 벡터 간 코사인 유사도
fn sparse_cosine_similarity(a: &HashMap<String, f32>, b: &HashMap<String, f32>) -> f32 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    let dot: f32 = a
        .iter()
        .filter_map(|(k, v)| b.get(k).map(|bv| v * bv))
        .sum();

    let norm_a: f32 = a.values().map(|v| v * v).sum::<f32>().sqrt();
    let norm_b: f32 = b.values().map(|v| v * v).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

/// 유사도 매트릭스 구성 (NxN)
fn build_similarity_matrix(tfidf_vectors: &[HashMap<String, f32>]) -> Vec<Vec<f32>> {
    let n = tfidf_vectors.len();
    let mut matrix = vec![vec![0.0_f32; n]; n];

    for i in 0..n {
        for j in (i + 1)..n {
            let sim = sparse_cosine_similarity(&tfidf_vectors[i], &tfidf_vectors[j]);
            matrix[i][j] = sim;
            matrix[j][i] = sim;
        }
    }

    matrix
}

/// Power Iteration으로 PageRank 스코어 계산
fn power_iteration(matrix: &[Vec<f32>]) -> Vec<f32> {
    let n = matrix.len();
    if n == 0 {
        return vec![];
    }
    if n == 1 {
        return vec![1.0];
    }

    // 행 정규화 (전이 확률 매트릭스)
    let row_sums: Vec<f32> = matrix.iter().map(|row| row.iter().sum::<f32>()).collect();

    let mut scores = vec![1.0 / n as f32; n];
    let base = (1.0 - DAMPING) / n as f32;

    for _ in 0..MAX_ITERATIONS {
        let mut new_scores = vec![base; n];

        for i in 0..n {
            for j in 0..n {
                if i != j && row_sums[j] > 0.0 {
                    new_scores[i] += DAMPING * matrix[j][i] / row_sums[j] * scores[j];
                }
            }
        }

        // 수렴 체크
        let diff: f32 = scores
            .iter()
            .zip(new_scores.iter())
            .map(|(a, b)| (a - b).abs())
            .sum();

        scores = new_scores;

        if diff < EPSILON {
            break;
        }
    }

    scores
}

/// TextRank 추출적 요약 수행
///
/// # Arguments
/// * `text` - 요약할 전체 텍스트
/// * `num_sentences` - 추출할 문장 수
///
/// # Returns
/// 랭크 순으로 정렬된 상위 N 문장 (원문 순서대로 재정렬)
pub fn summarize(text: &str, num_sentences: usize) -> Vec<RankedSentence> {
    if text.trim().is_empty() || num_sentences == 0 {
        return vec![];
    }

    // 1. 문장 분리
    let sentence_pairs = split_sentences_all(text);
    if sentence_pairs.is_empty() {
        return vec![];
    }

    let num_to_extract = num_sentences.min(sentence_pairs.len());

    // 문장이 요청 수 이하면 전부 반환
    if sentence_pairs.len() <= num_to_extract {
        return sentence_pairs
            .into_iter()
            .map(|(text, idx)| RankedSentence {
                text,
                score: 1.0,
                original_index: idx,
            })
            .collect();
    }

    let sentences: Vec<String> = sentence_pairs.iter().map(|(s, _)| s.clone()).collect();
    let indices: Vec<usize> = sentence_pairs.iter().map(|(_, i)| *i).collect();

    // 2. TF-IDF 벡터
    let tfidf = build_tfidf_vectors(&sentences);

    // 3. 유사도 매트릭스
    let sim_matrix = build_similarity_matrix(&tfidf);

    // 4. PageRank
    let scores = power_iteration(&sim_matrix);

    // 5. 상위 N 문장 선택
    let mut scored: Vec<(usize, f32, &str)> = scores
        .iter()
        .enumerate()
        .map(|(i, &score)| (indices[i], score, sentences[i].as_str()))
        .collect();

    // 스코어 내림차순
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(num_to_extract);

    // 원문 순서로 재정렬
    scored.sort_by_key(|&(idx, _, _)| idx);

    scored
        .into_iter()
        .map(|(idx, score, text)| RankedSentence {
            text: text.to_string(),
            score,
            original_index: idx,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_text() {
        let result = summarize("", 3);
        assert!(result.is_empty());
    }

    #[test]
    fn test_whitespace_only() {
        let result = summarize("   \n  \t  ", 3);
        assert!(result.is_empty());
    }

    #[test]
    fn test_single_sentence() {
        let result = summarize("이것은 하나의 긴 문장으로만 이루어진 텍스트입니다", 3);
        assert_eq!(result.len(), 1);
        assert!(result[0].text.contains("하나의 긴 문장"));
    }

    #[test]
    fn test_fewer_sentences_than_requested() {
        let text = "첫 번째 문장이 있습니다. 두 번째 문장도 있습니다.";
        let result = summarize(text, 5);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_korean_document_summary() {
        let text = "대한민국 헌법은 국가의 기본법으로서 국민의 기본권을 보장합니다.\n\
                     국회는 입법권을 가지며 법률을 제정하고 개정하는 권한이 있습니다.\n\
                     대통령은 행정부의 수반으로서 국가를 대표하고 외교 정책을 수행합니다.\n\
                     법원은 사법권을 행사하여 분쟁을 해결하고 법률을 해석합니다.\n\
                     헌법재판소는 위헌법률심판과 탄핵심판 등의 권한을 행사합니다.\n\
                     국민은 선거권과 피선거권을 가지며 민주적 절차에 참여할 수 있습니다.\n\
                     지방자치단체는 주민의 복리에 관한 사무를 처리하는 기관입니다.";

        let result = summarize(text, 3);
        assert_eq!(result.len(), 3);

        // 원문 순서 유지 확인
        for i in 1..result.len() {
            assert!(result[i].original_index > result[i - 1].original_index);
        }

        // 스코어 양수 확인
        for s in &result {
            assert!(s.score > 0.0, "score should be positive: {}", s.score);
        }
    }

    #[test]
    fn test_english_document_summary() {
        let text = "Machine learning is a subset of artificial intelligence.\n\
                     Deep learning uses neural networks with multiple layers.\n\
                     Natural language processing enables computers to understand text.\n\
                     Computer vision allows machines to interpret visual information.\n\
                     Reinforcement learning trains agents through rewards and penalties.";

        let result = summarize(text, 2);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_repeated_sentences() {
        let text = "같은 문장이 반복됩니다 이것은 테스트입니다.\n\
                     같은 문장이 반복됩니다 이것은 테스트입니다.\n\
                     같은 문장이 반복됩니다 이것은 테스트입니다.\n\
                     완전히 다른 내용의 고유한 문장이 여기 있습니다.\n\
                     또 다른 독특한 문장으로 다양성을 추가합니다.";

        let result = summarize(text, 2);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_preserves_original_order() {
        let text = "첫 번째로 중요한 내용이 이 문장에 담겨 있습니다.\n\
                     두 번째 문장은 덜 중요할 수도 있는 내용입니다.\n\
                     세 번째 문장에는 핵심 결론이 포함되어 있습니다.\n\
                     네 번째 문장은 부가적인 설명을 제공합니다.";

        let result = summarize(text, 2);
        assert_eq!(result.len(), 2);

        // 원본 순서 유지
        assert!(result[0].original_index < result[1].original_index);
    }

    #[test]
    fn test_zero_sentences_requested() {
        let result = summarize("충분히 긴 텍스트가 있지만 요청한 문장 수는 0입니다.", 0);
        assert!(result.is_empty());
    }

    #[test]
    fn test_tfidf_vectors_basic() {
        let sentences = vec![
            "안녕하세요 반갑습니다".to_string(),
            "오늘 날씨가 좋습니다".to_string(),
        ];
        let vectors = build_tfidf_vectors(&sentences);
        assert_eq!(vectors.len(), 2);
        assert!(!vectors[0].is_empty());
        assert!(!vectors[1].is_empty());
    }

    #[test]
    fn test_sparse_cosine_identical() {
        let mut a = HashMap::new();
        a.insert("ab".to_string(), 1.0);
        a.insert("bc".to_string(), 0.5);

        let sim = sparse_cosine_similarity(&a, &a);
        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_sparse_cosine_disjoint() {
        let mut a = HashMap::new();
        a.insert("ab".to_string(), 1.0);

        let mut b = HashMap::new();
        b.insert("cd".to_string(), 1.0);

        let sim = sparse_cosine_similarity(&a, &b);
        assert!(sim.abs() < 0.001);
    }

    #[test]
    fn test_power_iteration_convergence() {
        // 3x3 유사도 매트릭스 (대칭)
        let matrix = vec![
            vec![0.0, 0.5, 0.3],
            vec![0.5, 0.0, 0.8],
            vec![0.3, 0.8, 0.0],
        ];
        let scores = power_iteration(&matrix);
        assert_eq!(scores.len(), 3);

        // 합이 대략 1
        let sum: f32 = scores.iter().sum();
        assert!((sum - 1.0).abs() < 0.1, "scores sum: {}", sum);

        // 노드 1 (가장 연결 강한)이 가장 높은 스코어
        assert!(scores[1] >= scores[0]);
        assert!(scores[1] >= scores[2]);
    }

    #[test]
    fn test_split_sentences_all_no_limit() {
        // MAX_SENTENCES_PER_CHUNK(5)보다 많은 문장
        let text = (1..=10)
            .map(|i| format!("이것은 테스트 문장 번호 {}입니다", i))
            .collect::<Vec<_>>()
            .join(". ");

        let sentences = split_sentences_all(&text);
        assert!(
            sentences.len() > 5,
            "should exceed MAX_SENTENCES_PER_CHUNK, got {}",
            sentences.len()
        );
    }
}
