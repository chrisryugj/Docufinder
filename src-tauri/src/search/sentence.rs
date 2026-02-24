//! 문장 분리 및 유사도 계산 유틸리티
//!
//! 시맨틱 검색 결과에 "가장 유사한 문장"을 찾기 위한 모듈

/// 최소 문장 길이 (너무 짧은 문장 필터링)
const MIN_SENTENCE_LEN: usize = 10;

/// 청크당 최대 문장 수 (성능 제한)
pub const MAX_SENTENCES_PER_CHUNK: usize = 5;

/// 문장 종결 문자
const SENTENCE_DELIMITERS: &[char] = &['.', '!', '?', '。', '！', '？'];

/// 문장 분리 결과
#[derive(Debug, Clone)]
pub struct Sentence {
    /// 문장 텍스트
    pub text: String,
    /// 원본 텍스트 내 시작 위치 (바이트)
    pub start: usize,
    /// 원본 텍스트 내 끝 위치 (바이트)
    pub end: usize,
}

/// 텍스트를 문장으로 분리
///
/// # Arguments
/// * `text` - 분리할 텍스트
///
/// # Returns
/// 문장 목록 (최대 MAX_SENTENCES_PER_CHUNK개)
pub fn split_sentences(text: &str) -> Vec<Sentence> {
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

        // 문장 종결 문자 또는 줄바꿈 확인
        let is_delimiter = SENTENCE_DELIMITERS.contains(&c);
        let is_newline = c == '\n';

        if is_delimiter || is_newline {
            // 줄바꿈은 무조건 문장 종결, 구두점은 다음이 공백이거나 끝이면 종결
            let is_sentence_end =
                is_newline || char_idx + 1 >= chars.len() || chars[char_idx + 1].is_whitespace();

            if is_sentence_end {
                let sentence_end = byte_pos + char_len;
                let sentence_text = &trimmed[current_start..sentence_end];
                let sentence_trimmed = sentence_text.trim();

                if sentence_trimmed.len() >= MIN_SENTENCE_LEN {
                    // 원본 텍스트 내 실제 위치 계산
                    let offset = text.find(trimmed).unwrap_or(0);
                    sentences.push(Sentence {
                        text: sentence_trimmed.to_string(),
                        start: offset + current_start,
                        end: offset + sentence_end,
                    });

                    if sentences.len() >= MAX_SENTENCES_PER_CHUNK {
                        return sentences;
                    }
                }

                // 다음 문장 시작점 설정 (공백 건너뛰기)
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

    // 마지막 문장 처리
    if current_start < trimmed.len() {
        let sentence_text = &trimmed[current_start..];
        let sentence_trimmed = sentence_text.trim();

        if sentence_trimmed.len() >= MIN_SENTENCE_LEN && sentences.len() < MAX_SENTENCES_PER_CHUNK {
            let offset = text.find(trimmed).unwrap_or(0);
            sentences.push(Sentence {
                text: sentence_trimmed.to_string(),
                start: offset + current_start,
                end: offset + trimmed.len(),
            });
        }
    }

    // 문장이 하나도 없으면 전체 텍스트를 하나의 문장으로 취급
    if sentences.is_empty() && trimmed.len() >= MIN_SENTENCE_LEN {
        let offset = text.find(trimmed).unwrap_or(0);
        sentences.push(Sentence {
            text: trimmed.to_string(),
            start: offset,
            end: offset + trimmed.len(),
        });
    }

    sentences
}

/// 코사인 유사도 계산
///
/// L2 정규화된 벡터의 경우 내적이 코사인 유사도와 동일
///
/// # Arguments
/// * `a` - 첫 번째 벡터 (L2 정규화됨)
/// * `b` - 두 번째 벡터 (L2 정규화됨)
///
/// # Returns
/// 코사인 유사도 (-1.0 ~ 1.0, 높을수록 유사)
#[inline]
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len(), "Vector dimensions must match");
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_korean_sentences() {
        let text = "안녕하세요. 반갑습니다. 오늘 날씨가 좋네요.";
        let sentences = split_sentences(text);
        assert_eq!(sentences.len(), 3);
    }

    #[test]
    fn test_split_english_sentences() {
        let text = "Hello world test. How are you today? I am fine thanks!";
        let sentences = split_sentences(text);
        assert_eq!(sentences.len(), 3);
    }

    #[test]
    fn test_split_newline_sentences() {
        let text = "첫 번째 문장입니다\n두 번째 문장입니다\n세 번째 문장입니다";
        let sentences = split_sentences(text);
        assert_eq!(sentences.len(), 3);
    }

    #[test]
    fn test_split_short_sentences_filtered() {
        let text = "A. B. 이것은 충분히 긴 문장입니다.";
        let sentences = split_sentences(text);
        // A와 B는 MIN_SENTENCE_LEN 미만이라 필터링됨
        assert_eq!(sentences.len(), 1);
        assert!(sentences[0].text.contains("충분히 긴"));
    }

    #[test]
    fn test_split_max_sentences() {
        let text = "문장1입니다. 문장2입니다. 문장3입니다. 문장4입니다. 문장5입니다. 문장6입니다.";
        let sentences = split_sentences(text);
        assert_eq!(sentences.len(), MAX_SENTENCES_PER_CHUNK);
    }

    #[test]
    fn test_split_no_delimiter() {
        let text = "마침표 없이 긴 텍스트가 하나의 문장으로 처리됩니다";
        let sentences = split_sentences(text);
        assert_eq!(sentences.len(), 1);
        assert_eq!(sentences[0].text, text);
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        assert!(cosine_similarity(&a, &b).abs() < 0.001);
    }
}
