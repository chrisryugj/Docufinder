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
      <div className="max-w-3xl mx-auto w-full">
        {/* 검색 입력 */}
        <div
          className="flex items-center px-4 py-3 rounded-xl transition-shadow duration-200 focus-within:ring-2 focus-within:ring-blue-500/30"
          style={{
            backgroundColor: "var(--color-bg-secondary)",
            border: "1px solid var(--color-border)",
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
            onChange={(e) => onQueryChange(e.target.value)}
            placeholder="검색어 입력..."
            className="flex-1 bg-transparent border-none text-base focus:outline-none ml-3"
            style={{ color: "var(--color-text-primary)" }}
            aria-label="검색어 입력"
          />

          {isLoading && (
            <div
              className="w-5 h-5 rounded-full border-2 animate-spin ml-2"
              style={{ borderColor: "var(--color-border)", borderTopColor: "var(--color-accent)" }}
            />
          )}
        </div>

        {/* 하단 컨트롤 */}
        <div className="flex items-center justify-between mt-3 px-1">
          {/* 검색 모드 */}
          <div
            className="flex gap-1 p-1 rounded-lg"
            style={{ backgroundColor: "var(--color-bg-tertiary)" }}
          >
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
                    px-3 py-1.5 text-sm rounded-md transition-colors
                    ${disabled ? "opacity-40 cursor-not-allowed" : "cursor-pointer hover:opacity-80"}
                  `}
                  style={{
                    backgroundColor: isActive ? "var(--color-bg-secondary)" : "transparent",
                    color: isActive ? "var(--color-text-primary)" : "var(--color-text-muted)",
                  }}
                  title={disabled ? "모델 파일 필요" : mode.desc}
                >
                  {mode.label}
                </button>
              );
            })}
          </div>

          {/* 검색 결과 */}
          {searchTime !== null && resultCount !== undefined && resultCount > 0 && (
            <span className="text-sm" style={{ color: "var(--color-text-muted)" }}>
              {resultCount}건 · {searchTime}ms
            </span>
          )}
        </div>
      </div>
    );
  }
);

SearchBar.displayName = "SearchBar";
