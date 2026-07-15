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
    /// 전체 청크 내용 — Rust 내부 전용 (RAG 컨텍스트: commands/ai.rs, semantic enrichment).
    /// 프론트는 snippet/content_preview만 사용하므로 IPC 직렬화에서 제외 (페이로드 ~60-70% 절감)
    #[serde(skip_serializing, default)]
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
    /// 파일 크기 (바이트). 파일명 검색 결과에만 채워짐 (내용 검색은 None)
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub size: Option<i64>,
    /// 같은 경로에 원본 HWP 파일이 존재하는 HWPX인 경우 true
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub has_hwp_pair: bool,
    /// 해당 파일의 전체 청크 개수 (히트맵 절대 스케일용)
    #[serde(default)]
    pub total_chunks: i64,
    /// Document Lineage Graph: 같은 논리 문서의 버전 그룹 ID (SHA-256 16자)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lineage_id: Option<String>,
    /// Lineage 내 역할 — "canonical" | "version"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lineage_role: Option<String>,
    /// 파일명에서 추출한 버전 라벨 — "최최종", "v3" 등
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_label: Option<String>,
    /// 같은 lineage에 속한 전체 파일 수 (자기 자신 포함)
    #[serde(default)]
    pub version_count: i64,
    /// 복사 시 글자가 깨지는 문서(PDF CID/ToUnicode 누락, HWP PUA 커스텀폰트)로 감지됨 — 경고 배지용
    #[serde(default)]
    pub garbled: bool,
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

/// 검증가능한 인용 — `[출처N]`의 N(doc_num)을 실제 문서 위치/앵커로 매핑.
///
/// `build_rag_context`가 `[문서N]` 번호를 부여하는 시점에 그 문서 청크들의
/// 위치 메타(page/location_hint)와 본문 앵커 텍스트를 함께 캡처한다.
/// 프론트는 이 앵커를 미리보기 마크다운에서 검색해 해당 위치로 점프+하이라이트한다.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SourceRef {
    /// `[출처: N]` 의 N (1-based). `source_files[N-1]` 과 동일 문서.
    pub doc_num: usize,
    /// 파일 전체 경로
    pub file_path: String,
    /// 파일명
    pub file_name: String,
    /// 첫 위치정보 보유 청크의 페이지 번호 (없을 수 있음 — kordoc 경로)
    pub page_number: Option<i64>,
    /// 위치 힌트 (XLSX "Sheet1!행1-50", PDF "페이지 3" 등)
    pub location_hint: Option<String>,
    /// 미리보기 점프용 앵커 텍스트 목록 (각 청크 본문 앞부분, 검색 순서)
    pub anchors: Vec<String>,
}

/// AI RAG 응답 DTO
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AiAnalysis {
    /// AI 생성 답변 (마크다운)
    pub answer: String,
    /// 참조한 문서 경로 목록
    pub source_files: Vec<String>,
    /// 검증가능 인용 — doc_num ↔ 문서 위치/앵커 매핑 (인용 점프용)
    #[serde(default)]
    pub sources: Vec<SourceRef>,
    /// 처리 시간 (ms)
    pub processing_time_ms: u64,
    /// 사용 모델
    pub model: String,
    /// 토큰 사용량
    pub tokens_used: Option<TokenUsage>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// 스마트(자연어) 검색 응답 DTO
#[derive(Debug, Serialize)]
pub struct SmartSearchResponse {
    /// 검색 결과 목록
    pub results: Vec<SearchResult>,
    /// 총 결과 수
    pub total_count: usize,
    /// 검색 소요 시간 (ms)
    pub search_time_ms: u64,
    /// NL 파싱 결과 (프론트엔드에서 칩 UI 표시용)
    pub parsed_query: crate::search::nl_query::ParsedQuery,
}
