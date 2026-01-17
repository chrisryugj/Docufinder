import type {
  SearchFilters as FiltersType,
  SortOption,
  FileTypeFilter,
  DateRangeFilter,
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
}

/**
 * 검색 필터/정렬 바
 */
export function SearchFilters({
  filters,
  onFiltersChange,
  resultCount,
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
    filters.dateRange !== "all";

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

      {/* 초기화 버튼 */}
      {hasActiveFilters && (
        <button
          onClick={handleReset}
          className="px-2 py-1 transition-colors"
          style={{ color: "var(--color-text-muted)" }}
          onMouseEnter={(e) => (e.currentTarget.style.color = "var(--color-text-primary)")}
          onMouseLeave={(e) => (e.currentTarget.style.color = "var(--color-text-muted)")}
          aria-label="필터 초기화"
        >
          초기화
        </button>
      )}

      {/* 결과 수 */}
      {resultCount !== undefined && resultCount > 0 && (
        <span className="ml-auto" style={{ color: "var(--color-text-muted)" }}>
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
        className="appearance-none pl-3 pr-7 py-1.5 rounded-md border cursor-pointer
          transition-colors focus:outline-none focus:ring-2"
        style={{
          backgroundColor: isDefault ? "var(--color-bg-secondary)" : "var(--color-accent-light)",
          borderColor: isDefault ? "var(--color-border)" : "var(--color-accent)",
          color: isDefault ? "var(--color-text-muted)" : "var(--color-accent)",
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
        className="absolute right-2 top-1/2 -translate-y-1/2 w-3.5 h-3.5 pointer-events-none"
        style={{ color: "var(--color-text-muted)" }}
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
