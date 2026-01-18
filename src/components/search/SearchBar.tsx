import { forwardRef, useCallback, useEffect, useRef, useState } from "react";
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
    const innerRef = useRef<HTMLInputElement>(null);
    const hasInitializedIME = useRef(false);
    const [showModeDropdown, setShowModeDropdown] = useState(false);
    const dropdownRef = useRef<HTMLDivElement>(null);

    // 드롭다운 외부 클릭 시 닫기
    useEffect(() => {
      const handleClickOutside = (e: MouseEvent) => {
        if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
          setShowModeDropdown(false);
        }
      };
      if (showModeDropdown) {
        document.addEventListener("mousedown", handleClickOutside);
      }
      return () => document.removeEventListener("mousedown", handleClickOutside);
    }, [showModeDropdown]);

    // 첫 포커스 시 IME 초기화 (Windows 한영전환 문제 해결)
    const handleFocus = useCallback(() => {
      if (hasInitializedIME.current) return;
      hasInitializedIME.current = true;

      const input = innerRef.current;
      if (!input) return;

      // blur 후 최소 딜레이로 다시 focus (Windows IME 리셋)
      // requestAnimationFrame 2연속 ≈ 32ms (100ms보다 빠르고 안정적)
      input.blur();
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          input.focus();
        });
      });
    }, []);

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

    const currentMode = SEARCH_MODES.find((m) => m.value === searchMode);

    return (
      <div className="max-w-4xl mx-auto w-full">
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
            onFocus={handleFocus}
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

          {/* 검색 모드 배지 + 드롭다운 */}
          <div ref={dropdownRef} className="relative ml-2 flex-shrink-0">
            <button
              onClick={() => setShowModeDropdown(!showModeDropdown)}
              className="flex items-center gap-1 px-2 py-1 rounded-md text-xs font-medium transition-colors"
              style={{
                backgroundColor: "var(--color-bg-tertiary)",
                color: "var(--color-text-secondary)",
                border: "1px solid var(--color-border)",
              }}
              title={currentMode?.desc}
            >
              {currentMode?.label}
              <svg
                className={`w-3 h-3 transition-transform ${showModeDropdown ? "rotate-180" : ""}`}
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
              </svg>
            </button>

            {showModeDropdown && (
              <div
                className="absolute top-full right-0 mt-1 py-1 rounded-lg shadow-lg z-50 min-w-[140px]"
                style={{
                  backgroundColor: "var(--color-bg-secondary)",
                  border: "1px solid var(--color-border)",
                }}
              >
                {SEARCH_MODES.map((mode) => {
                  const needsSemantic = mode.value === "semantic" || mode.value === "hybrid";
                  const disabled = needsSemantic && !status?.semantic_available;
                  const isActive = searchMode === mode.value;

                  return (
                    <button
                      key={mode.value}
                      onClick={() => {
                        if (!disabled) {
                          onSearchModeChange(mode.value);
                          setShowModeDropdown(false);
                        }
                      }}
                      disabled={disabled}
                      className={`
                        w-full px-3 py-1.5 text-xs text-left transition-colors
                        ${disabled ? "opacity-40 cursor-not-allowed" : "cursor-pointer"}
                      `}
                      style={{
                        backgroundColor: isActive ? "var(--color-accent-light)" : "transparent",
                        color: isActive ? "var(--color-accent)" : "var(--color-text-secondary)",
                      }}
                      onMouseEnter={(e) => {
                        if (!disabled && !isActive) {
                          e.currentTarget.style.backgroundColor = "var(--color-bg-tertiary)";
                        }
                      }}
                      onMouseLeave={(e) => {
                        if (!isActive) {
                          e.currentTarget.style.backgroundColor = "transparent";
                        }
                      }}
                      title={disabled ? "모델 파일 필요" : mode.desc}
                    >
                      <div className="font-medium">{mode.label}</div>
                      <div className="text-[10px] opacity-70">{mode.desc}</div>
                    </button>
                  );
                })}
              </div>
            )}
          </div>
        </div>
      </div>
    );
  }
);

SearchBar.displayName = "SearchBar";
