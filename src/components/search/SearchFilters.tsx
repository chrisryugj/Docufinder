import type {
  SearchFilters as FiltersType,
  SortOption,
  FileTypeFilter,
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

interface SearchFiltersProps {
  filters: FiltersType;
  onFiltersChange: (filters: FiltersType) => void;
  resultCount?: number;
  viewMode?: ViewMode;
  onViewModeChange?: (mode: ViewMode) => void;
  searchMode?: SearchMode;
}

/**
 * 검색 필터/정렬 바
 */
export function SearchFilters({
  filters,
  onFiltersChange,
  resultCount,
  viewMode = "flat",
  onViewModeChange,
  searchMode,
}: SearchFiltersProps) {
  const handleSortChange = (sortBy: SortOption) => {
    onFiltersChange({ ...filters, sortBy });
  };

  const handleFileTypeChange = (fileType: FileTypeFilter) => {
    onFiltersChange({ ...filters, fileType });
  };

  const handleDateRangeChange = (dateRange: DateRangeFilter) => {
    onFiltersChange({ ...filters, dateRange });
  };

  const handleReset = () => {
    onFiltersChange(DEFAULT_FILTERS);
  };

  const hasActiveFilters =
    filters.sortBy !== "relevance" ||
    filters.fileType !== "all" ||
    filters.dateRange !== "all" ||
    filters.keywordOnly ||
    filters.filenameOnly;

  const showKeywordOnlyToggle = searchMode === "hybrid";
  // 파일명 모드가 아닐 때만 "파일명만" 필터 표시
  const showFilenameOnlyToggle = searchMode !== "filename";

  return (
    <div
      className="flex flex-wrap items-center gap-3 py-3 text-sm"
      role="toolbar"
      aria-label="검색 필터"
    >
      {/* 정렬 */}
      <FilterDropdown
        label="정렬"
        value={filters.sortBy}
        options={SORT_OPTIONS}
        onChange={handleSortChange}
      />

      {/* 파일 타입 */}
      <FilterDropdown
        label="파일"
        value={filters.fileType}
        options={FILE_TYPE_OPTIONS}
        onChange={handleFileTypeChange}
      />

      {/* 날짜 범위 */}
      <FilterDropdown
        label="날짜"
        value={filters.dateRange}
        options={DATE_RANGE_OPTIONS}
        onChange={handleDateRangeChange}
      />

      {showKeywordOnlyToggle && (
        <label
          className="flex items-center gap-2 px-3 py-1.5 rounded-md border cursor-pointer transition-colors"
          style={{
            borderColor: filters.keywordOnly ? "var(--color-accent)" : "var(--color-border)",
            backgroundColor: filters.keywordOnly ? "var(--color-accent-light)" : "var(--color-bg-secondary)",
            color: filters.keywordOnly ? "var(--color-accent)" : "var(--color-text-muted)",
          }}
        >
          <input
            type="checkbox"
            checked={filters.keywordOnly}
            onChange={(e) => onFiltersChange({ ...filters, keywordOnly: e.target.checked })}
            className="accent-blue-500"
            aria-label="키워드 포함 결과만 보기"
          />
          키워드 포함만
        </label>
      )}

      {showFilenameOnlyToggle && (
        <label
          className="flex items-center gap-2 px-3 py-1.5 rounded-md border cursor-pointer transition-colors"
          style={{
            borderColor: filters.filenameOnly ? "var(--color-accent)" : "var(--color-border)",
            backgroundColor: filters.filenameOnly ? "var(--color-accent-light)" : "var(--color-bg-secondary)",
            color: filters.filenameOnly ? "var(--color-accent)" : "var(--color-text-muted)",
          }}
        >
          <input
            type="checkbox"
            checked={filters.filenameOnly}
            onChange={(e) => onFiltersChange({ ...filters, filenameOnly: e.target.checked })}
            className="accent-blue-500"
            aria-label="파일명 매치만 보기"
          />
          파일명만
        </label>
      )}

      {/* 초기화 버튼 */}
      {hasActiveFilters && (
        <button
          onClick={handleReset}
          className="px-3 py-1.5 transition-colors border border-transparent rounded-md font-medium"
          style={{
            color: "var(--color-text-muted)",
          }}
          onMouseEnter={(e) => {
            e.currentTarget.style.color = "var(--color-error)";
            e.currentTarget.style.backgroundColor = "rgba(239, 68, 68, 0.1)";
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.color = "var(--color-text-muted)";
            e.currentTarget.style.backgroundColor = "transparent";
          }}
          aria-label="필터 초기화"
        >
          초기화
        </button>
      )}

      {/* 뷰 모드 토글 */}
      {onViewModeChange && (
        <div className="flex items-center gap-0.5 ml-auto border rounded-md p-0.5" style={{ backgroundColor: "var(--color-bg-tertiary)", borderColor: "var(--color-border)" }}>
          <button
            onClick={() => onViewModeChange("flat")}
            className="p-1.5 rounded-sm transition-colors"
            style={{
              backgroundColor: viewMode === "flat" ? "var(--color-bg-secondary)" : "transparent",
              color: viewMode === "flat" ? "var(--color-accent)" : "var(--color-text-muted)",
              boxShadow: viewMode === "flat" ? "0 1px 2px rgba(0,0,0,0.05)" : "none",
            }}
            title="목록 보기"
            aria-label="목록 보기"
            aria-pressed={viewMode === "flat"}
          >
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 6h16M4 12h16M4 18h16" />
            </svg>
          </button>
          <button
            onClick={() => onViewModeChange("grouped")}
            className="p-1.5 rounded-sm transition-colors"
            style={{
              backgroundColor: viewMode === "grouped" ? "var(--color-bg-secondary)" : "transparent",
              color: viewMode === "grouped" ? "var(--color-accent)" : "var(--color-text-muted)",
              boxShadow: viewMode === "grouped" ? "0 1px 2px rgba(0,0,0,0.05)" : "none",
            }}
            title="파일별 그룹 보기"
            aria-label="파일별 그룹 보기"
            aria-pressed={viewMode === "grouped"}
          >
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
            </svg>
          </button>
        </div>
      )}

      {/* 결과 수 */}
      {resultCount !== undefined && resultCount > 0 && (
        <span className="font-medium" style={{ color: "var(--color-text-secondary)" }}>
          {resultCount}개 결과
        </span>
      )}
    </div>
  );
}

// 드롭다운 컴포넌트
interface FilterDropdownProps<T extends string> {
  label: string;
  value: T;
  options: { value: T; label: string }[];
  onChange: (value: T) => void;
}

function FilterDropdown<T extends string>({
  label,
  value,
  options,
  onChange,
}: FilterDropdownProps<T>) {
  const isDefault = value === options[0].value;

  return (
    <div className="relative inline-block">
      <select
        value={value}
        onChange={(e) => onChange(e.target.value as T)}
        className="appearance-none pl-3 pr-8 py-1.5 rounded-md border cursor-pointer font-medium
          transition-colors focus:outline-none focus:ring-1 focus:ring-offset-0"
        style={{
          backgroundColor: isDefault ? "var(--color-bg-secondary)" : "var(--color-accent-light)",
          borderColor: isDefault ? "var(--color-border)" : "var(--color-accent)",
          color: isDefault ? "var(--color-text-secondary)" : "var(--color-accent)",
          fontSize: "0.875rem",
        }}
        aria-label={`${label} 필터`}
      >
        {options.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
      {/* 드롭다운 아이콘 */}
      <svg
        className="absolute right-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 pointer-events-none"
        style={{ color: isDefault ? "var(--color-text-muted)" : "var(--color-accent)" }}
        fill="none"
        stroke="currentColor"
        viewBox="0 0 24 24"
      >
        <path
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth={2}
          d="M19 9l-7 7-7-7"
        />
      </svg>
    </div>
  );
}
