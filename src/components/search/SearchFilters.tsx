import { memo, useState, useRef, useEffect } from "react";
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
import { CustomSelect } from "../ui/CustomSelect";
import type { FilterPreset } from "../../hooks/useFilterPresets";

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
  watchedFolders?: string[];
  /** 필터 프리셋 */
  presets?: FilterPreset[];
  onSavePreset?: (name: string) => void;
  onApplyPreset?: (preset: FilterPreset) => void;
  onRemovePreset?: (id: string) => void;
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
  presets = [],
  onSavePreset,
  onApplyPreset,
  onRemovePreset,
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
      <InlineFilterDropdown
        label="정렬"
        value={filters.sortBy}
        options={SORT_OPTIONS}
        onChange={handleSortChange}
      />

      {/* 파일 타입 */}
      <InlineFilterDropdown
        label="파일"
        value={filters.fileType}
        options={FILE_TYPE_OPTIONS}
        onChange={handleFileTypeChange}
      />

      {/* 날짜 범위 */}
      <InlineFilterDropdown
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
            className="pl-6 pr-6 py-0.5 rounded border transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-[var(--color-accent)] focus-visible:ring-offset-1"
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

      {/* 프리셋 */}
      {onSavePreset && onApplyPreset && (
        <PresetDropdown
          presets={presets}
          hasActiveFilters={hasActiveFilters}
          onSave={onSavePreset}
          onApply={onApplyPreset}
          onRemove={onRemovePreset}
        />
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
  const scopeOptions = [
    { value: "__all__" as const, label: "전체" },
    ...folders.map((folder) => ({
      value: folder,
      label: getFolderLabel(folder),
    })),
  ];

  return (
    <CustomSelect
      value={value ?? "__all__"}
      options={scopeOptions}
      onChange={(v) => onChange(v === "__all__" ? null : v)}
      ariaLabel="검색 범위 필터"
      isActive={value !== null}
    />
  );
}

function InlineFilterDropdown<T extends string>({
  label,
  value,
  options,
  onChange,
}: FilterDropdownProps<T>) {
  return (
    <CustomSelect
      value={value}
      options={options}
      onChange={onChange}
      ariaLabel={`${label} 필터`}
    />
  );
}

/** 프리셋 저장/불러오기 드롭다운 */
function PresetDropdown({
  presets,
  hasActiveFilters,
  onSave,
  onApply,
  onRemove,
}: {
  presets: FilterPreset[];
  hasActiveFilters: boolean;
  onSave: (name: string) => void;
  onApply: (preset: FilterPreset) => void;
  onRemove?: (id: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const [saving, setSaving] = useState(false);
  const [name, setName] = useState("");
  const ref = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // 외부 클릭 닫기
  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false);
        setSaving(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open]);

  useEffect(() => {
    if (saving) inputRef.current?.focus();
  }, [saving]);

  const handleSave = () => {
    const trimmed = name.trim();
    if (!trimmed) return;
    onSave(trimmed);
    setName("");
    setSaving(false);
  };

  return (
    <div ref={ref} className="relative">
      <button
        onClick={() => { setOpen(!open); setSaving(false); }}
        className="flex items-center gap-1 px-2 py-0.5 rounded border text-xs transition-colors"
        style={{
          borderColor: "var(--color-border)",
          backgroundColor: "var(--color-bg-secondary)",
          color: "var(--color-text-muted)",
        }}
        title="필터 프리셋"
        aria-label="필터 프리셋"
        aria-expanded={open}
      >
        <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z" />
        </svg>
        프리셋
      </button>

      {open && (
        <div
          className="absolute top-full left-0 mt-1 w-56 rounded-lg border shadow-lg z-50 overflow-hidden"
          style={{
            backgroundColor: "var(--color-bg-primary)",
            borderColor: "var(--color-border)",
          }}
        >
          {presets.length > 0 ? (
            <div className="max-h-48 overflow-y-auto">
              {presets.map((preset) => (
                <div
                  key={preset.id}
                  className="flex items-center gap-1 px-3 py-2 hover:bg-[var(--color-bg-tertiary)] cursor-pointer group"
                  onClick={() => { onApply(preset); setOpen(false); }}
                >
                  <span className="flex-1 text-xs text-[var(--color-text-primary)] truncate">
                    {preset.name}
                  </span>
                  <span className="text-[10px] text-[var(--color-text-muted)]">
                    {describePreset(preset)}
                  </span>
                  {onRemove && (
                    <button
                      onClick={(e) => { e.stopPropagation(); onRemove(preset.id); }}
                      className="opacity-0 group-hover:opacity-100 p-0.5 rounded hover:bg-[var(--color-bg-secondary)] text-[var(--color-text-muted)] transition-opacity"
                      title="삭제"
                      aria-label={`프리셋 "${preset.name}" 삭제`}
                    >
                      <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                      </svg>
                    </button>
                  )}
                </div>
              ))}
            </div>
          ) : (
            <div className="px-3 py-3 text-xs text-center text-[var(--color-text-muted)]">
              저장된 프리셋이 없습니다
            </div>
          )}

          <div className="border-t" style={{ borderColor: "var(--color-border)" }}>
            {saving ? (
              <div className="flex items-center gap-1 p-2">
                <input
                  ref={inputRef}
                  type="text"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  onKeyDown={(e) => { if (e.key === "Enter") handleSave(); if (e.key === "Escape") setSaving(false); }}
                  placeholder="프리셋 이름..."
                  maxLength={30}
                  className="flex-1 px-2 py-1 text-xs rounded border focus:outline-none focus-visible:ring-1 focus-visible:ring-[var(--color-accent)]"
                  style={{
                    backgroundColor: "var(--color-bg-secondary)",
                    borderColor: "var(--color-border)",
                    color: "var(--color-text-primary)",
                  }}
                />
                <button
                  onClick={handleSave}
                  disabled={!name.trim()}
                  className="px-2 py-1 text-xs rounded font-medium text-white disabled:opacity-40"
                  style={{ backgroundColor: "var(--color-accent)" }}
                >
                  저장
                </button>
              </div>
            ) : (
              <button
                onClick={() => setSaving(true)}
                disabled={!hasActiveFilters}
                className="w-full px-3 py-2 text-xs text-left transition-colors disabled:opacity-40"
                style={{ color: "var(--color-accent)" }}
              >
                + 현재 필터를 프리셋으로 저장
              </button>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

/** 프리셋 설명 텍스트 (간략) */
function describePreset(preset: FilterPreset): string {
  const parts: string[] = [];
  if (preset.filters.fileType !== "all") {
    const map: Record<string, string> = { hwpx: "한글", docx: "워드", pptx: "PPT", xlsx: "엑셀", pdf: "PDF", txt: "TXT" };
    parts.push(map[preset.filters.fileType] || preset.filters.fileType);
  }
  if (preset.filters.dateRange !== "all") {
    const map: Record<string, string> = { today: "오늘", week: "7일", month: "30일" };
    parts.push(map[preset.filters.dateRange] || preset.filters.dateRange);
  }
  if (preset.filters.sortBy !== "relevance") {
    const map: Record<string, string> = { confidence: "신뢰도", date_desc: "최신", date_asc: "오래된", name: "이름", size: "크기" };
    parts.push(map[preset.filters.sortBy] || preset.filters.sortBy);
  }
  return parts.length > 0 ? parts.join(" · ") : "기본";
}
