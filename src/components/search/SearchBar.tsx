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
            className="w-full px-4 py-3 pl-12 bg-gray-800 border border-gray-700 rounded-lg
                       focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent
                       text-white placeholder-gray-500"
            aria-label="검색어 입력"
          />
          {/* 검색 아이콘 */}
          <svg
            className="absolute left-4 top-1/2 -translate-y-1/2 w-5 h-5 text-gray-500"
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
                className="w-5 h-5 border-2 border-gray-500 border-t-blue-500 rounded-full animate-spin"
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
            return (
              <button
                key={mode.value}
                onClick={() => !disabled && onSearchModeChange(mode.value)}
                disabled={disabled}
                className={`px-3 py-1.5 text-sm rounded-lg transition-colors ${
                  searchMode === mode.value
                    ? "bg-blue-600 text-white"
                    : disabled
                      ? "bg-gray-800 text-gray-600 cursor-not-allowed"
                      : "bg-gray-800 text-gray-400 hover:bg-gray-700"
                }`}
                title={disabled ? "모델 파일 필요" : mode.desc}
                aria-pressed={searchMode === mode.value}
              >
                {mode.label}
              </button>
            );
          })}

          {/* 검색 결과 카운트 */}
          {searchTime !== null && resultCount !== undefined && resultCount > 0 && (
            <span className="ml-auto text-gray-500 text-sm self-center">
              {resultCount}개 결과 ({searchTime}ms)
            </span>
          )}
        </div>
      </div>
    );
  }
);

SearchBar.displayName = "SearchBar";
