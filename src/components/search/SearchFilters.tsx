import { memo } from "react";
import type {
  SearchFilters as FiltersType,
  SortOption,
  FileTypeFilter,
  DateRangeFilter,
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
  /** 결과 내 검색 표시 여부 (결과가 있을 때만 표시) */
  showRefineSearch?: boolean;
  searchMode?: SearchMode;
  /** 결과 내 검색 쿼리 */
  refineQuery?: string;
  onRefineQueryChange?: (query: string) => void;
  onRefineQueryClear?: () => void;
  /** 등록된 폴더 목록 (범위 필터용) */
  watchedFolders?: string[];
}

/**
 * 검색 필터/정렬 바
 */
export const SearchFilters = memo(function SearchFilters({
  filters,
  onFiltersChange,
  showRefineSearch = false,
  searchMode,
  refineQuery = "",
  onRefineQueryChange,
  onRefineQueryClear,
  watchedFolders = [],
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
    filters.excludeFilename ||
    filters.searchScope !== null;

  const showKeywordOnlyToggle = searchMode === "hybrid";
  // 파일명 모드가 아닐 때만 "파일명 제외" 필터 표시
  const showExcludeFilenameToggle = searchMode !== "filename";

  return (
    <div
      className="flex flex-wrap items-center gap-1.5 py-1 text-xs"
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

      {/* 검색 범위 */}
      {watchedFolders.length > 1 && (
        <ScopeDropdown
          value={filters.searchScope}
          folders={watchedFolders}
          onChange={(scope) => onFiltersChange({ ...filters, searchScope: scope })}
        />
      )}

      {showKeywordOnlyToggle && (
        <label
          className="flex items-center gap-1 px-2 py-0.5 rounded border cursor-pointer transition-colors"
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
            className="accent-[var(--color-accent)] w-3 h-3"
            aria-label="키워드 포함 결과만 보기"
          />
          키워드 포함만
        </label>
      )}

      {showExcludeFilenameToggle && (
        <label
          className="flex items-center gap-1 px-2 py-0.5 rounded border cursor-pointer transition-colors"
          style={{
            borderColor: filters.excludeFilename ? "var(--color-accent)" : "var(--color-border)",
            backgroundColor: filters.excludeFilename ? "var(--color-accent-light)" : "var(--color-bg-secondary)",
            color: filters.excludeFilename ? "var(--color-accent)" : "var(--color-text-muted)",
          }}
        >
          <input
            type="checkbox"
            checked={filters.excludeFilename}
            onChange={(e) => onFiltersChange({ ...filters, excludeFilename: e.target.checked })}
            className="accent-[var(--color-accent)] w-3 h-3"
            aria-label="파일명 검색 결과 제외"
          />
          파일명 제외
        </label>
      )}

      {/* 결과 내 검색 */}
      {onRefineQueryChange && showRefineSearch && (
        <div className="relative flex items-center">
          <div
            className="absolute left-2 top-1/2 -translate-y-1/2"
            style={{ color: refineQuery ? "var(--color-accent)" : "var(--color-text-muted)" }}
          >
            <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
            </svg>
          </div>
          <input
            type="text"
            value={refineQuery}
            onChange={(e) => onRefineQueryChange(e.target.value)}
            placeholder="결과 내 검색..."
            className="pl-6 pr-6 py-0.5 rounded border transition-colors focus:outline-none"
            style={{
              width: "130px",
              maxWidth: "180px",
              backgroundColor: "var(--color-bg-secondary)",
              borderColor: refineQuery ? "var(--color-accent)" : "var(--color-border)",
              color: "var(--color-text-primary)",
            }}
            aria-label="결과 내 검색"
          />
          {refineQuery && onRefineQueryClear && (
            <button
              onClick={onRefineQueryClear}
              className="absolute right-2 top-1/2 -translate-y-1/2 p-0.5 rounded-full transition-colors hover:bg-[var(--color-bg-tertiary)]"
              style={{ color: "var(--color-text-muted)" }}
              aria-label="결과 내 검색 초기화"
            >
              <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          )}
        </div>
      )}

      {/* 초기화 버튼 */}
      {hasActiveFilters && (
        <button
          onClick={handleReset}
          className="px-1.5 py-0.5 border border-transparent rounded text-xs font-medium btn-reset-hover"
          aria-label="필터 초기화"
        >
          초기화
        </button>
      )}
    </div>
  );
});

// 드롭다운 컴포넌트
interface FilterDropdownProps<T extends string> {
  label: string;
  value: T;
  options: { value: T; label: string }[];
  onChange: (value: T) => void;
}

/** 폴더 경로에서 짧은 라벨 추출 */
function getFolderLabel(path: string): string {
  // 드라이브 루트: "C:\" → "C:"
  const normalized = path.replace(/\\\\\?\\/, "").replace(/\//g, "\\");
  if (/^[A-Za-z]:\\?$/.test(normalized)) {
    return normalized.replace(/\\$/, "");
  }
  // 일반 폴더: 마지막 폴더명
  const parts = normalized.replace(/\\$/, "").split("\\");
  return parts[parts.length - 1] || path;
}

function ScopeDropdown({
  value,
  folders,
  onChange,
}: {
  value: string | null;
  folders: string[];
  onChange: (scope: string | null) => void;
}) {
  const isActive = value !== null;

  return (
    <div className="relative inline-block">
      <select
        value={value ?? "__all__"}
        onChange={(e) => onChange(e.target.value === "__all__" ? null : e.target.value)}
        className="appearance-none pl-2 pr-5 py-0.5 rounded border cursor-pointer font-medium
          transition-colors focus:outline-none"
        style={{
          backgroundColor: isActive ? "var(--color-accent-light)" : "var(--color-bg-secondary)",
          borderColor: isActive ? "var(--color-accent)" : "var(--color-border)",
          color: isActive ? "var(--color-accent)" : "var(--color-text-secondary)",
        }}
        aria-label="검색 범위 필터"
      >
        <option value="__all__">전체</option>
        {folders.map((folder) => (
          <option key={folder} value={folder}>
            {getFolderLabel(folder)}
          </option>
        ))}
      </select>
      <svg
        className="absolute right-1.5 top-1/2 -translate-y-1/2 w-3 h-3 pointer-events-none"
        style={{ color: isActive ? "var(--color-accent)" : "var(--color-text-muted)" }}
        fill="none"
        stroke="currentColor"
        viewBox="0 0 24 24"
      >
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
      </svg>
    </div>
  );
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
        className="appearance-none pl-2 pr-5 py-0.5 rounded border cursor-pointer font-medium
          transition-colors focus:outline-none"
        style={{
          backgroundColor: isDefault ? "var(--color-bg-secondary)" : "var(--color-accent-light)",
          borderColor: isDefault ? "var(--color-border)" : "var(--color-accent)",
          color: isDefault ? "var(--color-text-secondary)" : "var(--color-accent)",
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
        className="absolute right-1.5 top-1/2 -translate-y-1/2 w-3 h-3 pointer-events-none"
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
