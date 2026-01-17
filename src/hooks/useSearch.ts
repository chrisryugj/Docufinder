import { useState, useEffect, useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import type {
  SearchResult,
  SearchResponse,
  SearchMode,
  SearchFilters,
  FileTypeFilter,
} from "../types/search";
import { DEFAULT_FILTERS } from "../types/search";
import { SEARCH_COMMANDS } from "../types/api";

interface UseSearchOptions {
  debounceMs?: number;
}

interface UseSearchReturn {
  query: string;
  setQuery: (query: string) => void;
  results: SearchResult[];
  filteredResults: SearchResult[];
  searchTime: number | null;
  isLoading: boolean;
  error: string | null;
  clearError: () => void;
  searchMode: SearchMode;
  setSearchMode: (mode: SearchMode) => void;
  filters: SearchFilters;
  setFilters: (filters: SearchFilters) => void;
}

/**
 * 검색 로직 훅 (디바운스 포함)
 */
export function useSearch(options: UseSearchOptions = {}): UseSearchReturn {
  const { debounceMs = 300 } = options;

  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [searchTime, setSearchTime] = useState<number | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [searchMode, setSearchMode] = useState<SearchMode>("keyword");
  const [filters, setFilters] = useState<SearchFilters>(DEFAULT_FILTERS);

  const clearError = useCallback(() => setError(null), []);

  // 검색 실행 함수
  const executeSearch = useCallback(
    async (searchQuery: string, mode: SearchMode) => {
      if (!searchQuery.trim()) {
        setResults([]);
        setSearchTime(null);
        return;
      }

      setIsLoading(true);
      setError(null);

      try {
        const response = await invoke<SearchResponse>(SEARCH_COMMANDS[mode], {
          query: searchQuery,
        });
        setResults(response.results);
        setSearchTime(response.search_time_ms);
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        console.error("Search failed:", err);
        setError(`검색 실패: ${message}`);
        setResults([]);
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
      case "date_desc":
        // 파일 수정일이 없으므로 현재는 변경 없음
        // TODO: 백엔드에서 수정일 추가 시 정렬 구현
        break;
      case "date_asc":
        // TODO: 수정일 역순
        break;
      case "name":
        filtered.sort((a, b) => a.file_name.localeCompare(b.file_name, "ko"));
        break;
    }

    return filtered;
  }, [results, filters]);

  return {
    query,
    setQuery,
    results,
    filteredResults,
    searchTime,
    isLoading,
    error,
    clearError,
    searchMode,
    setSearchMode,
    filters,
    setFilters,
  };
}
