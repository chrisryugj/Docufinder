//! Search DTOs - 검색 관련 데이터 전송 객체

use serde::{Deserialize, Serialize};

/// 검색 매칭 타입
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MatchType {
    Keyword,
    Semantic,
    Hybrid,
    Filename,
}

/// 검색 쿼리 DTO
#[derive(Debug, Clone)]
pub struct SearchQuery {
    /// 검색어
    pub query: String,
    /// 검색 모드 (keyword, semantic, hybrid, filename)
    pub mode: SearchMode,
    /// 최대 결과 수
    pub max_results: usize,
}

/// 검색 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    Keyword,
    Semantic,
    Hybrid,
    Filename,
}

impl Default for SearchQuery {
    fn default() -> Self {
        Self {
            query: String::new(),
            mode: SearchMode::Hybrid,
            max_results: 50,
        }
    }
}

/// 개별 검색 결과 DTO
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SearchResult {
    /// 파일 전체 경로
    pub file_path: String,
    /// 파일명
    pub file_name: String,
    /// 청크 인덱스
    pub chunk_index: i64,
    /// 미리보기 텍스트 (200자)
    pub content_preview: String,
    /// 전체 청크 내용
    pub full_content: String,
    /// 원시 스코어
    pub score: f64,
    /// 정규화된 신뢰도 (0-100)
    pub confidence: u8,
    /// 검색 매칭 타입
    pub match_type: MatchType,
    /// 하이라이트 범위 [(시작, 끝), ...]
    pub highlight_ranges: Vec<(usize, usize)>,
    /// 페이지 번호 (PDF)
    pub page_number: Option<i64>,
    /// 청크 시작 오프셋
    pub start_offset: i64,
    /// 위치 힌트 (XLSX: "Sheet1!행1-50", PDF: "페이지 3" 등)
    pub location_hint: Option<String>,
    /// FTS5 snippet (하이라이트 마커 포함)
    pub snippet: Option<String>,
    /// 파일 수정 시간 (Unix timestamp, 초)
    pub modified_at: Option<i64>,
    /// 같은 경로에 원본 HWP 파일이 존재하는 HWPX인 경우 true
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub has_hwp_pair: bool,
}

/// 검색 응답 DTO
#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResponse {
    /// 검색 결과 목록
    pub results: Vec<SearchResult>,
    /// 총 결과 수
    pub total_count: usize,
    /// 검색 소요 시간 (ms)
    pub search_time_ms: u64,
    /// 검색 모드
    pub search_mode: String,
}

impl SearchResponse {
    /// 빈 응답 생성
    pub fn empty(mode: &str) -> Self {
        Self {
            results: vec![],
            total_count: 0,
            search_time_ms: 0,
            search_mode: mode.to_string(),
        }
    }
}
