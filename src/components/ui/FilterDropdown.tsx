import { useState, useRef, useEffect, useCallback } from "react";
import type {
  SearchFilters as FiltersType,
  SortOption,
  DateRangeFilter,
  ViewMode,
  SearchMode,
} from "../../types/search";
import {
  SORT_OPTIONS,
  FILE_TYPE_OPTIONS,
  DATE_RANGE_OPTIONS,
  DEFAULT_FILTERS,
} from "../../types/search";
import { CustomSelect } from "./CustomSelect";

interface FilterDropdownProps {
  filters: FiltersType;
  onFiltersChange: (filters: FiltersType) => void;
  searchMode?: SearchMode;
  viewMode?: ViewMode;
  onViewModeChange?: (mode: ViewMode) => void;
  refineQuery?: string;
  onRefineQueryChange?: (query: string) => void;
  onRefineQueryClear?: () => void;
  totalResultCount?: number;
}

export function FilterDropdown({
  filters,
  onFiltersChange,
  searchMode,
  viewMode = "flat",
  onViewModeChange,
  refineQuery = "",
  onRefineQueryChange,
  onRefineQueryClear,
  totalResultCount,
}: FilterDropdownProps) {
  const [isOpen, setIsOpen] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  const hasActiveFilters =
    filters.sortBy !== "relevance" ||
    filters.fileTypes.length > 0 ||
    filters.dateRange !== "all" ||
    filters.keywordOnly ||
    filters.excludeFilename;

  const activeFilterCount = [
    filters.sortBy !== "relevance",
    filters.fileTypes.length > 0,
    filters.dateRange !== "all",
    filters.keywordOnly,
    filters.excludeFilename,
  ].filter(Boolean).length;

  const showKeywordOnlyToggle = searchMode === "hybrid";
  const showExcludeFilenameToggle = searchMode !== "filename";

  // 외부 클릭 시 닫기
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        setIsOpen(false);
      }
    };

    if (isOpen) {
      document.addEventListener("mousedown", handleClickOutside);
    }

    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
    };
  }, [isOpen]);

  // ESC 키로 닫기
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape" && isOpen) {
        setIsOpen(false);
      }
    };

    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [isOpen]);

  const handleReset = useCallback(() => {
    onFiltersChange(DEFAULT_FILTERS);
    onRefineQueryClear?.();
  }, [onFiltersChange, onRefineQueryClear]);

  return (
    <div ref={dropdownRef} className="relative inline-block">
      {/* 필터 버튼 */}
      <button
        onClick={() => setIsOpen(!isOpen)}
        className="flex items-center gap-1 px-2 py-1 rounded-md border text-xs font-medium transition-colors"
        style={{
          backgroundColor: hasActiveFilters ? "var(--color-accent-light)" : "var(--color-bg-secondary)",
          borderColor: hasActiveFilters ? "var(--color-accent)" : "var(--color-border)",
          color: hasActiveFilters ? "var(--color-accent)" : "var(--color-text-secondary)",
        }}
        aria-expanded={isOpen}
        aria-haspopup="menu"
      >
        <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 4a1 1 0 011-1h16a1 1 0 011 1v2.586a1 1 0 01-.293.707l-6.414 6.414a1 1 0 00-.293.707V17l-4 4v-6.586a1 1 0 00-.293-.707L3.293 7.293A1 1 0 013 6.586V4z" />
        </svg>
        필터
        {activeFilterCount > 0 && (
          <span
            className="flex items-center justify-center w-4 h-4 rounded-full text-[10px] font-bold"
            style={{ backgroundColor: "var(--color-accent)", color: "white" }}
          >
            {activeFilterCount}
          </span>
        )}
        <svg
          className={`w-3 h-3 transition-transform ${isOpen ? "rotate-180" : ""}`}
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
        </svg>
      </button>

      {/* 드롭다운 패널 */}
      {isOpen && (
        <div
          className="absolute left-0 top-full mt-2 w-72 rounded-lg border shadow-lg z-50 animate-scale-in"
          style={{
            backgroundColor: "var(--color-bg-secondary)",
            borderColor: "var(--color-border)",
          }}
        >
          <div className="p-3 space-y-3">
            {/* 정렬 */}
            <FilterSelect
              label="정렬"
              value={filters.sortBy}
              options={SORT_OPTIONS}
              onChange={(v) => onFiltersChange({ ...filters, sortBy: v as SortOption })}
            />

            {/* 확장자 (다중 선택) */}
            <div>
              <span className="text-sm font-medium" style={{ color: "var(--color-text-secondary)" }}>
                확장자
              </span>
              <div className="flex flex-wrap gap-1.5 mt-1">
                {FILE_TYPE_OPTIONS.map((opt) => {
                  const checked = filters.fileTypes.includes(opt.value);
                  return (
                    <label
                      key={opt.value}
                      className="flex items-center gap-1 px-2 py-0.5 rounded border cursor-pointer transition-colors text-xs"
                      style={{
                        borderColor: checked ? "var(--color-accent)" : "var(--color-border)",
                        backgroundColor: checked ? "var(--color-accent-light)" : "transparent",
                        color: checked ? "var(--color-accent)" : "var(--color-text-secondary)",
                        fontWeight: checked ? 600 : 400,
                      }}
                    >
                      <input
                        type="checkbox"
                        checked={checked}
                        onChange={() => {
                          const prev = filters.fileTypes;
                          const next = prev.includes(opt.value)
                            ? prev.filter((t) => t !== opt.value)
                            : [...prev, opt.value];
                          onFiltersChange({ ...filters, fileTypes: next });
                        }}
                        className="accent-[var(--color-accent)] w-3 h-3"
                      />
                      {opt.label}
                    </label>
                  );
                })}
              </div>
            </div>

            {/* 날짜 범위 */}
            <FilterSelect
              label="날짜"
              value={filters.dateRange}
              options={
                filters.dateRange.startsWith("custom:")
                  ? [...DATE_RANGE_OPTIONS, { value: filters.dateRange, label: `${filters.dateRange.slice(7)}일` }]
                  : DATE_RANGE_OPTIONS
              }
              onChange={(v) => onFiltersChange({ ...filters, dateRange: v as DateRangeFilter })}
            />

            {/* 체크박스 옵션들 */}
            {(showKeywordOnlyToggle || showExcludeFilenameToggle) && (
              <div className="pt-2 border-t space-y-2" style={{ borderColor: "var(--color-border)" }}>
                {showKeywordOnlyToggle && (
                  <label className="flex items-center gap-2 cursor-pointer">
                    <input
                      type="checkbox"
                      checked={filters.keywordOnly}
                      onChange={(e) => onFiltersChange({ ...filters, keywordOnly: e.target.checked })}
                      className="accent-blue-500 w-4 h-4"
                    />
                    <span className="text-sm" style={{ color: "var(--color-text-secondary)" }}>
                      키워드 포함 결과만
                    </span>
                  </label>
                )}

                {showExcludeFilenameToggle && (
                  <label className="flex items-center gap-2 cursor-pointer">
                    <input
                      type="checkbox"
                      checked={filters.excludeFilename}
                      onChange={(e) => onFiltersChange({ ...filters, excludeFilename: e.target.checked })}
                      className="accent-blue-500 w-4 h-4"
                    />
                    <span className="text-sm" style={{ color: "var(--color-text-secondary)" }}>
                      파일명 검색 제외
                    </span>
                  </label>
                )}
              </div>
            )}

            {/* 결과 내 검색 */}
            {onRefineQueryChange && totalResultCount !== undefined && totalResultCount > 0 && (
              <div className="pt-2 border-t" style={{ borderColor: "var(--color-border)" }}>
                <label className="block text-xs font-medium mb-1.5" style={{ color: "var(--color-text-muted)" }}>
                  결과 내 검색
                </label>
                <div className="relative">
                  <input
                    type="text"
                    value={refineQuery}
                    onChange={(e) => onRefineQueryChange(e.target.value)}
                    placeholder="키워드 입력..."
                    className="w-full pl-8 pr-8 py-2 rounded-md border text-sm focus:outline-none focus:ring-1"
                    style={{
                      backgroundColor: "var(--color-bg-primary)",
                      borderColor: refineQuery ? "var(--color-accent)" : "var(--color-border)",
                      color: "var(--color-text-primary)",
                    }}
                  />
                  <svg
                    className="absolute left-2.5 top-1/2 -translate-y-1/2 w-4 h-4"
                    style={{ color: refineQuery ? "var(--color-accent)" : "var(--color-text-muted)" }}
                    fill="none"
                    stroke="currentColor"
                    viewBox="0 0 24 24"
                  >
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
                  </svg>
                  {refineQuery && onRefineQueryClear && (
                    <button
                      onClick={onRefineQueryClear}
                      className="absolute right-2.5 top-1/2 -translate-y-1/2 p-0.5 rounded-full hover:bg-gray-200"
                      style={{ color: "var(--color-text-muted)" }}
                    >
                      <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                      </svg>
                    </button>
                  )}
                </div>
              </div>
            )}

            {/* 뷰 모드 */}
            {onViewModeChange && (
              <div className="pt-2 border-t" style={{ borderColor: "var(--color-border)" }}>
                <label className="block text-xs font-medium mb-1.5" style={{ color: "var(--color-text-muted)" }}>
                  보기 방식
                </label>
                <div className="flex gap-2">
                  <button
                    onClick={() => onViewModeChange("flat")}
                    className="flex-1 flex items-center justify-center gap-1.5 px-3 py-2 rounded-md text-sm font-medium transition-colors"
                    style={{
                      backgroundColor: viewMode === "flat" ? "var(--color-accent-light)" : "var(--color-bg-tertiary)",
                      color: viewMode === "flat" ? "var(--color-accent)" : "var(--color-text-secondary)",
                    }}
                  >
                    <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 6h16M4 12h16M4 18h16" />
                    </svg>
                    목록
                  </button>
                  <button
                    onClick={() => onViewModeChange("grouped")}
                    className="flex-1 flex items-center justify-center gap-1.5 px-3 py-2 rounded-md text-sm font-medium transition-colors"
                    style={{
                      backgroundColor: viewMode === "grouped" ? "var(--color-accent-light)" : "var(--color-bg-tertiary)",
                      color: viewMode === "grouped" ? "var(--color-accent)" : "var(--color-text-secondary)",
                    }}
                  >
                    <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
                    </svg>
                    그룹
                  </button>
                </div>
              </div>
            )}

            {/* 초기화 버튼 */}
            {hasActiveFilters && (
              <div className="pt-2 border-t" style={{ borderColor: "var(--color-border)" }}>
                <button
                  onClick={handleReset}
                  className="w-full px-3 py-2 rounded-md text-sm font-medium transition-colors"
                  style={{
                    backgroundColor: "rgba(239, 68, 68, 0.1)",
                    color: "var(--color-error)",
                  }}
                >
                  필터 초기화
                </button>
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

// 활성 필터 칩
interface FilterChipProps {
  label: string;
  onRemove: () => void;
}

export function FilterChip({ label, onRemove }: FilterChipProps) {
  return (
    <span
      className="inline-flex items-center gap-1 px-2 py-1 rounded-md text-xs font-medium"
      style={{
        backgroundColor: "var(--color-accent-light)",
        color: "var(--color-accent)",
        border: "1px solid var(--color-accent)",
      }}
    >
      {label}
      <button
        onClick={onRemove}
        className="p-0.5 rounded-full hover:bg-blue-200 transition-colors"
        aria-label={`${label} 필터 제거`}
      >
        <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
        </svg>
      </button>
    </span>
  );
}

// 필터 셀렉트 (드롭다운 내부용)
interface FilterSelectProps<T extends string> {
  label: string;
  value: T;
  options: { value: T; label: string }[];
  onChange: (value: T) => void;
}

function FilterSelect<T extends string>({
  label,
  value,
  options,
  onChange,
}: FilterSelectProps<T>) {
  return (
    <div className="flex items-center justify-between gap-2">
      <span className="text-sm font-medium" style={{ color: "var(--color-text-secondary)" }}>
        {label}
      </span>
      <CustomSelect
        value={value}
        options={options}
        onChange={onChange}
        ariaLabel={`${label} 필터`}
        compact={false}
      />
    </div>
  );
}
