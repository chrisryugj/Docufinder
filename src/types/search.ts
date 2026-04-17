/** 단일 검색 결과 */
export interface SearchResult {
  file_path: string;
  file_name: string;
  chunk_index: number;
  content_preview: string;
  score: number;
  /** 정규화된 신뢰도 (0-100) */
  confidence: number;
  /** 검색 매칭 타입 */
  match_type: SearchResultMatchType;
  highlight_ranges: [number, number][];
  page_number: number | null;
  start_offset: number;
  /** 위치 힌트 (XLSX: "Sheet1!행1-50", PDF: "페이지 3", HWPX: "섹션 2" 등) */
  location_hint: string | null;
  /** FTS5 snippet (하이라이트 마커 포함) */
  snippet?: string;
  /** 파일 수정 시간 (Unix timestamp, 초) */
  modified_at: number | null;
  /** 같은 경로에 원본 HWP 파일이 존재하는 HWPX */
  has_hwp_pair?: boolean;
  /** 해당 파일의 전체 청크 개수 (히트맵 절대 스케일용, 0이면 미제공) */
  total_chunks?: number;
}

/** 검색 매칭 타입 */
export type SearchResultMatchType = "keyword" | "semantic" | "hybrid" | "filename";

/** 그룹화된 검색 결과 (파일별) */
export interface GroupedSearchResult {
  file_path: string;
  file_name: string;
  chunks: SearchResult[];
  /** 가장 높은 신뢰도 */
  top_confidence: number;
  /** 총 매칭 수 */
  total_matches: number;
}

/** 결과 뷰 모드 */
export type ViewMode = "flat" | "grouped";

/** 검색 응답 */
export interface SearchResponse {
  results: SearchResult[];
  total_count: number;
  search_time_ms: number;
}

/** 검색 모드 (hybrid/semantic은 내부 RAG 전용, UI 미노출) */
export type SearchMode = "keyword" | "semantic" | "hybrid" | "filename";

/** 키워드 매칭 모드 (AND / OR / EXACT) */
export type KeywordMatchMode = "and" | "or" | "exact";

/** 키워드 매칭 모드 정보 */
export interface KeywordMatchModeInfo {
  value: KeywordMatchMode;
  label: string;
  desc: string;
}

/** 키워드 매칭 모드 목록 */
export const KEYWORD_MATCH_MODES: KeywordMatchModeInfo[] = [
  { value: "and", label: "모두 포함", desc: "모든 키워드가 포함된 문서" },
  { value: "or", label: "하나 이상", desc: "키워드 중 하나라도 포함된 문서" },
  { value: "exact", label: "정확히 일치", desc: "입력한 구문이 그대로 포함된 문서" },
];

/** 최근 검색 기록 */
export interface RecentSearch {
  query: string;
  timestamp: number;  // Unix timestamp (ms)
}

/** 검색 모드 정보 */
export interface SearchModeInfo {
  value: SearchMode;
  label: string;
  desc: string;
}

/** 검색 모드 목록 (UI에 노출되는 것만) */
export const SEARCH_MODES: SearchModeInfo[] = [
  { value: "keyword", label: "키워드", desc: "FTS5 전문검색" },
  { value: "filename", label: "파일명", desc: "파일명 검색" },
];

// =====================
// 필터/정렬 관련 타입
// =====================

/** 정렬 옵션 ("size"는 향후 파일 크기 정렬 기능용으로 예약) */
export type SortOption = "relevance" | "confidence" | "date_desc" | "date_asc" | "name" | "size";

/** 파일 타입 필터 (개별 확장자) */
export type FileTypeFilter = "hwpx" | "docx" | "pptx" | "xlsx" | "pdf" | "txt";

/** 날짜 범위 필터 */
export type DateRangeFilter = "all" | "today" | "week" | "month" | "quarter" | "half" | "year" | `custom:${string}`;

/** 검색 필터 상태 */
export interface SearchFilters {
  sortBy: SortOption;
  /** 선택된 확장자들 (빈 배열 = 전체) */
  fileTypes: FileTypeFilter[];
  dateRange: DateRangeFilter;
  keywordOnly: boolean;
  /** 파일명 검색 결과 제외 */
  excludeFilename: boolean;
  /** 검색 범위 (null = 전체, string = 폴더 경로 prefix) */
  searchScope: string | null;
}

/** 기본 필터 값 */
export const DEFAULT_FILTERS: SearchFilters = {
  sortBy: "relevance",
  fileTypes: [],
  dateRange: "all",
  keywordOnly: false,
  excludeFilename: false,
  searchScope: null,
};

/** 정렬 옵션 목록 */
export const SORT_OPTIONS: { value: SortOption; label: string }[] = [
  { value: "relevance", label: "관련도순" },
  { value: "confidence", label: "신뢰도순" },
  { value: "date_desc", label: "최신순" },
  { value: "date_asc", label: "오래된순" },
  { value: "name", label: "이름순" },
];

/** 파일 타입 필터 목록 */
export const FILE_TYPE_OPTIONS: { value: FileTypeFilter; label: string }[] = [
  { value: "hwpx", label: "HWPX" },
  { value: "docx", label: "DOCX" },
  { value: "pptx", label: "PPTX" },
  { value: "xlsx", label: "XLSX" },
  { value: "pdf", label: "PDF" },
  { value: "txt", label: "TXT" },
];

/** 날짜 범위 필터 목록 */
export const DATE_RANGE_OPTIONS: { value: DateRangeFilter; label: string }[] = [
  { value: "all", label: "기간 없음" },
  { value: "today", label: "오늘" },
  { value: "week", label: "7일" },
  { value: "month", label: "30일" },
  { value: "quarter", label: "90일" },
  { value: "half", label: "6개월" },
  { value: "year", label: "1년" },
];

// =====================
// 검색 패러다임 (v2.5)
// =====================

/** 검색 패러다임: 즉시(실시간) vs 자연어(Enter 실행) vs 질문(AI RAG) */
export type SearchParadigm = "instant" | "natural" | "question";

/** NL 파서 결과 (자연어 검색 모드) */
export interface ParsedQueryInfo {
  keywords: string;
  exclude_keywords: string[];
  date_filter: { type: string; value?: number } | null;
  file_type: string | null;
  filename_filter: string | null;
  original_query: string;
  parse_log: string[];
}

/** 스마트(자연어) 검색 응답 */
export interface SmartSearchResponse {
  results: SearchResult[];
  total_count: number;
  search_time_ms: number;
  parsed_query: ParsedQueryInfo;
}

// =====================
// AI RAG (v2.6)
// =====================

/** AI 분석 응답 */
export interface AiAnalysis {
  answer: string;
  source_files: string[];
  processing_time_ms: number;
  model: string;
  tokens_used: TokenUsage | null;
}

export interface TokenUsage {
  prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
}

// =====================
// 문서 요약 (v2.4)
// =====================

/** 요약 문장 */
export interface SummarySentence {
  text: string;
  score: number;
  original_index: number;
  page_number: number | null;
  location_hint: string | null;
}

/** 요약 응답 */
export interface SummaryResponse {
  sentences: SummarySentence[];
  total_sentences: number;
  generation_time_ms: number;
}

// =====================
// 통계 대시보드 (v2.3)
// =====================

export interface StatEntry {
  label: string;
  count: number;
}

export interface FileEntry {
  path: string;
  name: string;
  value: number;
}

export interface DocumentStatistics {
  total_files: number;
  indexed_files: number;
  total_size: number;
  file_types: StatEntry[];
  years: StatEntry[];
  folders: StatEntry[];
  recent_files: FileEntry[];
  largest_files: FileEntry[];
}
