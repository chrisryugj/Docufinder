//! Lindera 기반 한국어 형태소 분석기
//!
//! ko-dic 사전을 사용하여 한국어 텍스트를 형태소 단위로 분리합니다.

use super::TextTokenizer;
use lindera::{
    dictionary::load_dictionary,
    mode::Mode,
    segmenter::Segmenter,
    tokenizer::Tokenizer,
};
use std::sync::Mutex;

/// Lindera 한국어 토크나이저
///
/// ko-dic 사전 기반 형태소 분석을 수행합니다.
pub struct LinderaKoTokenizer {
    /// Lindera tokenizer (내부적으로 mutable 필요)
    tokenizer: Mutex<Tokenizer>,
}

impl LinderaKoTokenizer {
    /// 새 토크나이저 생성
    pub fn new() -> Result<Self, LinderaError> {
        // ko-dic 사전 로드 (embedded 방식)
        let dictionary = load_dictionary("embedded://ko-dic")
            .map_err(|e| LinderaError::DictionaryLoad(e.to_string()))?;

        // Normal 모드로 segmenter 생성
        let segmenter = Segmenter::new(Mode::Normal, dictionary, None);

        // Tokenizer 생성
        let tokenizer = Tokenizer::new(segmenter);

        Ok(Self {
            tokenizer: Mutex::new(tokenizer),
        })
    }

    /// 텍스트를 형태소로 분리 (원본 텍스트 + 형태소 결합)
    ///
    /// FTS 검색 시 원본 텍스트와 형태소 모두 검색 가능하도록 합니다.
    fn tokenize_with_original(&self, text: &str) -> Vec<String> {
        let mut result = Vec::new();

        // 원본 단어들 추가 (공백 기준 분리)
        for word in text.split_whitespace() {
            let clean = Self::clean_token(word);
            if !clean.is_empty() {
                result.push(clean);
            }
        }

        // 형태소 분석 결과 추가
        if let Ok(tokenizer) = self.tokenizer.lock() {
            if let Ok(tokens) = tokenizer.tokenize(text) {
                for token in tokens {
                    let surface = token.surface.as_ref().to_string();
                    // 1글자 이상이고, 아직 없는 토큰만 추가
                    if surface.len() >= 2 && !result.contains(&surface) {
                        result.push(surface);
                    }
                }
            }
        }

        result
    }

    /// 토큰 정제 (특수문자 제거)
    fn clean_token(token: &str) -> String {
        token
            .chars()
            .filter(|c| c.is_alphanumeric() || *c >= '\u{AC00}' && *c <= '\u{D7AF}') // 한글 유니코드 범위
            .collect()
    }

    /// 한글 포함 여부 확인
    fn contains_korean(text: &str) -> bool {
        text.chars().any(|c| c >= '\u{AC00}' && c <= '\u{D7AF}')
    }

    /// 숫자+한글 조합 토큰 추출 (예: "1종", "2차", "3분기")
    /// 형태소 분석기가 "1종" → "1" + "종"으로 쪼개는 것 방지
    fn extract_number_korean_tokens(text: &str) -> Vec<String> {
        let mut tokens = Vec::new();
        let mut current = String::new();
        let mut has_number = false;
        let mut has_korean = false;

        for c in text.chars() {
            if c.is_ascii_digit() {
                if has_korean && !current.is_empty() {
                    // 한글 뒤에 숫자가 오면 (예: "종1") - 저장하고 새로 시작
                    if has_number {
                        tokens.push(current.clone());
                    }
                    current.clear();
                    has_korean = false;
                }
                current.push(c);
                has_number = true;
            } else if c >= '\u{AC00}' && c <= '\u{D7AF}' {
                // 한글
                current.push(c);
                has_korean = true;
            } else {
                // 공백/특수문자 - 현재 토큰 저장
                if has_number && has_korean && current.len() >= 2 {
                    tokens.push(current.clone());
                }
                current.clear();
                has_number = false;
                has_korean = false;
            }
        }

        // 마지막 토큰 처리
        if has_number && has_korean && current.len() >= 2 {
            tokens.push(current);
        }

        tokens
    }
}

impl TextTokenizer for LinderaKoTokenizer {
    fn tokenize(&self, text: &str) -> Vec<String> {
        // 한글이 포함된 경우에만 형태소 분석
        if Self::contains_korean(text) {
            self.tokenize_with_original(text)
        } else {
            // 영어/숫자만 있는 경우 공백 기준 분리
            text.split_whitespace()
                .map(|s| Self::clean_token(s))
                .filter(|s| !s.is_empty())
                .collect()
        }
    }

    fn tokenize_query(&self, query: &str) -> String {
        // 1. 숫자+한글 조합 보존 (예: "1종", "2차", "3분기")
        let preserved_tokens = Self::extract_number_korean_tokens(query);

        // 2. 형태소 분석
        let mut tokens = self.tokenize(query);

        // 3. 숫자+한글 조합 토큰 추가 (중복 제거)
        for token in preserved_tokens {
            if !tokens.contains(&token) {
                tokens.push(token);
            }
        }

        if tokens.is_empty() {
            return String::new();
        }

        // 4. OR 기반 검색 쿼리 생성
        // "분기 1종 홍보" → ("분기"* OR "1종"* OR "홍보"*)
        let term_queries: Vec<String> = tokens
            .iter()
            .map(|t| {
                let escaped = t.replace('"', "\"\"");
                format!("\"{}\"*", escaped)
            })
            .collect();

        // 단일 토큰이면 OR 불필요
        if term_queries.len() == 1 {
            return term_queries[0].clone();
        }

        // 여러 토큰이면 OR로 연결
        format!("({})", term_queries.join(" OR "))
    }
}

impl Default for LinderaKoTokenizer {
    fn default() -> Self {
        Self::new().expect("Failed to create LinderaKoTokenizer")
    }
}

/// Lindera 관련 에러
#[derive(Debug, thiserror::Error)]
pub enum LinderaError {
    #[error("사전 로드 실패: {0}")]
    DictionaryLoad(String),
    #[allow(dead_code)]
    #[error("토큰화 실패: {0}")]
    Tokenize(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_korean_tokenize() {
        let tokenizer = LinderaKoTokenizer::new().unwrap();
        let tokens = tokenizer.tokenize("사용했습니다");

        // "사용"이 토큰에 포함되어야 함
        println!("Tokens: {:?}", tokens);
        assert!(
            tokens.iter().any(|t| t.contains("사용")),
            "Expected '사용' in tokens"
        );
    }

    #[test]
    fn test_tokenize_query() {
        let tokenizer = LinderaKoTokenizer::new().unwrap();
        let query = tokenizer.tokenize_query("검색 테스트");

        println!("Query: {}", query);
        assert!(query.contains("\"*"));
    }

    #[test]
    fn test_english_fallback() {
        let tokenizer = LinderaKoTokenizer::new().unwrap();
        let tokens = tokenizer.tokenize("hello world");

        assert_eq!(tokens, vec!["hello", "world"]);
    }

    #[test]
    fn test_mixed_text() {
        let tokenizer = LinderaKoTokenizer::new().unwrap();
        let tokens = tokenizer.tokenize("한글과 English 혼합");

        println!("Mixed tokens: {:?}", tokens);
        assert!(tokens.contains(&"English".to_string()));
    }

    #[test]
    fn test_number_korean_extraction() {
        // 숫자+한글 조합 추출 테스트
        let tokens = LinderaKoTokenizer::extract_number_korean_tokens("1종 2차 3분기");
        println!("Number+Korean tokens: {:?}", tokens);
        assert!(tokens.contains(&"1종".to_string()));
        assert!(tokens.contains(&"2차".to_string()));
        assert!(tokens.contains(&"3분기".to_string()));
    }

    #[test]
    fn test_or_query_generation() {
        let tokenizer = LinderaKoTokenizer::new().unwrap();
        let query = tokenizer.tokenize_query("분기 1종 홍보");

        println!("OR Query: {}", query);
        // OR 기반 쿼리 생성 확인
        assert!(query.contains(" OR "));
        assert!(query.starts_with('('));
        assert!(query.ends_with(')'));
        // 1종이 보존되어야 함
        assert!(query.contains("1종"));
    }

    #[test]
    fn test_single_token_no_or() {
        let tokenizer = LinderaKoTokenizer::new().unwrap();
        let query = tokenizer.tokenize_query("검색");

        println!("Single token query: {}", query);
        // 단일 토큰이면 OR 없어야 함
        assert!(!query.contains(" OR "));
    }
}
