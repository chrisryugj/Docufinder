import { forwardRef, memo, useCallback, useRef, useEffect } from "react";
import type { SearchMode, SearchParadigm, SuggestionItem } from "../../types/search";
import type { IndexStatus } from "../../types/index";
import { useSearchInput } from "../../hooks/useSearchInput";
import { SearchModeDropdown } from "./SearchModeDropdown";
import SearchParadigmToggle from "./SearchParadigmToggle";

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
  /** 자동완성 */
  suggestions?: SuggestionItem[];
  isSuggestionsOpen?: boolean;
  suggestionsSelectedIndex?: number;
  onSuggestionSelect?: (text: string) => void;
  onSuggestionsKeyDown?: (e: React.KeyboardEvent) => string | null;
  onSuggestionsClose?: () => void;
  onSuggestionsSetIndex?: (index: number) => void;
  /** 검색 패러다임 */
  paradigm?: SearchParadigm;
  onParadigmChange?: (p: SearchParadigm) => void;
  /** 자연어 검색 실행 */
  onSubmitNatural?: () => void;
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
      suggestions = [],
      isSuggestionsOpen = false,
      suggestionsSelectedIndex = -1,
      onSuggestionSelect,
      onSuggestionsKeyDown,
      onSuggestionsClose,
      onSuggestionsSetIndex,
      paradigm = "instant",
      onParadigmChange,
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
        // 자연어 모드: Enter로 검색 실행
        if (isNatural && e.key === "Enter" && !e.nativeEvent.isComposing) {
          e.preventDefault();
          onSubmitNatural?.();
          return;
        }

        // 자동완성 키보드 처리 우선 (즉시 모드만)
        if (!isNatural && onSuggestionsKeyDown) {
          const selected = onSuggestionsKeyDown(e);
          if (selected !== null) {
            onSuggestionSelect?.(selected);
            return;
          }
        }
      },
      [isNatural, onSubmitNatural, onSuggestionsKeyDown, onSuggestionSelect]
    );

    const blurTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
    useEffect(() => () => { if (blurTimerRef.current) clearTimeout(blurTimerRef.current); }, []);
    const handleBlur = useCallback(() => {
      // 드롭다운 클릭이 blur보다 먼저 처리되도록 지연
      blurTimerRef.current = setTimeout(() => { onSuggestionsClose?.(); blurTimerRef.current = null; }, 150);
    }, [onSuggestionsClose]);

    return (
      <div className="w-full relative">
        {/* Paradigm Toggle (탭 헤더) */}
        {onParadigmChange && (
          <div className="mb-1">
            <SearchParadigmToggle paradigm={paradigm} onChange={onParadigmChange} />
          </div>
        )}

        <div
          className="group/search flex items-center px-4 py-3 rounded-lg transition-all duration-200 focus-within:ring-2 focus-within:ring-[var(--color-accent)] focus-within:ring-offset-1"
          style={{
            backgroundColor: "var(--color-bg-secondary)",
            border: `1px solid ${isNatural ? "var(--color-accent)" : "var(--color-border)"}`,
            boxShadow: isNatural
              ? "var(--shadow-sm), 0 0 0 3px var(--color-accent-subtle)"
              : "var(--shadow-sm)",
          }}
        >
          {/* Search Icon */}
          <svg
            className="w-4.5 h-4.5 flex-shrink-0"
            fill="none"
            stroke="currentColor"
            strokeWidth={2}
            viewBox="0 0 24 24"
            style={{ color: "var(--color-text-muted)", width: "18px", height: "18px" }}
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
            />
          </svg>

          {/* Input */}
          <input
            ref={innerRef}
            type="text"
            defaultValue={query}
            {...imeHandlers}
            onKeyDown={handleKeyDown}
            onBlur={handleBlur}
            placeholder={isNatural
              ? "작년 예산 한글 문서, 최근 30일 계약서 PDF만"
              : "예산 집행현황, 계약서, 인사발령"
            }
            className="flex-1 bg-transparent border-none focus:outline-none ml-3"
            style={{
              color: "var(--color-text-primary)",
              fontSize: "var(--text-sm)",
              fontWeight: 500,
              letterSpacing: "0.01em",
            }}
            aria-label="검색어 입력"
            autoComplete="off"
            role="combobox"
            aria-expanded={isSuggestionsOpen}
            aria-autocomplete="list"
            aria-owns={isSuggestionsOpen ? "suggestion-listbox" : undefined}
            aria-activedescendant={
              isSuggestionsOpen && suggestionsSelectedIndex >= 0
                ? `suggestion-${suggestionsSelectedIndex}`
                : undefined
            }
          />

          {/* Shortcut Hint */}
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

          {/* Loading Spinner */}
          {isLoading && (
            <div
              className="w-4 h-4 rounded-full animate-spin ml-2"
              style={{
                border: "1.5px solid var(--color-border)",
                borderTopColor: "var(--color-accent)",
              }}
              role="status"
              aria-label="검색 중"
            />
          )}

          {/* Enter 힌트 (자연어 모드) */}
          {isNatural && query && (
            <kbd
              className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-mono ml-2"
              style={{
                color: "var(--color-text-muted)",
                backgroundColor: "var(--color-bg-tertiary)",
                border: "1px solid var(--color-border)",
              }}
            >
              Enter
            </kbd>
          )}

          {/* Search Mode (즉시 모드만) */}
          {!isNatural && (
            <SearchModeDropdown
              searchMode={searchMode}
              onSearchModeChange={onSearchModeChange}
              status={status}
            />
          )}
        </div>

        {/* 자동완성 드롭다운 (즉시 모드만) */}
        {!isNatural && isSuggestionsOpen && suggestions.length > 0 && (
          <div
            id="suggestion-listbox"
            className="absolute left-0 right-0 mt-1 rounded-lg overflow-hidden z-50 shadow-lg"
            style={{
              backgroundColor: "var(--color-bg-secondary)",
              border: "1px solid var(--color-border)",
            }}
            role="listbox"
          >
            {suggestions.map((item, index) => (
              <button
                id={`suggestion-${index}`}
                key={`${item.source}-${item.text}`}
                className="w-full flex items-center gap-2 px-4 py-2 text-left transition-colors"
                style={{
                  backgroundColor:
                    index === suggestionsSelectedIndex
                      ? "var(--color-bg-tertiary)"
                      : "transparent",
                  color: "var(--color-text-primary)",
                }}
                onMouseEnter={() => onSuggestionsSetIndex?.(index)}
                onMouseDown={(e) => {
                  e.preventDefault(); // blur 방지
                  onSuggestionSelect?.(item.text);
                }}
                role="option"
                aria-selected={index === suggestionsSelectedIndex}
              >
                {/* 아이콘: 히스토리 vs vocab */}
                <span
                  className="flex-shrink-0 w-4 h-4 flex items-center justify-center"
                  style={{ color: "var(--color-text-muted)" }}
                >
                  {item.source === "history" ? (
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <circle cx="12" cy="12" r="10" />
                      <polyline points="12 6 12 12 16 14" />
                    </svg>
                  ) : (
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <path d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
                    </svg>
                  )}
                </span>
                <span className="flex-1 truncate text-sm">{item.text}</span>
                <span
                  className="text-[10px] tabular-nums flex-shrink-0"
                  style={{ color: "var(--color-text-muted)" }}
                >
                  {item.source === "history"
                    ? `${item.frequency}회`
                    : `${item.frequency}건`}
                </span>
              </button>
            ))}
          </div>
        )}
      </div>
    );
  }
));

SearchBar.displayName = "SearchBar";
