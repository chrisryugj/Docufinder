import { createContext, useContext, useRef, useCallback, useEffect, useMemo, type ReactNode } from "react";
import { useSearch, useAutoComplete, useCollapsibleSearch, useRecentSearches, useExport, useSimilarDocuments, useRecentSearchSaver, useResultSelection } from "../hooks";
import { clearSearchCache } from "../hooks/useSearch";
import { useAiSearch } from "../hooks/useAiSearch";
import { useFilterPresets, type FilterPreset } from "../hooks/useFilterPresets";
import { useTypoCorrection } from "../hooks/useTypoCorrection";
import type { SearchResult, SearchMode, SearchFilters, GroupedSearchResult, ViewMode, SearchParadigm, ParsedQueryInfo, SuggestionItem, RecentSearch, AiAnalysis } from "../types/search";
import { useUIContext } from "./UIContext";

// ── Types ──────────────────────────────────────────────

export interface SearchContextValue {
  // Core search
  query: string;
  setQuery: (q: string) => void;
  results: SearchResult[];
  filenameResults: SearchResult[];
  filteredResults: SearchResult[];
  groupedResults: GroupedSearchResult[];
  searchTime: number | null;
  isLoading: boolean;
  searchError: string | null;
  clearSearchError: () => void;
  searchMode: SearchMode;
  setSearchMode: (mode: SearchMode) => void;
  filters: SearchFilters;
  setFilters: (filters: SearchFilters | ((prev: SearchFilters) => SearchFilters)) => void;
  viewMode: ViewMode;
  setViewMode: (mode: ViewMode) => void;
  refineQuery: string;
  setRefineQuery: (q: string) => void;
  clearRefine: () => void;
  setComposing: (v: boolean, finalValue?: string) => void;
  invalidateSearch: () => void;
  paradigm: SearchParadigm;
  setParadigm: (p: SearchParadigm) => void;
  submitNaturalQuery: () => void;
  parsedQuery: ParsedQueryInfo | null;
  nlSubmitted: boolean;

  // Collapsible search area
  isCollapsed: boolean;
  handleScroll: (e: React.UIEvent<HTMLDivElement>) => void;
  scrollToTop: () => void;
  scrollContainerRef: React.RefObject<HTMLDivElement | null>;
  showScrollTop: boolean;
  expand: () => void;
  handleExpand: () => void;

  // Autocomplete
  autoComplete: {
    suggestions: SuggestionItem[];
    isOpen: boolean;
    selectedIndex: number;
    setSelectedIndex: (i: number) => void;
    close: () => void;
    handleKeyDown: (e: React.KeyboardEvent) => string | null;
  };
  handleSuggestionSelect: (text: string) => void;

  // Recent searches
  recentSearches: RecentSearch[];
  addSearch: (q: string) => void;
  removeSearch: (q: string) => void;
  clearSearches: () => void;
  handleSelectSearch: (q: string) => void;
  handleQueryChange: (q: string) => void;

  // Typo correction
  typoSuggestion: { suggestions: { word: string; distance: number; frequency: number }[] } | null;
  dismissTypo: () => void;

  // Filter presets
  presets: FilterPreset[];
  handleSavePreset: (name: string) => void;
  handleApplyPreset: (preset: FilterPreset) => void;
  removePreset: (id: string) => void;

  // Similar documents
  similarResults: SearchResult[];
  similarSourceFile: string | null;
  handleFindSimilar: (filePath: string) => void;
  clearSimilarResults: () => void;

  // Result selection
  selectedIndex: number;
  setSelectedIndex: (i: number) => void;

  // Export (memoized)
  handleExportCSV: () => void;
  handleExportXLSX: () => void;
  handleExportJSON: () => void;
  handlePackageZip: () => void;
  handleCopyAll: () => void;
  memoizedRefineKeywords: string[] | undefined;

  // AI search
  aiAnalysis: AiAnalysis | null;
  isAiLoading: boolean;
  aiError: string | null;
  requestAiAnalysis: (query: string, results: SearchResult[]) => void;
  clearAiAnalysis: () => void;
  handleAskAi: () => void;

  // Refs
  searchInputRef: React.RefObject<HTMLInputElement | null>;
  compactSearchInputRef: React.RefObject<HTMLInputElement | null>;

  // Utilities
  clearSearchCache: () => void;
}

// ── Context ────────────────────────────────────────────

const SearchContext = createContext<SearchContextValue | null>(null);

export function useSearchContext(): SearchContextValue {
  const ctx = useContext(SearchContext);
  if (!ctx) throw new Error("useSearchContext must be used within SearchProvider");
  return ctx;
}

// ── Constants ──────────────────────────────────────────

const EXPAND_FOCUS_DELAY_MS = 100;

// ── Provider ───────────────────────────────────────────

export function SearchProvider({ children }: { children: ReactNode }) {
  const { showToast, setPreviewFilePath } = useUIContext();

  const searchInputRef = useRef<HTMLInputElement | null>(null);
  const compactSearchInputRef = useRef<HTMLInputElement | null>(null);

  // ── Core Search ──
  const {
    query, setQuery, results, filenameResults, filteredResults, groupedResults,
    searchTime, isLoading, error: searchError, clearError: clearSearchError,
    searchMode, setSearchMode, filters, setFilters, viewMode, setViewMode,
    refineQuery, setRefineQuery, clearRefine, setComposing,
    invalidate: invalidateSearch, paradigm, setParadigm,
    submitNaturalQuery, parsedQuery, nlSubmitted,
  } = useSearch({ minConfidence: 0 });

  // ── Collapsible ──
  const {
    isCollapsed, handleScroll, scrollToTop, scrollContainerRef,
    showScrollTopButton: showScrollTop, expand,
  } = useCollapsibleSearch({
    threshold: 80,
    onCollapse: () => searchInputRef.current?.blur(),
    searchInputRef,
    query,
  });

  const handleExpand = useCallback(() => {
    expand();
    scrollToTop();
    setTimeout(() => searchInputRef.current?.focus(), EXPAND_FOCUS_DELAY_MS);
  }, [expand, scrollToTop]);

  // ── Autocomplete ──
  const autoComplete = useAutoComplete({ query });
  const autoCompleteCloseRef = useRef(autoComplete.close);
  autoCompleteCloseRef.current = autoComplete.close;

  // 결과 등장 후 800ms 뒤 추천어 자동 닫기
  useEffect(() => {
    if (!autoComplete.isOpen) return;
    if (results.length === 0 && filenameResults.length === 0) return;
    const timer = setTimeout(() => autoCompleteCloseRef.current(), 800);
    return () => clearTimeout(timer);
  }, [results, filenameResults, autoComplete.isOpen]);

  const handleSuggestionSelect = useCallback((text: string) => {
    setQuery(text);
    autoCompleteCloseRef.current();
    searchInputRef.current?.focus();
  }, [setQuery]);

  // ── Recent Searches ──
  const { searches: recentSearches, addSearch, removeSearch, clearSearches } = useRecentSearches();

  const handleSelectSearch = useCallback((searchQuery: string) => {
    setQuery(searchQuery);
    searchInputRef.current?.focus();
  }, [setQuery]);

  const handleQueryChange = useCallback((newQuery: string) => setQuery(newQuery), [setQuery]);

  // 검색 결과가 있고 3초 유지 시 최근 검색에 자동 저장
  useRecentSearchSaver(query, filteredResults.length, addSearch);

  // ── Typo Correction ──
  const { suggestion: typoSuggestion, dismiss: dismissTypo } = useTypoCorrection(query, results.length === 0 && !isLoading);

  // ── Filter Presets ──
  const { presets, addPreset, removePreset, applyPreset } = useFilterPresets();
  const handleSavePreset = useCallback((name: string) => {
    addPreset(name, filters);
    showToast(`프리셋 "${name}" 저장됨`, "success");
  }, [addPreset, filters, showToast]);
  const handleApplyPreset = useCallback((preset: FilterPreset) => {
    setFilters(applyPreset(preset, filters));
  }, [applyPreset, filters, setFilters]);

  // ── Similar Documents ──
  const { similarResults, similarSourceFile, handleFindSimilar, clearSimilarResults } = useSimilarDocuments(showToast);

  // ── Result Selection + Preview 연동 ──
  const { selectedIndex, setSelectedIndex } = useResultSelection(filteredResults, setPreviewFilePath);

  // ── Export (memoized) ──
  const { exportToCSV, exportToXLSX, exportToJSON, packageToZip, copyToClipboard } = useExport({ showToast });
  const handleExportCSV = useCallback(() => exportToCSV(filteredResults, query), [exportToCSV, filteredResults, query]);
  const handleExportXLSX = useCallback(() => exportToXLSX(filteredResults, query), [exportToXLSX, filteredResults, query]);
  const handleExportJSON = useCallback(() => exportToJSON(filteredResults, query), [exportToJSON, filteredResults, query]);
  const handlePackageZip = useCallback(() => packageToZip(filteredResults), [packageToZip, filteredResults]);
  const handleCopyAll = useCallback(() => copyToClipboard(filteredResults, query), [copyToClipboard, filteredResults, query]);
  const memoizedRefineKeywords = useMemo(
    () => refineQuery.trim() ? refineQuery.trim().split(/\s+/) : undefined,
    [refineQuery]
  );

  // ── AI Search ──
  const { aiAnalysis, isAiLoading, aiError, requestAiAnalysis, clearAiAnalysis } = useAiSearch();

  // 자연어 모드: 검색 완료 시 AI 자동 분석
  const aiAutoRef = useRef({ aiEnabled: false, paradigm, query, filteredResults, isLoading, requestAiAnalysis });
  aiAutoRef.current = { aiEnabled: false, paradigm, query, filteredResults, isLoading, requestAiAnalysis };
  // Note: aiEnabled는 AppContent에서 별도 effect로 처리

  const handleAskAi = useCallback(() => {
    if (query.trim() && filteredResults.length > 0) {
      requestAiAnalysis(query, filteredResults);
    }
  }, [query, filteredResults, requestAiAnalysis]);

  // ── searchMode 변경 시 keywordOnly 리셋 ──
  useEffect(() => {
    if (searchMode !== "hybrid") {
      setFilters((prev) => prev.keywordOnly ? { ...prev, keywordOnly: false } : prev);
    }
  }, [searchMode, setFilters]);

  const value: SearchContextValue = {
    query, setQuery, results, filenameResults, filteredResults, groupedResults,
    searchTime, isLoading, searchError, clearSearchError,
    searchMode, setSearchMode, filters, setFilters, viewMode, setViewMode,
    refineQuery, setRefineQuery, clearRefine, setComposing,
    invalidateSearch, paradigm, setParadigm, submitNaturalQuery, parsedQuery, nlSubmitted,
    isCollapsed, handleScroll, scrollToTop, scrollContainerRef, showScrollTop, expand, handleExpand,
    autoComplete, handleSuggestionSelect,
    recentSearches, addSearch, removeSearch, clearSearches, handleSelectSearch, handleQueryChange,
    typoSuggestion, dismissTypo,
    presets, handleSavePreset, handleApplyPreset, removePreset,
    similarResults, similarSourceFile, handleFindSimilar, clearSimilarResults,
    selectedIndex, setSelectedIndex,
    handleExportCSV, handleExportXLSX, handleExportJSON, handlePackageZip, handleCopyAll, memoizedRefineKeywords,
    aiAnalysis, isAiLoading, aiError, requestAiAnalysis, clearAiAnalysis, handleAskAi,
    searchInputRef, compactSearchInputRef,
    clearSearchCache,
  };

  return <SearchContext.Provider value={value}>{children}</SearchContext.Provider>;
}
