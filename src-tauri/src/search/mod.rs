pub mod filename;
pub mod filename_cache;
pub mod fts;
pub mod hybrid;
pub mod nl_query;
pub mod sentence;
pub mod vector;

/// 키워드 검색 모드 (AND / OR / EXACT)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeywordMode {
    /// 모든 키워드 포함 (기본)
    #[default]
    And,
    /// 하나 이상 포함
    Or,
    /// 정확한 구문 일치
    Exact,
}
