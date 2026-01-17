import { forwardRef, useEffect, useRef } from "react";
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
    // 내부 ref가 없을 경우를 대비해 로컬 ref 생성 (forwardRef와 병합 필요하지만 여기선 간단히 처리)
    // 실제로는 forwardedRef가 함수일 수도 있어 복잡하지만, 이 컴포넌트는 App에서 객체 ref를 넘김을 가정
    // 안전을 위해 innerRef 도입
    const innerRef = useRef<HTMLInputElement>(null);

    useEffect(() => {
      // 외부에서 query prop이 바뀌면 (예: 최근검색 클릭, 클리어 등) input 값 동기화
      // 단, 타이핑 중에는 커서 위치 튐 방지를 위해 현재 값과 다를 때만 업데이트
      if (innerRef.current && innerRef.current.value !== query) {
        innerRef.current.value = query;
      }
    }, [query]);

    // ref 병합 (외부 ref + 내부 innerRef)
    useEffect(() => {
      if (!ref) return;
      if (typeof ref === 'function') {
        ref(innerRef.current);
      } else {
        ref.current = innerRef.current;
      }
    }, [ref]);

    return (
      <div className="max-w-4xl mx-auto w-full relative z-10">
        {/* 검색 입력 컨테이너 */}
        <div className="relative group">
          {/* Glow Effect Background */}
          <div className="absolute -inset-0.5 bg-gradient-to-r from-blue-500 to-purple-600 rounded-2xl opacity-20 group-hover:opacity-40 transition duration-500 blur-lg group-focus-within:opacity-70 group-focus-within:duration-200"></div>

          <div className="relative rounded-2xl shadow-xl transition-all duration-300 transform group-focus-within:scale-[1.01] group-focus-within:shadow-2xl ring-1 ring-black/5" style={{ backgroundColor: "var(--color-bg-secondary)" }}>
            {/* Input Wrapper */}
            <div className="flex items-center px-6 py-5">
              {/* Search Icon */}
              <div className="group-focus-within:text-blue-500 transition-colors duration-300" style={{ color: "var(--color-text-muted)" }}>
                <svg
                  className="w-7 h-7"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2.5}
                    d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
                  />
                </svg>
              </div>

              <input
                ref={innerRef}
                type="text"
                defaultValue={query} // Use defaultValue for uncontrolled-like behavior initially
                // controlled behavior handled manually via ref or local state if strictly needed,
                // but standard React controlled input with composition handling is better:
                onChange={(e) => {
                  onQueryChange(e.target.value);
                }}
                onCompositionStart={() => {
                  // Optional: marking a ref if we need to block updates, but usually just ensuring
                  // we don't force-update the value prop during composition is key.
                  // However, uncontrolled with sync is the most robust for this specific "External IME" issue.
                }}
                placeholder="무엇을 찾고 계신가요?"
                className="w-full bg-transparent border-none text-xl font-medium focus:outline-none focus:ring-0 ml-4 h-full py-2"
                style={{ color: "var(--color-text-primary)" }}
                aria-label="검색어 입력"
              />

              {/* Loading Spinner */}
              {isLoading && (
                <div className="ml-4">
                  <div
                    className="w-6 h-6 rounded-full border-[3px] animate-spin"
                    style={{ borderColor: "var(--color-border)", borderTopColor: "var(--color-accent)" }}
                  />
                </div>
              )}
            </div>
          </div>
        </div>

        {/* 하단 컨트롤 바 */}
        <div className="flex items-center justify-between mt-5 px-2">
          {/* 검색 모드 토글 */}
          <div className="flex p-1.5 rounded-xl backdrop-blur-sm shadow-sm" style={{ backgroundColor: "var(--color-bg-tertiary)", border: "1px solid var(--color-border)" }}>
            {SEARCH_MODES.map((mode) => {
              const needsSemantic = mode.value !== "keyword";
              const disabled = needsSemantic && !status?.semantic_available;
              const isActive = searchMode === mode.value;

              return (
                <button
                  key={mode.value}
                  onClick={() => !disabled && onSearchModeChange(mode.value)}
                  disabled={disabled}
                  className={`
                    relative px-5 py-2 text-sm font-semibold rounded-lg transition-all duration-200 ease-out flex items-center gap-2
                    ${disabled ? "opacity-50 cursor-not-allowed grayscale" : "cursor-pointer"}
                  `}
                  style={{
                    backgroundColor: isActive ? "var(--color-bg-secondary)" : "transparent",
                    color: isActive ? "var(--color-accent)" : "var(--color-text-muted)",
                    boxShadow: isActive ? "0 1px 3px rgba(0,0,0,0.1)" : "none",
                  }}
                  title={disabled ? "모델 파일 필요" : mode.desc}
                >
                  {/* 작은 닷 인디케이터 */}
                  {isActive && (
                    <span className="w-1.5 h-1.5 rounded-full bg-blue-500 animate-pulse-glow" />
                  )}
                  {mode.label}
                </button>
              );
            })}
          </div>

          {/* 검색 결과 카운트 (Hero Badge) */}
          <div className="flex items-center gap-3">
            {searchTime !== null && resultCount !== undefined && resultCount > 0 && (
              <div
                className="flex items-center gap-2 px-4 py-2 rounded-full text-sm font-medium animate-fade-in shadow-sm"
                style={{
                  backgroundColor: "var(--color-accent-subtle)",
                  border: "1px solid var(--color-accent)",
                  color: "var(--color-accent)"
                }}
              >
                <span
                  className="flex items-center justify-center w-5 h-5 rounded-full text-[10px] font-bold"
                  style={{ backgroundColor: "var(--color-accent-light)" }}
                >
                  {resultCount > 99 ? '99+' : resultCount}
                </span>
                <span>Found in {searchTime}ms</span>
              </div>
            )}
          </div>
        </div>
      </div>
    );
  }
);

SearchBar.displayName = "SearchBar";
