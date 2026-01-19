import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import type {
  SearchResult,
  SearchResponse,
  SearchMode,
  SearchFilters,
  FileTypeFilter,
  GroupedSearchResult,
  ViewMode,
} from "../types/search";
import { DEFAULT_FILTERS } from "../types/search";
import { SEARCH_COMMANDS } from "../types/api";

interface UseSearchOptions {
  debounceMs?: number;
  minConfidence?: number;
}

interface UseSearchReturn {
  query: string;
  setQuery: (query: string) => void;
  results: SearchResult[];
  /** 파일명 검색 결과 (통합 모드에서 상단 표시용) */
  filenameResults: SearchResult[];
  filteredResults: SearchResult[];
  groupedResults: GroupedSearchResult[];
  searchTime: number | null;
  isLoading: boolean;
  error: string | null;
  clearError: () => void;
  searchMode: SearchMode;
  setSearchMode: (mode: SearchMode) => void;
  filters: SearchFilters;
  setFilters: (filters: SearchFilters) => void;
  viewMode: ViewMode;
  setViewMode: (mode: ViewMode) => void;
  /** 결과 내 검색 쿼리 */
  refineQuery: string;
  setRefineQuery: (query: string) => void;
  /** 결과 내 검색 초기화 */
  clearRefine: () => void;
  /** 결과 내 검색 활성화 여부 */
  isRefineActive: boolean;
}

/**
 * 검색 로직 훅 (디바운스 포함)
 */
export function useSearch(options: UseSearchOptions = {}): UseSearchReturn {
  const { debounceMs = 300 } = options;
  // minConfidence는 외부에서 변경될 수 있으므로 직접 참조
  const minConfidence = options.minConfidence ?? 0;

  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [filenameResults, setFilenameResults] = useState<SearchResult[]>([]);
  const [searchTime, setSearchTime] = useState<number | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [searchMode, setSearchMode] = useState<SearchMode>("hybrid");
  const [filters, setFiltersInternal] = useState<SearchFilters>(() => {
    // localStorage에서 excludeFilename 복원
    try {
      const saved = localStorage.getItem("docufinder_exclude_filename");
      if (saved !== null) {
        return { ...DEFAULT_FILTERS, excludeFilename: JSON.parse(saved) };
      }
    } catch {}
    return DEFAULT_FILTERS;
  });
  const [viewMode, setViewMode] = useState<ViewMode>("flat");
  const [refineQuery, setRefineQuery] = useState("");

  const clearError = useCallback(() => setError(null), []);
  const clearRefine = useCallback(() => setRefineQuery(""), []);

  // excludeFilename 변경 시 localStorage 저장
  const prevExcludeFilename = useRef(filters.excludeFilename);
  const setFilters = useCallback((newFilters: SearchFilters) => {
    setFiltersInternal(newFilters);
    if (newFilters.excludeFilename !== prevExcludeFilename.current) {
      prevExcludeFilename.current = newFilters.excludeFilename;
      try {
        localStorage.setItem("docufinder_exclude_filename", JSON.stringify(newFilters.excludeFilename));
      } catch {}
    }
  }, []);

  // 검색 실행 함수
  const executeSearch = useCallback(
    async (searchQuery: string, mode: SearchMode) => {
      if (!searchQuery.trim()) {
        setResults([]);
        setFilenameResults([]);
        setSearchTime(null);
        return;
      }

      setIsLoading(true);
      setError(null);

      try {
        if (mode === "filename") {
          // 파일명 모드: 파일명 검색만 실행
          const response = await invoke<SearchResponse>(SEARCH_COMMANDS[mode], {
            query: searchQuery,
          });
          setResults(response.results);
          setFilenameResults([]);
          setSearchTime(response.search_time_ms);
        } else {
          // 내용 검색 모드: 파일명 검색도 병렬 실행 (통합 모드)
          const [contentResponse, filenameResponse] = await Promise.all([
            invoke<SearchResponse>(SEARCH_COMMANDS[mode], { query: searchQuery }),
            invoke<SearchResponse>(SEARCH_COMMANDS.filename, { query: searchQuery }),
          ]);
          setResults(contentResponse.results);
          setFilenameResults(filenameResponse.results);
          setSearchTime(contentResponse.search_time_ms);
        }
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        console.error("Search failed:", err);
        setError(`검색 실패: ${message}`);
        setResults([]);
        setFilenameResults([]);
        setSearchTime(null);
      }

      setIsLoading(false);
    },
    []
  );

  // 디바운스 검색
  useEffect(() => {
    const timer = setTimeout(() => {
      executeSearch(query, searchMode);
    }, debounceMs);

    return () => clearTimeout(timer);
  }, [query, searchMode, debounceMs, executeSearch]);

  // 필터링된 결과
  const filteredResults = useMemo(() => {
    let filtered = [...results];

    if (minConfidence > 0) {
      filtered = filtered.filter((r) => r.confidence >= minConfidence);
    }

    if (filters.keywordOnly) {
      filtered = filtered.filter((r) => {
        // 대소문자 무관하게 비교 (serde 직렬화 호환성)
        const type = (r.match_type ?? "").toLowerCase();
        return type === "keyword" || type === "hybrid";
      });
    }

    // 파일 타입 필터
    if (filters.fileType !== "all") {
      const extMap: Record<FileTypeFilter, string[]> = {
        all: [],
        hwpx: ["hwpx"],
        docx: ["docx", "doc"],
        xlsx: ["xlsx", "xls"],
        pdf: ["pdf"],
        txt: ["txt", "md"],
      };
      const allowedExts = extMap[filters.fileType];
      filtered = filtered.filter((r) => {
        const ext = r.file_name.split(".").pop()?.toLowerCase() || "";
        return allowedExts.includes(ext);
      });
    }

    // 정렬
    switch (filters.sortBy) {
      case "relevance":
        // 이미 score 순으로 정렬됨
        break;
      case "confidence":
        // 신뢰도 높은 순
        filtered.sort((a, b) => b.confidence - a.confidence);
        break;
      case "date_desc":
        // 최신순 (수정일 내림차순)
        filtered.sort((a, b) => (b.modified_at ?? 0) - (a.modified_at ?? 0));
        break;
      case "date_asc":
        // 오래된순 (수정일 오름차순)
        filtered.sort((a, b) => (a.modified_at ?? 0) - (b.modified_at ?? 0));
        break;
      case "name":
        filtered.sort((a, b) => a.file_name.localeCompare(b.file_name, "ko"));
        break;
    }

    // 결과 내 검색 필터링
    if (refineQuery.trim()) {
      const refineKeywords = refineQuery.trim().toLowerCase().split(/\s+/);
      filtered = filtered.filter((r) => {
        const content = r.full_content.toLowerCase();
        // 모든 키워드가 포함되어야 함 (AND 조건)
        return refineKeywords.every((kw) => content.includes(kw));
      });
    }

    return filtered;
  }, [results, filters, minConfidence, refineQuery]);

  // 파일명 검색 결과도 결과 내 검색 필터링
  const filteredFilenameResults = useMemo(() => {
    if (!refineQuery.trim()) {
      return filenameResults;
    }
    const keywords = refineQuery.trim().toLowerCase().split(/\s+/);
    return filenameResults.filter((r) => {
      const fileName = r.file_name.toLowerCase();
      // 파일명에서 키워드 검색
      return keywords.every((kw) => fileName.includes(kw));
    });
  }, [filenameResults, refineQuery]);

  // 파일별 그룹핑 결과
  const groupedResults = useMemo(() => {
    const groups = new Map<string, GroupedSearchResult>();

    for (const result of filteredResults) {
      const existing = groups.get(result.file_path);
      if (existing) {
        existing.chunks.push(result);
        existing.top_confidence = Math.max(existing.top_confidence, result.confidence);
        existing.total_matches++;
      } else {
        groups.set(result.file_path, {
          file_path: result.file_path,
          file_name: result.file_name,
          chunks: [result],
          top_confidence: result.confidence,
          total_matches: 1,
        });
      }
    }

    // 최고 신뢰도순 정렬
    return Array.from(groups.values()).sort(
      (a, b) => b.top_confidence - a.top_confidence
    );
  }, [filteredResults]);

  return {
    query,
    setQuery,
    results,
    filenameResults: filteredFilenameResults,
    filteredResults,
    groupedResults,
    searchTime,
    isLoading,
    error,
    clearError,
    searchMode,
    setSearchMode,
    filters,
    setFilters,
    viewMode,
    setViewMode,
    refineQuery,
    setRefineQuery,
    clearRefine,
    isRefineActive: refineQuery.trim().length > 0,
  };
}
