import { forwardRef } from "react";
import type { SearchMode } from "../../types/search";
import { SEARCH_MODES } from "../../types/search";
import type { IndexStatus } from "../../types/index";

interface SearchBarProps {
  query: string;
  onQueryChange: (query: string) => void;
  searchMode: SearchMode;
  onSearchModeChange: (mode: SearchMode) => void;
  isLoading: boolean;
  status: IndexStatus | null;
  resultCount?: number;
  searchTime?: number | null;
}

export const SearchBar = forwardRef<HTMLInputElement, SearchBarProps>(
  (
    {
      query,
      onQueryChange,
      searchMode,
      onSearchModeChange,
      isLoading,
      status,
      resultCount,
      searchTime,
    },
    ref
  ) => {
    return (
      <div className="max-w-2xl mx-auto">
        {/* 검색 입력 */}
        <div className="relative">
          <input
            ref={ref}
            type="text"
            value={query}
            onChange={(e) => onQueryChange(e.target.value)}
            placeholder="검색어를 입력하세요..."
            className="w-full px-4 py-3.5 pl-12 rounded-xl transition-all duration-200"
            style={{
              backgroundColor: "var(--color-bg-secondary)",
              border: "1px solid var(--color-border)",
              color: "var(--color-text-primary)",
              boxShadow: "var(--shadow-sm)",
            }}
            onFocus={(e) => {
              e.currentTarget.style.borderColor = "var(--color-accent)";
              e.currentTarget.style.boxShadow = "0 0 0 3px var(--color-accent-light)";
            }}
            onBlur={(e) => {
              e.currentTarget.style.borderColor = "var(--color-border)";
              e.currentTarget.style.boxShadow = "var(--shadow-sm)";
            }}
            aria-label="검색어 입력"
          />
          {/* 검색 아이콘 */}
          <svg
            className="absolute left-4 top-1/2 -translate-y-1/2 w-5 h-5"
            style={{ color: "var(--color-text-muted)" }}
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
            aria-hidden="true"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
            />
          </svg>
          {/* 로딩 스피너 */}
          {isLoading && (
            <div className="absolute right-4 top-1/2 -translate-y-1/2">
              <div
                className="w-5 h-5 rounded-full animate-spin"
                style={{
                  border: "2px solid var(--color-border)",
                  borderTopColor: "var(--color-accent)",
                }}
                role="status"
                aria-label="검색 중"
              />
            </div>
          )}
        </div>

        {/* 검색 모드 선택 */}
        <div className="flex gap-2 mt-3">
          {SEARCH_MODES.map((mode) => {
            const needsSemantic = mode.value !== "keyword";
            const disabled = needsSemantic && !status?.semantic_available;
            const isActive = searchMode === mode.value;

            return (
              <button
                key={mode.value}
                onClick={() => !disabled && onSearchModeChange(mode.value)}
                disabled={disabled}
                className="px-3.5 py-1.5 text-sm rounded-lg transition-all duration-200"
                style={{
                  backgroundColor: isActive
                    ? "var(--color-accent)"
                    : "var(--color-bg-secondary)",
                  color: isActive
                    ? "white"
                    : disabled
                      ? "var(--color-text-muted)"
                      : "var(--color-text-secondary)",
                  opacity: disabled ? 0.5 : 1,
                  cursor: disabled ? "not-allowed" : "pointer",
                  fontWeight: isActive ? 500 : 400,
                }}
                onMouseEnter={(e) => {
                  if (!disabled && !isActive) {
                    e.currentTarget.style.backgroundColor = "var(--color-bg-tertiary)";
                  }
                }}
                onMouseLeave={(e) => {
                  if (!disabled && !isActive) {
                    e.currentTarget.style.backgroundColor = "var(--color-bg-secondary)";
                  }
                }}
                title={disabled ? "모델 파일 필요" : mode.desc}
                aria-pressed={isActive}
              >
                {mode.label}
              </button>
            );
          })}

          {/* 검색 결과 카운트 */}
          {searchTime !== null && resultCount !== undefined && resultCount > 0 && (
            <span
              className="ml-auto text-sm self-center"
              style={{ color: "var(--color-text-muted)" }}
            >
              {resultCount}개 결과 ({searchTime}ms)
            </span>
          )}
        </div>
      </div>
    );
  }
);

SearchBar.displayName = "SearchBar";
