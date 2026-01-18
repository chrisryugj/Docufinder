import { forwardRef, useCallback, useRef, useEffect, useState } from "react";
import type { SearchMode } from "../../types/search";
import { SEARCH_MODES } from "../../types/search";
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
}

export const CompactSearchBar = forwardRef<HTMLInputElement, CompactSearchBarProps>(
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
    },
    ref
  ) => {
    const innerRef = useRef<HTMLInputElement>(null);
    const [showModeDropdown, setShowModeDropdown] = useState(false);
    const dropdownRef = useRef<HTMLDivElement>(null);

    // 드롭다운 외부 클릭 시 닫기
    useEffect(() => {
      const handleClickOutside = (e: MouseEvent) => {
        if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
          setShowModeDropdown(false);
        }
      };
      if (showModeDropdown) {
        document.addEventListener("mousedown", handleClickOutside);
      }
      return () => document.removeEventListener("mousedown", handleClickOutside);
    }, [showModeDropdown]);

    // ref 병합
    useEffect(() => {
      if (!ref) return;
      if (typeof ref === "function") {
        ref(innerRef.current);
      } else {
        ref.current = innerRef.current;
      }
    }, [ref]);

    // 입력값 동기화
    useEffect(() => {
      if (innerRef.current && innerRef.current.value !== query) {
        innerRef.current.value = query;
      }
    }, [query]);

    // 활성 필터 라벨 생성
    const getActiveFilterLabels = useCallback(() => {
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

      return labels;
    }, [filters, onFiltersChange]);

    const activeFilterLabels = getActiveFilterLabels();
    const currentMode = SEARCH_MODES.find((m) => m.value === searchMode);

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
          <img src="/icon.png" alt="DocuFinder" className="w-6 h-6" />
        </button>

        {/* 검색 입력 */}
        <div
          className="flex items-center flex-1 min-w-0 px-3 py-1.5 rounded-lg"
          style={{
            backgroundColor: "var(--color-bg-secondary)",
            border: "1px solid var(--color-border)",
          }}
        >
          <svg
            className="w-4 h-4 flex-shrink-0"
            fill="none"
            stroke="currentColor"
            strokeWidth={2}
            viewBox="0 0 24 24"
            style={{ color: "var(--color-text-muted)" }}
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
            />
          </svg>
          <input
            ref={innerRef}
            type="text"
            defaultValue={query}
            onChange={(e) => onQueryChange(e.target.value)}
            placeholder="검색어 입력..."
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
            />
          )}

          {/* 검색 모드 배지 + 드롭다운 */}
          <div ref={dropdownRef} className="relative ml-2 flex-shrink-0">
            <button
              onClick={() => setShowModeDropdown(!showModeDropdown)}
              className="flex items-center gap-1 px-2 py-1 rounded-md text-xs font-medium transition-colors"
              style={{
                backgroundColor: "var(--color-bg-tertiary)",
                color: "var(--color-text-secondary)",
                border: "1px solid var(--color-border)",
              }}
              title={currentMode?.desc}
            >
              {currentMode?.label}
              <svg
                className={`w-3 h-3 transition-transform ${showModeDropdown ? "rotate-180" : ""}`}
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
              </svg>
            </button>

            {showModeDropdown && (
              <div
                className="absolute top-full right-0 mt-1 py-1 rounded-lg shadow-lg z-50 min-w-[140px]"
                style={{
                  backgroundColor: "var(--color-bg-secondary)",
                  border: "1px solid var(--color-border)",
                }}
              >
                {SEARCH_MODES.map((mode) => {
                  const needsSemantic = mode.value === "semantic" || mode.value === "hybrid";
                  const disabled = needsSemantic && !status?.semantic_available;
                  const isActive = searchMode === mode.value;

                  return (
                    <button
                      key={mode.value}
                      onClick={() => {
                        if (!disabled) {
                          onSearchModeChange(mode.value);
                          setShowModeDropdown(false);
                        }
                      }}
                      disabled={disabled}
                      className={`
                        w-full px-3 py-1.5 text-xs text-left transition-colors
                        ${disabled ? "opacity-40 cursor-not-allowed" : "cursor-pointer"}
                      `}
                      style={{
                        backgroundColor: isActive ? "var(--color-accent-light)" : "transparent",
                        color: isActive ? "var(--color-accent)" : "var(--color-text-secondary)",
                      }}
                      onMouseEnter={(e) => {
                        if (!disabled && !isActive) {
                          e.currentTarget.style.backgroundColor = "var(--color-bg-tertiary)";
                        }
                      }}
                      onMouseLeave={(e) => {
                        if (!isActive) {
                          e.currentTarget.style.backgroundColor = "transparent";
                        }
                      }}
                      title={disabled ? "모델 파일 필요" : mode.desc}
                    >
                      <div className="font-medium">{mode.label}</div>
                      <div className="text-[10px] opacity-70">{mode.desc}</div>
                    </button>
                  );
                })}
              </div>
            )}
          </div>
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
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M8.228 9c.549-1.165 2.03-2 3.772-2 2.21 0 4 1.343 4 3 0 1.4-1.278 2.575-3.006 2.907-.542.104-.994.54-.994 1.093m0 3h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
            />
          </svg>
        </button>

        {/* 설정 */}
        <button
          onClick={onOpenSettings}
          className="p-1.5 rounded hover:bg-[var(--color-bg-tertiary)] transition-colors flex-shrink-0"
          style={{ color: "var(--color-text-muted)" }}
          aria-label="설정"
        >
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"
            />
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
          </svg>
        </button>
      </div>
    );
  }
);

CompactSearchBar.displayName = "CompactSearchBar";
