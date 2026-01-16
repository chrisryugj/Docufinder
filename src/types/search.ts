/** 단일 검색 결과 */
export interface SearchResult {
  file_path: string;
  file_name: string;
  chunk_index: number;
  content_preview: string;
  full_content: string;
  score: number;
  highlight_ranges: [number, number][];
  page_number: number | null;
  start_offset: number;
  /** 위치 힌트 (XLSX: "Sheet1!행1-50", PDF: "페이지 3", HWPX: "섹션 2" 등) */
  location_hint: string | null;
}

/** 검색 응답 */
export interface SearchResponse {
  results: SearchResult[];
  total_count: number;
  search_time_ms: number;
}

/** 검색 모드 */
export type SearchMode = "keyword" | "semantic" | "hybrid";

/** 검색 모드 정보 */
export interface SearchModeInfo {
  value: SearchMode;
  label: string;
  desc: string;
}

/** 검색 모드 목록 */
export const SEARCH_MODES: SearchModeInfo[] = [
  { value: "keyword", label: "키워드", desc: "FTS5 전문검색" },
  { value: "hybrid", label: "하이브리드", desc: "키워드 + AI 통합" },
  { value: "semantic", label: "시맨틱", desc: "AI 의미 검색" },
];

// =====================
// 필터/정렬 관련 타입
// =====================

/** 정렬 옵션 */
export type SortOption = "relevance" | "date_desc" | "date_asc" | "name" | "size";

/** 파일 타입 필터 */
export type FileTypeFilter = "all" | "hwpx" | "docx" | "xlsx" | "pdf" | "txt";

/** 날짜 범위 필터 */
export type DateRangeFilter = "all" | "today" | "week" | "month";

/** 검색 필터 상태 */
export interface SearchFilters {
  sortBy: SortOption;
  fileType: FileTypeFilter;
  dateRange: DateRangeFilter;
}

/** 기본 필터 값 */
export const DEFAULT_FILTERS: SearchFilters = {
  sortBy: "relevance",
  fileType: "all",
  dateRange: "all",
};

/** 정렬 옵션 목록 */
export const SORT_OPTIONS: { value: SortOption; label: string }[] = [
  { value: "relevance", label: "관련도순" },
  { value: "date_desc", label: "최신순" },
  { value: "date_asc", label: "오래된순" },
  { value: "name", label: "이름순" },
];

/** 파일 타입 필터 목록 */
export const FILE_TYPE_OPTIONS: { value: FileTypeFilter; label: string }[] = [
  { value: "all", label: "전체" },
  { value: "hwpx", label: "한글" },
  { value: "docx", label: "워드" },
  { value: "xlsx", label: "엑셀" },
  { value: "pdf", label: "PDF" },
  { value: "txt", label: "텍스트" },
];

/** 날짜 범위 필터 목록 */
export const DATE_RANGE_OPTIONS: { value: DateRangeFilter; label: string }[] = [
  { value: "all", label: "전체" },
  { value: "today", label: "오늘" },
  { value: "week", label: "7일" },
  { value: "month", label: "30일" },
];
