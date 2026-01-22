//! 텍스트 토크나이저 모듈
//!
//! 한국어 형태소 분석을 통해 FTS5 검색 품질을 개선합니다.

pub mod lindera_ko;

pub use lindera_ko::LinderaKoTokenizer;

/// 텍스트 토크나이저 trait
///
/// FTS5 인덱싱 및 검색 쿼리 토큰화에 사용됩니다.
pub trait TextTokenizer: Send + Sync {
    /// 텍스트를 토큰 목록으로 분해
    fn tokenize(&self, text: &str) -> Vec<String>;

    /// FTS5 저장용 토큰화된 문자열 반환 (공백 구분)
    fn tokenize_for_fts(&self, text: &str) -> String {
        self.tokenize(text).join(" ")
    }

    /// 검색 쿼리 토큰화 (FTS5 prefix 매칭용)
    ///
    /// 각 토큰에 쌍따옴표와 와일드카드를 적용합니다.
    /// 예: "사용했습니다" → "\"사용\"* \"하\"* \"았\"* \"습니다\"*"
    fn tokenize_query(&self, query: &str) -> String;
}
