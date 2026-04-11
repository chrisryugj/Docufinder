import { forwardRef, memo, useCallback, useRef, useEffect, useMemo } from "react";
import type { SearchMode, SearchParadigm, SuggestionItem } from "../../types/search";
import type { IndexStatus } from "../../types/index";
import { useSearchInput } from "../../hooks/useSearchInput";
import { SearchModeDropdown } from "./SearchModeDropdown";
import SearchParadigmToggle from "./SearchParadigmToggle";
import { ScopeChip } from "./ScopeChip";
import { parseSmartPreview } from "../../utils/parseSmartPreview";

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
  /** AI 검색 범위 */
  watchedFolders?: string[];
  searchScope?: string | null;
  onSearchScopeChange?: (scope: string | null) => void;
}

// ── 아이콘 ──────────────────────────────────────────────

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
      watchedFolders = [],
      searchScope,
      onSearchScopeChange,
    },
    ref
  ) => {
    const isNatural = paradigm === "natural";
    const isQuestion = paradigm === "question";
    const needsEnterToSubmit = isNatural || isQuestion;

    // 스마트 모드 실시간 파싱 미리보기
    const smartPreview = useMemo(
      () => (isNatural ? parseSmartPreview(query) : null),
      [isNatural, query]
    );

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
      // 1000자 제한 (백엔드 MAX_QUERY_LEN과 일치)
      if (ta.value.length > 1000) {
        ta.value = ta.value.slice(0, 1000);
      }
      ta.style.height = "auto";
      ta.style.height = `${Math.min(ta.scrollHeight, 160)}px`;
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

        {/* ── 검색바 (3모드 통일 레이아웃) ── */}
        <div className="group/search">
        <div
          className="flex items-center px-3 rounded-lg transition-all duration-200 focus-within:ring-2 focus-within:ring-[var(--color-accent)] focus-within:ring-offset-1"
          style={{
            backgroundColor: "var(--color-bg-secondary)",
            border: `1px solid ${needsEnterToSubmit ? "var(--color-accent)" : "var(--color-border)"}`,
            boxShadow: needsEnterToSubmit
              ? "var(--shadow-sm), 0 0 0 3px var(--color-accent-subtle)"
              : "var(--shadow-sm)",
            minHeight: "44px",
          }}
        >
          {/* 모드별 아이콘 */}
          <svg
            className="flex-shrink-0 mr-2"
            fill={isQuestion ? "currentColor" : "none"}
            stroke={isQuestion ? "none" : "currentColor"}
            strokeWidth={2}
            viewBox="0 0 24 24"
            style={{
              color: needsEnterToSubmit ? "var(--color-accent)" : "var(--color-text-muted)",
              width: "16px",
              height: "16px",
            }}
          >
            {isQuestion ? (
              <path d="M12 2l2.4 6.4L21 11l-6.6 2.4L12 21l-2.4-7.6L3 11l6.6-2.4L12 2z" />
            ) : isNatural ? (
              <path strokeLinecap="round" strokeLinejoin="round" d="M15 4V2M15 16v-2M8 9h2M20 9h2M17.8 11.8l1.4 1.4M17.8 6.2l1.4-1.4M12.2 6.2l-1.4-1.4M3 21l9-9" />
            ) : (
              <path strokeLinecap="round" strokeLinejoin="round" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
            )}
          </svg>

          {/* AI 스코프 칩 (Anything 모드) */}
          {isQuestion && onSearchScopeChange && (
            <ScopeChip
              watchedFolders={watchedFolders}
              searchScope={searchScope ?? null}
              onSearchScopeChange={onSearchScopeChange}
            />
          )}

          {/* 입력 필드 */}
          {isQuestion ? (
            <textarea
              ref={textareaRef}
              defaultValue={query}
              onChange={handleTextareaChange}
              onKeyDown={handleTextareaKeyDown}
              onCompositionStart={onCompositionStart}
              onCompositionEnd={(e) => onCompositionEnd?.(e.currentTarget.value)}
              rows={1}
              placeholder="문서에 대해 무엇이든 물어보세요..."
              className="flex-1 bg-transparent border-none focus:outline-none resize-none overflow-hidden py-2.5"
              style={{
                color: "var(--color-text-primary)",
                fontSize: "var(--text-sm)",
                fontWeight: 500,
                lineHeight: "1.5",
                minHeight: "24px",
                maxHeight: "96px",
              }}
            />
          ) : (
            <input
              ref={innerRef}
              type="text"
              defaultValue={query}
              maxLength={1000}
              {...imeHandlers}
              onKeyDown={handleKeyDown}
              onBlur={handleBlur}
              placeholder={isNatural
                ? "자연어로 검색 조건을 입력하세요..."
                : "키워드로 문서 검색..."
              }
              className="flex-1 bg-transparent border-none focus:outline-none h-[24px]"
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
          )}

          {/* Enter 힌트 (스마트 모드) */}
          {needsEnterToSubmit && query && !isQuestion && (
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

          {/* Loading Spinner */}
          {isLoading && (
            <div
              className={`w-4 h-4 rounded-full animate-spin shrink-0 ml-2 ${isQuestion ? "mt-0.5" : ""}`}
              style={{
                border: "1.5px solid var(--color-border)",
                borderTopColor: "var(--color-accent)",
              }}
              role="status"
              aria-label="처리 중"
            />
          )}

          {/* 전송 버튼 (Anything 모드) */}
          {isQuestion && query.trim() && !isLoading && (
            <button
              onClick={onSubmitNatural}
              className="shrink-0 ml-2 mt-0 p-1.5 rounded-md transition-all duration-150 hover:opacity-90 active:scale-95"
              style={{
                backgroundColor: "var(--color-accent)",
                color: "white",
              }}
              title="Anything에게 질문 (Enter)"
            >
              <SendIcon />
            </button>
          )}

          {/* Search Mode Dropdown (검색 모드만) */}
          {!isNatural && !isQuestion && (
            <SearchModeDropdown
              searchMode={searchMode}
              onSearchModeChange={onSearchModeChange}
              status={status}
            />
          )}
        </div>

        {/* 모드 힌트 토스트 — 스마트/Anything 모드, 입력 비어있을 때 호버 시 표시 */}
        {!query && needsEnterToSubmit && (
          <div
            className="overflow-hidden transition-all duration-300 ease-out opacity-0 max-h-0 group-hover/search:opacity-100 group-hover/search:max-h-12 group-hover/search:mt-1.5"
          >
            <div
              className="flex items-center gap-2 px-3 py-1.5 rounded-md text-xs"
              style={{
                color: "var(--color-text-muted)",
              }}
            >
              <span style={{ color: "var(--color-accent)" }}>
                {isQuestion ? "Anything" : "스마트"}
              </span>
              <span
                className="w-px h-3 shrink-0"
                style={{ backgroundColor: "var(--color-border)" }}
              />
              <span>
                {isQuestion
                  ? "문서를 분석하여 답변합니다 · 예: \"연차 사용 조건이 어떻게 되나요?\""
                  : "자연어로 조건을 조합합니다 · 예: \"작년 예산 한글 문서\""
                }
              </span>
            </div>
          </div>
        )}

        {/* 스마트 모드 파싱 미리보기 */}
        {isNatural && smartPreview && (
          <div className="flex items-center gap-2 flex-wrap mt-1.5 px-1">
            {smartPreview.keywords && (
              <span className="text-xs text-[var(--color-text-muted)]">
                {smartPreview.keywords}
              </span>
            )}
            {smartPreview.dateLabel && (
              <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[11px] font-medium bg-[var(--color-accent)]/10 text-[var(--color-accent)] border border-[var(--color-accent)]/20">
                <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><rect x="3" y="4" width="18" height="18" rx="2"/><path d="M16 2v4M8 2v4M3 10h18"/></svg>
                {smartPreview.dateLabel}
              </span>
            )}
            {smartPreview.fileType && (
              <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[11px] font-medium bg-[var(--color-accent)]/10 text-[var(--color-accent)] border border-[var(--color-accent)]/20">
                <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z"/><polyline points="14 2 14 8 20 8"/></svg>
                {smartPreview.fileType}
              </span>
            )}
            {smartPreview.excludeKeywords.map((ex, i) => (
              <span
                key={i}
                className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[11px] font-medium bg-red-500/10 text-red-500 border border-red-500/20"
              >
                <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
                {ex}
              </span>
            ))}
          </div>
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
      </div>
    );
  }
));

SearchBar.displayName = "SearchBar";
