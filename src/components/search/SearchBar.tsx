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
  /** 자연어/질문 실행 */
  onSubmitNatural?: () => void;
}

// ── 아이콘 ──────────────────────────────────────────────

const SparkleIcon = () => (
  <svg width="11" height="11" viewBox="0 0 24 24" fill="currentColor" stroke="none">
    <path d="M12 2l2.4 6.4L21 11l-6.6 2.4L12 21l-2.4-7.6L3 11l6.6-2.4L12 2z" />
  </svg>
);

const SendIcon = () => (
  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
    <line x1="22" y1="2" x2="11" y2="13" />
    <polygon points="22 2 15 22 11 13 2 9 22 2" />
  </svg>
);

// ── SearchBar ────────────────────────────────────────────

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
    const isQuestion = paradigm === "question";
    const needsEnterToSubmit = isNatural || isQuestion;

    // 일반 검색 input 훅
    const { innerRef, imeHandlers } = useSearchInput({
      query,
      onQueryChange,
      onCompositionStart,
      onCompositionEnd,
      forwardedRef: ref,
    });

    // ── 질문 모드: textarea 전용 ──────────────────────────
    const textareaRef = useRef<HTMLTextAreaElement>(null);

    // query → textarea 동기화 (외부에서 query 변경 시)
    useEffect(() => {
      if (!isQuestion) return;
      const ta = textareaRef.current;
      if (!ta || ta.value === query) return;
      ta.value = query;
      ta.style.height = "auto";
      ta.style.height = `${Math.min(ta.scrollHeight, 96)}px`;
    }, [query, isQuestion]);

    // 질문 모드일 때 forwardedRef → textareaRef (Ctrl+K 포커스 등)
    useEffect(() => {
      if (!isQuestion || !ref) return;
      const ta = textareaRef.current as unknown as HTMLInputElement;
      if (typeof ref === "function") ref(ta);
      else ref.current = ta;
    }, [isQuestion, ref]);

    const handleTextareaChange = useCallback((e: React.ChangeEvent<HTMLTextAreaElement>) => {
      const ta = e.currentTarget;
      ta.style.height = "auto";
      ta.style.height = `${Math.min(ta.scrollHeight, 96)}px`;
      onQueryChange(ta.value);
    }, [onQueryChange]);

    const handleTextareaKeyDown = useCallback((e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
        e.preventDefault();
        onSubmitNatural?.();
      }
    }, [onSubmitNatural]);

    // ── 일반 모드: 기존 keydown ───────────────────────────
    const handleKeyDown = useCallback(
      (e: React.KeyboardEvent<HTMLInputElement>) => {
        if (needsEnterToSubmit && e.key === "Enter" && !e.nativeEvent.isComposing) {
          e.preventDefault();
          onSubmitNatural?.();
          return;
        }
        if (!needsEnterToSubmit && onSuggestionsKeyDown) {
          const selected = onSuggestionsKeyDown(e);
          if (selected !== null) {
            onSuggestionSelect?.(selected);
            return;
          }
        }
      },
      [needsEnterToSubmit, onSubmitNatural, onSuggestionsKeyDown, onSuggestionSelect]
    );

    const blurTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
    useEffect(() => () => { if (blurTimerRef.current) clearTimeout(blurTimerRef.current); }, []);
    const handleBlur = useCallback(() => {
      blurTimerRef.current = setTimeout(() => { onSuggestionsClose?.(); blurTimerRef.current = null; }, 150);
    }, [onSuggestionsClose]);

    return (
      <div className="w-full relative">
        {/* Paradigm Toggle */}
        {onParadigmChange && (
          <div className="mb-1">
            <SearchParadigmToggle paradigm={paradigm} onChange={onParadigmChange} />
          </div>
        )}

        {isQuestion ? (
          /* ── AI 질문 모드 ── */
          <div
            className="flex items-start px-3 py-2.5 rounded-lg transition-all duration-200 focus-within:ring-2 focus-within:ring-[var(--color-accent)] focus-within:ring-offset-1"
            style={{
              backgroundColor: "var(--color-bg-secondary)",
              border: "1px solid var(--color-accent)",
              boxShadow: "var(--shadow-sm), 0 0 0 3px var(--color-accent-subtle)",
            }}
          >
            {/* AI 배지 */}
            <div
              className="flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-semibold shrink-0 mt-0.5 mr-2.5 select-none"
              style={{
                backgroundColor: "var(--color-accent-light)",
                color: "var(--color-accent)",
              }}
            >
              <SparkleIcon />
              AI
            </div>

            {/* Textarea */}
            <textarea
              ref={textareaRef}
              defaultValue={query}
              onChange={handleTextareaChange}
              onKeyDown={handleTextareaKeyDown}
              onCompositionStart={onCompositionStart}
              onCompositionEnd={(e) => onCompositionEnd?.(e.currentTarget.value)}
              rows={1}
              placeholder="문서에 대해 질문하세요... (Shift+Enter로 줄바꿈)"
              className="flex-1 bg-transparent border-none focus:outline-none resize-none overflow-hidden leading-relaxed"
              style={{
                color: "var(--color-text-primary)",
                fontSize: "var(--text-sm)",
                fontWeight: 500,
                minHeight: "22px",
                maxHeight: "96px",
              }}
            />

            {/* 로딩 스피너 */}
            {isLoading && (
              <div
                className="w-4 h-4 rounded-full animate-spin shrink-0 ml-2 mt-0.5"
                style={{
                  border: "1.5px solid var(--color-border)",
                  borderTopColor: "var(--color-accent)",
                }}
              />
            )}

            {/* 전송 버튼 */}
            {query.trim() && !isLoading && (
              <button
                onClick={onSubmitNatural}
                className="shrink-0 ml-2 mt-0 p-1.5 rounded-md transition-all duration-150 hover:opacity-90 active:scale-95"
                style={{
                  backgroundColor: "var(--color-accent)",
                  color: "white",
                }}
                title="질문 전송 (Enter)"
              >
                <SendIcon />
              </button>
            )}
          </div>
        ) : (
          /* ── 일반 검색 모드 ── */
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

            {/* Search Mode Dropdown */}
            <SearchModeDropdown
              searchMode={searchMode}
              onSearchModeChange={onSearchModeChange}
              status={status}
            />
          </div>
        )}

        {/* 모드 힌트 (자연어/질문 모드 전환 시) */}
        {needsEnterToSubmit && !query && (
          <p
            className="mt-1.5 text-[11px] leading-relaxed px-1"
            style={{ color: "var(--color-text-muted)" }}
          >
            {isQuestion
              ? "문서를 기반으로 AI가 답변합니다. 질문을 입력하고 Enter를 누르세요."
              : "날짜, 파일 타입, 제외 조건 등을 자유롭게 입력하세요. 예: \"최근 30일 계약서 PDF만\""
            }
          </p>
        )}

        {/* 자동완성 드롭다운 (즉시 모드만) */}
        {!isNatural && !isQuestion && isSuggestionsOpen && suggestions.length > 0 && (
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
                  e.preventDefault();
                  onSuggestionSelect?.(item.text);
                }}
                role="option"
                aria-selected={index === suggestionsSelectedIndex}
              >
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
                  {item.source === "history" ? `${item.frequency}회` : `${item.frequency}건`}
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
