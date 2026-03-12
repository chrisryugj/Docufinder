import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { SearchMode } from "../../types/search";
import { SEARCH_MODES } from "../../types/search";
import type { IndexStatus } from "../../types/index";

interface SearchModeDropdownProps {
  searchMode: SearchMode;
  onSearchModeChange: (mode: SearchMode) => void;
  status: IndexStatus | null;
}

export const SearchModeDropdown = memo(
  ({ searchMode, onSearchModeChange, status }: SearchModeDropdownProps) => {
    const [showDropdown, setShowDropdown] = useState(false);
    const [focusedIndex, setFocusedIndex] = useState(-1);
    const dropdownRef = useRef<HTMLDivElement>(null);
    const optionRefs = useRef<(HTMLButtonElement | null)[]>([]);

    // 활성화 가능한 모드 목록 (키보드 탐색용)
    const enabledIndices = useMemo(
      () =>
        SEARCH_MODES.map((mode, i) => {
          const needsSemantic = mode.value === "semantic" || mode.value === "hybrid";
          return needsSemantic && !status?.semantic_available ? -1 : i;
        }).filter((i) => i >= 0),
      [status]
    );

    // 외부 클릭 시 닫기
    useEffect(() => {
      const handleClickOutside = (e: MouseEvent) => {
        if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
          setShowDropdown(false);
        }
      };
      if (showDropdown) {
        document.addEventListener("mousedown", handleClickOutside);
      }
      return () => document.removeEventListener("mousedown", handleClickOutside);
    }, [showDropdown]);

    // 드롭다운 열릴 때 현재 활성 모드에 포커스
    useEffect(() => {
      if (showDropdown) {
        const activeIdx = SEARCH_MODES.findIndex((m) => m.value === searchMode);
        setFocusedIndex(activeIdx);
        optionRefs.current[activeIdx]?.focus();
      }
    }, [showDropdown, searchMode]);

    // 키보드 핸들러
    const handleKeyDown = useCallback(
      (e: React.KeyboardEvent) => {
        if (!showDropdown) {
          if (e.key === "ArrowDown" || e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            setShowDropdown(true);
          }
          return;
        }

        switch (e.key) {
          case "Escape":
            e.preventDefault();
            setShowDropdown(false);
            dropdownRef.current?.querySelector("button")?.focus();
            break;
          case "ArrowDown": {
            e.preventDefault();
            const curPos = enabledIndices.indexOf(focusedIndex);
            const nextPos = Math.min(curPos + 1, enabledIndices.length - 1);
            const nextIdx = enabledIndices[nextPos];
            setFocusedIndex(nextIdx);
            optionRefs.current[nextIdx]?.focus();
            break;
          }
          case "ArrowUp": {
            e.preventDefault();
            const curPos = enabledIndices.indexOf(focusedIndex);
            const prevPos = Math.max(curPos - 1, 0);
            const prevIdx = enabledIndices[prevPos];
            setFocusedIndex(prevIdx);
            optionRefs.current[prevIdx]?.focus();
            break;
          }
          case "Enter":
          case " ":
            e.preventDefault();
            if (focusedIndex >= 0) {
              const mode = SEARCH_MODES[focusedIndex];
              onSearchModeChange(mode.value);
              setShowDropdown(false);
              dropdownRef.current?.querySelector("button")?.focus();
            }
            break;
        }
      },
      [showDropdown, focusedIndex, enabledIndices, onSearchModeChange]
    );

    const currentMode = SEARCH_MODES.find((m) => m.value === searchMode);

    return (
      <div ref={dropdownRef} className="relative ml-2 flex-shrink-0" onKeyDown={handleKeyDown}>
        <button
          onClick={() => setShowDropdown(!showDropdown)}
          className="flex items-center gap-1 px-2 py-1 rounded-md text-xs font-medium transition-colors"
          style={{
            backgroundColor: "var(--color-bg-tertiary)",
            color: "var(--color-text-secondary)",
            border: "1px solid var(--color-border)",
          }}
          title={currentMode?.desc}
          aria-haspopup="listbox"
          aria-expanded={showDropdown}
          aria-label={`검색 모드: ${currentMode?.label}`}
        >
          {currentMode?.label}
          <svg
            className={`w-3 h-3 transition-transform ${showDropdown ? "rotate-180" : ""}`}
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
          </svg>
        </button>

        {showDropdown && (
          <div
            className="absolute top-full right-0 mt-1 py-1 rounded-lg shadow-lg z-50 min-w-[140px]"
            style={{
              backgroundColor: "var(--color-bg-secondary)",
              border: "1px solid var(--color-border)",
            }}
            role="listbox"
            aria-label="검색 모드 선택"
          >
            {SEARCH_MODES.map((mode, index) => {
              const needsSemantic = mode.value === "semantic" || mode.value === "hybrid";
              const disabled = needsSemantic && !status?.semantic_available;
              const isActive = searchMode === mode.value;

              return (
                <button
                  key={mode.value}
                  ref={(el) => { optionRefs.current[index] = el; }}
                  role="option"
                  aria-selected={isActive}
                  tabIndex={-1}
                  onClick={() => {
                    if (!disabled) {
                      onSearchModeChange(mode.value);
                      setShowDropdown(false);
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
                  title={disabled ? "설정에서 모델을 다운로드하세요" : mode.desc}
                >
                  <div className="font-medium">{mode.label}</div>
                  <div className="text-[10px] opacity-70">
                    {disabled ? "모델 다운로드 필요" : mode.desc}
                  </div>
                </button>
              );
            })}
          </div>
        )}
      </div>
    );
  }
);

SearchModeDropdown.displayName = "SearchModeDropdown";
