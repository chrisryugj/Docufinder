import { forwardRef, memo } from "react";
import type { SearchMode } from "../../types/search";
import type { IndexStatus } from "../../types/index";
import { useSearchInput } from "../../hooks/useSearchInput";
import { SearchModeDropdown } from "./SearchModeDropdown";

interface SearchBarProps {
  query: string;
  onQueryChange: (query: string) => void;
  searchMode: SearchMode;
  onSearchModeChange: (mode: SearchMode) => void;
  isLoading: boolean;
  status: IndexStatus | null;
  resultCount?: number;
  searchTime?: number | null;
  onCompositionStart?: () => void;
  onCompositionEnd?: (finalValue: string) => void;
}

export const SearchBar = memo(forwardRef<HTMLInputElement, SearchBarProps>(
  (
    {
      query,
      onQueryChange,
      searchMode,
      onSearchModeChange,
      isLoading,
      status,
      resultCount: _resultCount,
      searchTime: _searchTime,
      onCompositionStart,
      onCompositionEnd,
    },
    ref
  ) => {
    const { innerRef, imeHandlers } = useSearchInput({
      query,
      onQueryChange,
      onCompositionStart,
      onCompositionEnd,
      forwardedRef: ref,
    });

    return (
      <div className="max-w-4xl mx-auto w-full">
        <div
          className="flex items-center px-4 py-3.5 rounded-xl transition-all duration-200 focus-within:ring-2"
          style={{
            backgroundColor: "var(--color-bg-secondary)",
            border: "1px solid var(--color-border)",
            boxShadow: "var(--shadow-card)",
            ["--tw-ring-color" as string]: "var(--color-accent-light)",
          }}
        >
          <svg
            className="w-5 h-5 flex-shrink-0"
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
            {...imeHandlers}
            placeholder="검색어 입력..."
            className="flex-1 bg-transparent border-none text-base focus:outline-none ml-3"
            style={{ color: "var(--color-text-primary)" }}
            aria-label="검색어 입력"
          />

          {/* 단축키 힌트 */}
          {!query && (
            <kbd
              className="hidden sm:inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-mono ml-2"
              style={{
                color: "var(--color-text-muted)",
                backgroundColor: "var(--color-bg-tertiary)",
                border: "1px solid var(--color-border)",
              }}
            >
              Ctrl+K
            </kbd>
          )}

          {isLoading && (
            <div
              className="w-5 h-5 rounded-full border-2 animate-spin ml-2"
              style={{ borderColor: "var(--color-border)", borderTopColor: "var(--color-accent)" }}
              role="status"
              aria-label="검색 중"
            />
          )}

          <SearchModeDropdown
            searchMode={searchMode}
            onSearchModeChange={onSearchModeChange}
            status={status}
          />
        </div>
      </div>
    );
  }
));

SearchBar.displayName = "SearchBar";
