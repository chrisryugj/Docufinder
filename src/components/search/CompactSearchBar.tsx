import { forwardRef, memo, useCallback, useMemo } from "react";
import { Search, HelpCircle, Settings } from "lucide-react";
import { useSearchInput } from "../../hooks/useSearchInput";
import { SearchModeDropdown } from "./SearchModeDropdown";
import type { SearchMode, SearchParadigm } from "../../types/search";
import type { IndexStatus } from "../../types/index";
import type {
  SearchFilters as FiltersType,
  ViewMode,
} from "../../types/search";
import { FilterDropdown, FilterChip } from "../ui/FilterDropdown";
import {
  SORT_OPTIONS,
  FILE_TYPE_OPTIONS,
  DATE_RANGE_OPTIONS,
} from "../../types/search";

interface CompactSearchBarProps {
  query: string;
  onQueryChange: (query: string) => void;
  searchMode: SearchMode;
  onSearchModeChange: (mode: SearchMode) => void;
  isLoading: boolean;
  status: IndexStatus | null;
  resultCount: number;
  onExpand: () => void;
  onAddFolder: () => void;
  onOpenSettings: () => void;
  onOpenHelp: () => void;
  isIndexing: boolean;
  isSidebarOpen: boolean;
  // 필터 관련
  filters: FiltersType;
  onFiltersChange: (filters: FiltersType) => void;
  viewMode: ViewMode;
  onViewModeChange: (mode: ViewMode) => void;
  refineQuery: string;
  onRefineQueryChange: (query: string) => void;
  onRefineQueryClear: () => void;
  totalResultCount: number;
  onCompositionStart?: () => void;
  onCompositionEnd?: (finalValue: string) => void;
  /** 검색 패러다임 */
  paradigm?: SearchParadigm;
  /** 자연어 검색 실행 */
  onSubmitNatural?: () => void;
}

export const CompactSearchBar = memo(forwardRef<HTMLInputElement, CompactSearchBarProps>(
  (
    {
      query,
      onQueryChange,
      searchMode,
      onSearchModeChange,
      isLoading,
      status,
      resultCount,
      onExpand,
      onOpenSettings,
      onOpenHelp,
      isSidebarOpen,
      // 필터 관련
      filters,
      onFiltersChange,
      viewMode,
      onViewModeChange,
      refineQuery,
      onRefineQueryChange,
      onRefineQueryClear,
      totalResultCount,
      onCompositionStart,
      onCompositionEnd,
      paradigm = "instant",
      onSubmitNatural,
    },
    ref
  ) => {
    const isNatural = paradigm === "natural";
    const { innerRef, imeHandlers } = useSearchInput({
      query,
      onQueryChange,
      onCompositionStart,
      onCompositionEnd,
      forwardedRef: ref,
    });

    const handleKeyDown = useCallback(
      (e: React.KeyboardEvent<HTMLInputElement>) => {
        if (isNatural && e.key === "Enter" && !e.nativeEvent.isComposing) {
          e.preventDefault();
          onSubmitNatural?.();
        }
      },
      [isNatural, onSubmitNatural]
    );

    // 활성 필터 라벨 생성 (filters 변경 시에만 재계산)
    const activeFilterLabels = useMemo(() => {
      const labels: { key: string; label: string; onRemove: () => void }[] = [];

      if (filters.sortBy !== "relevance") {
        const opt = SORT_OPTIONS.find((o) => o.value === filters.sortBy);
        labels.push({
          key: "sort",
          label: `정렬:${opt?.label || filters.sortBy}`,
          onRemove: () => onFiltersChange({ ...filters, sortBy: "relevance" }),
        });
      }

      if (filters.fileType !== "all") {
        const opt = FILE_TYPE_OPTIONS.find((o) => o.value === filters.fileType);
        labels.push({
          key: "fileType",
          label: `파일:${opt?.label || filters.fileType}`,
          onRemove: () => onFiltersChange({ ...filters, fileType: "all" }),
        });
      }

      if (filters.dateRange !== "all") {
        const opt = DATE_RANGE_OPTIONS.find((o) => o.value === filters.dateRange);
        labels.push({
          key: "dateRange",
          label: `날짜:${opt?.label || filters.dateRange}`,
          onRemove: () => onFiltersChange({ ...filters, dateRange: "all" }),
        });
      }

      if (filters.keywordOnly) {
        labels.push({
          key: "keywordOnly",
          label: "키워드만",
          onRemove: () => onFiltersChange({ ...filters, keywordOnly: false }),
        });
      }

      if (filters.excludeFilename) {
        labels.push({
          key: "excludeFilename",
          label: "파일명제외",
          onRemove: () => onFiltersChange({ ...filters, excludeFilename: false }),
        });
      }

      if (filters.searchScope !== null) {
        const normalized = filters.searchScope.replace(/\\\\\?\\/, "").replace(/\//g, "\\").replace(/\\$/, "");
        const parts = normalized.split("\\");
        const last = parts[parts.length - 1] || "";
        const scopeLabel = /^[A-Za-z]:?$/.test(last) ? last.replace(/:?$/, ":") : last || filters.searchScope;
        labels.push({
          key: "scope",
          label: `범위:${scopeLabel}`,
          onRemove: () => onFiltersChange({ ...filters, searchScope: null }),
        });
      }

      return labels;
    }, [filters, onFiltersChange]);

    return (
      <div
        className={`flex items-center gap-3 py-2 border-b transition-all duration-300 ${
          isSidebarOpen ? "px-4" : "pl-16 pr-4"
        }`}
        style={{
          backgroundColor: "var(--color-bg-primary)",
          borderColor: "var(--color-border)",
        }}
      >
        {/* 로고 (클릭 시 확장) */}
        <button
          onClick={onExpand}
          className="flex items-center gap-2 flex-shrink-0 hover:opacity-80 transition-opacity"
          aria-label="검색 영역 확장"
        >
          <img src="/anything.png" alt="Anything" className="w-6 h-6 object-contain dark:hidden" />
          <img src="/anything-l.png" alt="Anything" className="w-6 h-6 object-contain hidden dark:block" />
        </button>

        {/* 검색 입력 */}
        <div
          className="flex items-center flex-1 min-w-0 px-3 py-1.5 rounded-lg focus-within:ring-2 focus-within:ring-[var(--color-accent)] focus-within:ring-offset-1"
          style={{
            backgroundColor: "var(--color-bg-secondary)",
            border: "1px solid var(--color-border)",
          }}
        >
          <Search className="w-4 h-4 flex-shrink-0" style={{ color: "var(--color-text-muted)" }} />
          <input
            ref={innerRef}
            type="text"
            defaultValue={query}
            {...imeHandlers}
            onKeyDown={isNatural ? handleKeyDown : undefined}
            placeholder={isNatural ? "자연어로 검색 후 Enter…" : "예: 예산 집행현황, 민원처리 규정..."}
            className="flex-1 min-w-0 bg-transparent border-none text-sm focus:outline-none ml-2"
            style={{ color: "var(--color-text-primary)" }}
            aria-label="검색어 입력"
          />
          {isLoading && (
            <div
              className="w-4 h-4 rounded-full border-2 animate-spin ml-2 flex-shrink-0"
              style={{
                borderColor: "var(--color-border)",
                borderTopColor: "var(--color-accent)",
              }}
              role="status"
              aria-label="검색 중"
            />
          )}

          {!isNatural && (
            <SearchModeDropdown
              searchMode={searchMode}
              onSearchModeChange={onSearchModeChange}
              status={status}
            />
          )}
          {isNatural && (
            <span className="text-xs ml-2 flex-shrink-0" style={{ color: "var(--color-text-muted)" }}>
              Enter ↵
            </span>
          )}
        </div>

        {/* 필터 버튼 + 칩 */}
        {resultCount > 0 && (
          <>
            <FilterDropdown
              filters={filters}
              onFiltersChange={onFiltersChange}
              searchMode={searchMode}
              viewMode={viewMode}
              onViewModeChange={onViewModeChange}
              refineQuery={refineQuery}
              onRefineQueryChange={onRefineQueryChange}
              onRefineQueryClear={onRefineQueryClear}
              totalResultCount={totalResultCount}
            />

            {/* 활성 필터 칩 (최대 2개만 표시) */}
            {activeFilterLabels.slice(0, 2).map((f) => (
              <FilterChip key={f.key} label={f.label} onRemove={f.onRemove} />
            ))}
            {activeFilterLabels.length > 2 && (
              <span className="text-xs" style={{ color: "var(--color-text-muted)" }}>
                +{activeFilterLabels.length - 2}
              </span>
            )}
          </>
        )}

        {/* 결과 수 */}
        {resultCount > 0 && (
          <span className="text-xs font-medium flex-shrink-0" style={{ color: "var(--color-text-muted)" }}>
            {resultCount}건
          </span>
        )}

        {/* 구분선 */}
        <div className="w-px h-5 flex-shrink-0" style={{ backgroundColor: "var(--color-border)" }} />

        {/* 도움말 */}
        <button
          onClick={onOpenHelp}
          className="p-1.5 rounded hover:bg-[var(--color-bg-tertiary)] transition-colors flex-shrink-0"
          style={{ color: "var(--color-text-muted)" }}
          aria-label="도움말"
        >
          <HelpCircle className="w-4 h-4" />
        </button>

        {/* 설정 */}
        <button
          onClick={onOpenSettings}
          className="p-1.5 rounded hover:bg-[var(--color-bg-tertiary)] transition-colors flex-shrink-0"
          style={{ color: "var(--color-text-muted)" }}
          aria-label="설정"
        >
          <Settings className="w-4 h-4" />
        </button>
      </div>
    );
  }
));

CompactSearchBar.displayName = "CompactSearchBar";
