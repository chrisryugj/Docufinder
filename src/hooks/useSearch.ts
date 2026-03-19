import { useState, useEffect, useCallback, useMemo, useRef, startTransition } from "react";
import { invokeWithTimeout, IPC_TIMEOUT } from "../utils/invokeWithTimeout";
import { logToBackend } from "../utils/errorLogger";
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

// LRU 캐시 (검색 결과 캐싱)
const CACHE_MAX_SIZE = 50;
const CACHE_TTL_MS = 30000; // 30초
const COMPOSITION_IDLE_MS = 300;

interface CacheEntry {
  results: SearchResult[];
  filenameResults: SearchResult[];
  searchTime: number;
  timestamp: number;
}

const searchCache = new Map<string, CacheEntry>();
let sweepTimerId: ReturnType<typeof setInterval> | null = null;

/** 만료 엔트리 proactive sweep (CACHE_TTL_MS 간격) */
function ensureSweepTimer(): void {
  if (sweepTimerId !== null) return;
  sweepTimerId = setInterval(() => {
    const now = Date.now();
    for (const [key, entry] of searchCache) {
      if (now - entry.timestamp > CACHE_TTL_MS) {
        searchCache.delete(key);
      }
    }
    // 캐시 비면 타이머 중지 (리소스 절약)
    if (searchCache.size === 0 && sweepTimerId !== null) {
      clearInterval(sweepTimerId);
      sweepTimerId = null;
    }
  }, CACHE_TTL_MS);
}

/** 검색 캐시 전체 초기화 (데이터 리셋 시 호출) */
export function clearSearchCache(): void {
  searchCache.clear();
  if (sweepTimerId !== null) {
    clearInterval(sweepTimerId);
    sweepTimerId = null;
  }
}

function getCacheKey(query: string, mode: SearchMode, excludeFilename: boolean, searchScope: string | null): string {
  return JSON.stringify([mode, excludeFilename, searchScope, query.trim().toLowerCase()]);
}

function getFromCache(key: string): CacheEntry | null {
  const entry = searchCache.get(key);
  if (!entry) return null;

  // TTL 체크
  if (Date.now() - entry.timestamp > CACHE_TTL_MS) {
    searchCache.delete(key);
    return null;
  }

  // LRU: 접근 시 맨 뒤로 이동
  searchCache.delete(key);
  searchCache.set(key, entry);
  return entry;
}

function setToCache(key: string, entry: Omit<CacheEntry, "timestamp">): void {
  // LRU: 최대 크기 초과 시 가장 오래된 항목 제거
  if (searchCache.size >= CACHE_MAX_SIZE) {
    const firstKey = searchCache.keys().next().value;
    if (firstKey) searchCache.delete(firstKey);
  }
  searchCache.set(key, { ...entry, timestamp: Date.now() });
  ensureSweepTimer();
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
  setFilters: (filters: SearchFilters | ((prev: SearchFilters) => SearchFilters)) => void;
  viewMode: ViewMode;
  setViewMode: (mode: ViewMode) => void;
  /** 결과 내 검색 쿼리 */
  refineQuery: string;
  setRefineQuery: (query: string) => void;
  /** 결과 내 검색 초기화 */
  clearRefine: () => void;
  /** 결과 내 검색 활성화 여부 */
  isRefineActive: boolean;
  /** IME 조합 상태 설정 (compositionEnd 시 최종 쿼리 전달) */
  setComposing: (v: boolean, finalQuery?: string) => void;
  /** 캐시 무효화 + 재검색 (데이터 변경 시) */
  invalidate: () => void;
}

/**
 * 검색 로직 훅 (디바운스 포함)
 */
export function useSearch(options: UseSearchOptions = {}): UseSearchReturn {
  const { debounceMs = 150 } = options;
  // minConfidence는 외부에서 변경될 수 있으므로 직접 참조
  const minConfidence = options.minConfidence ?? 0;

  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [filenameResults, setFilenameResults] = useState<SearchResult[]>([]);
  const [searchTime, setSearchTime] = useState<number | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [searchMode, setSearchMode] = useState<SearchMode>("keyword");
  // IME 조합 중 여부
  const isComposingRef = useRef(false);
  // 검색 요청 ID (이전 검색 결과 무시용)
  const searchIdRef = useRef(0);
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
  const [debouncedRefineQuery, setDebouncedRefineQuery] = useState("");

  // refineQuery debounce (300ms)
  useEffect(() => {
    const timer = setTimeout(() => setDebouncedRefineQuery(refineQuery), 300);
    return () => clearTimeout(timer);
  }, [refineQuery]);

  const clearError = useCallback(() => setError(null), []);
  const clearRefine = useCallback(() => { setRefineQuery(""); setDebouncedRefineQuery(""); }, []);

  // excludeFilename 변경 시 localStorage 저장
  const prevExcludeFilename = useRef(filters.excludeFilename);
  const setFilters = useCallback((newFiltersOrUpdater: SearchFilters | ((prev: SearchFilters) => SearchFilters)) => {
    setFiltersInternal((prev) => {
      const newFilters = typeof newFiltersOrUpdater === "function" ? newFiltersOrUpdater(prev) : newFiltersOrUpdater;
      if (newFilters.excludeFilename !== prevExcludeFilename.current) {
        prevExcludeFilename.current = newFilters.excludeFilename;
        try {
          localStorage.setItem("docufinder_exclude_filename", JSON.stringify(newFilters.excludeFilename));
        } catch {}
      }
      return newFilters;
    });
  }, []);

  // 검색 실행 함수 (결과 업데이트는 startTransition으로 입력 블로킹 방지)
  const executeSearch = useCallback(
    async (searchQuery: string, mode: SearchMode) => {
      // 항상 ID 증가 — 빈 쿼리/캐시 히트 시에도 이전 비동기 검색 무효화
      const currentId = ++searchIdRef.current;

      if (!searchQuery.trim()) {
        startTransition(() => {
          setResults([]);
          setFilenameResults([]);
          setSearchTime(null);
        });
        setIsLoading(false);
        return;
      }

      // LRU 캐시 확인
      const cacheKey = getCacheKey(searchQuery, mode, filters.excludeFilename, filters.searchScope);
      const cached = getFromCache(cacheKey);
      if (cached) {
        startTransition(() => {
          setResults(cached.results);
          setFilenameResults(cached.filenameResults);
          setSearchTime(cached.searchTime);
        });
        setIsLoading(false);
        return;
      }

      setIsLoading(true);
      setError(null);

      try {
        if (mode === "filename") {
          const response = await invokeWithTimeout<SearchResponse>(SEARCH_COMMANDS[mode], {
            query: searchQuery,
            folderScope: filters.searchScope,
          }, IPC_TIMEOUT.SEARCH);
          if (searchIdRef.current !== currentId) return;
          // 캐시 저장
          setToCache(cacheKey, {
            results: response.results,
            filenameResults: [],
            searchTime: response.search_time_ms,
          });
          startTransition(() => {
            setResults(response.results);
            setFilenameResults([]);
            setSearchTime(response.search_time_ms);
            setIsLoading(false);
          });
          return;
        } else {
          // excludeFilename이면 파일명 검색 스킵 (불필요한 백엔드 호출 방지)
          const ipcArgs = { query: searchQuery, folderScope: filters.searchScope };
          const contentPromise = invokeWithTimeout<SearchResponse>(SEARCH_COMMANDS[mode], ipcArgs, IPC_TIMEOUT.SEARCH);
          const filenamePromise = filters.excludeFilename
            ? Promise.resolve({ results: [], search_time_ms: 0, total_count: 0 })
            : invokeWithTimeout<SearchResponse>(SEARCH_COMMANDS.filename, ipcArgs, IPC_TIMEOUT.SEARCH);

          // 파일명 검색 실패가 본문 검색까지 죽이지 않도록 allSettled 사용
          const [contentResult, filenameResult] = await Promise.allSettled([
            contentPromise,
            filenamePromise,
          ]);
          if (searchIdRef.current !== currentId) return;

          // 본문 검색 실패 시 에러 throw
          if (contentResult.status === "rejected") {
            throw contentResult.reason;
          }
          const contentResponse = contentResult.value;
          // 파일명 검색 실패 시 graceful degrade
          const filenameResponse = filenameResult.status === "fulfilled"
            ? filenameResult.value
            : { results: [], search_time_ms: 0, total_count: 0 };

          // 캐시 저장
          setToCache(cacheKey, {
            results: contentResponse.results,
            filenameResults: filenameResponse.results,
            searchTime: contentResponse.search_time_ms,
          });
          startTransition(() => {
            setResults(contentResponse.results);
            setFilenameResults(filenameResponse.results);
            setSearchTime(contentResponse.search_time_ms);
            setIsLoading(false);
          });
        }
      } catch (err) {
        if (searchIdRef.current !== currentId) return;
        const message = err instanceof Error ? err.message : String(err);
        logToBackend("error", "Search failed", message, "useSearch");
        setError(`검색 실패: ${message}`);
        startTransition(() => {
          setResults([]);
          setFilenameResults([]);
          setSearchTime(null);
          setIsLoading(false);
        });
      }
    },
    [filters.excludeFilename, filters.searchScope]
  );

  // IME 상태 설정 (SearchBar에서 호출)
  // compositionEnd 이후에도 debounce에 의해 검색되도록만 상태 갱신
  const setComposing = useCallback((v: boolean, finalQuery?: string) => {
    isComposingRef.current = v;
    if (!v && finalQuery !== undefined && finalQuery !== query) {
      setQuery(finalQuery);
    }
  }, [query, setQuery]);

  // 디바운스 검색 — 조합 중에는 대기, 완료 후 실행
  useEffect(() => {
    const delay = isComposingRef.current
      ? Math.max(debounceMs, COMPOSITION_IDLE_MS)
      : debounceMs;
    const timer = setTimeout(() => {
      if (isComposingRef.current) {
        isComposingRef.current = false;
      }
      executeSearch(query, searchMode);
    }, delay);

    return () => clearTimeout(timer);
  }, [query, searchMode, debounceMs, executeSearch, COMPOSITION_IDLE_MS]);

  // 필터링된 결과
  const filteredResults = useMemo(() => {
    const needsFilter =
      minConfidence > 0 ||
      filters.keywordOnly ||
      filters.fileType !== "all" ||
      filters.dateRange !== "all" ||
      debouncedRefineQuery.trim().length > 0;
    const needsSort = filters.sortBy !== "relevance";

    // 필터/정렬 불필요 시 원본 반환 (배열 복사 회피)
    if (!needsFilter && !needsSort) {
      return results;
    }

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

    // 날짜 범위 필터
    if (filters.dateRange !== "all") {
      const nowSec = Math.floor(Date.now() / 1000);
      const cutoff: Record<string, number> = {
        today: nowSec - 86400,
        week: nowSec - 86400 * 7,
        month: nowSec - 86400 * 30,
      };
      const minTime = cutoff[filters.dateRange] ?? 0;
      filtered = filtered.filter((r) => (r.modified_at ?? 0) >= minTime);
    }

    // 파일 타입 필터
    if (filters.fileType !== "all") {
      const extMap: Record<FileTypeFilter, string[]> = {
        all: [],
        hwpx: ["hwpx"],
        docx: ["docx", "doc"],
        pptx: ["pptx", "ppt"],
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

    // 결과 내 검색 필터링 (debounced)
    // ⚡ full_content 대신 snippet/content_preview 사용 (성능 최적화)
    if (debouncedRefineQuery.trim()) {
      const refineKeywords = debouncedRefineQuery.trim().toLowerCase().split(/\s+/);
      filtered = filtered.filter((r) => {
        const content = (r.snippet || r.content_preview || "").toLowerCase();
        // 모든 키워드가 포함되어야 함 (AND 조건)
        return refineKeywords.every((kw) => content.includes(kw));
      });
    }

    return filtered;
  }, [results, filters, minConfidence, debouncedRefineQuery]);

  // 파일명 검색 결과도 결과 내 검색 필터링
  const filteredFilenameResults = useMemo(() => {
    if (!debouncedRefineQuery.trim()) {
      return filenameResults;
    }
    const keywords = debouncedRefineQuery.trim().toLowerCase().split(/\s+/);
    return filenameResults.filter((r) => {
      const fileName = r.file_name.toLowerCase();
      // 파일명에서 키워드 검색
      return keywords.every((kw) => fileName.includes(kw));
    });
  }, [filenameResults, debouncedRefineQuery]);

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

  // 캐시 무효화 + 재검색 (폴더 삭제 등 데이터 변경 시)
  const invalidate = useCallback(() => {
    searchCache.clear();
    if (query.trim()) {
      executeSearch(query, searchMode);
    } else {
      startTransition(() => {
        setResults([]);
        setFilenameResults([]);
        setSearchTime(null);
      });
    }
  }, [query, searchMode, executeSearch]);

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
    setComposing,
    invalidate,
  };
}
