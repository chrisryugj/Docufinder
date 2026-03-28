import { useState, useRef, useEffect, useCallback, memo } from "react";

interface CustomSelectOption<T extends string> {
  value: T;
  label: string;
}

interface CustomSelectProps<T extends string> {
  value: T;
  options: CustomSelectOption<T>[];
  onChange: (value: T) => void;
  /** aria-label */
  ariaLabel?: string;
  /** 컴팩트 모드 (필터 바용, 기본값 true) */
  compact?: boolean;
  /** 활성/비활성 스타일 적용 여부 (기본: 첫 번째 옵션이 기본값) */
  isActive?: boolean;
}

/**
 * 네이티브 <select> 대체 커스텀 드롭다운
 * - 다크모드에서 옵션 목록 완전 스타일링
 * - 키보드 네비게이션 (ArrowUp/Down, Enter, Escape)
 * - 외부 클릭 닫기
 */
function CustomSelectInner<T extends string>({
  value,
  options,
  onChange,
  ariaLabel,
  compact = true,
  isActive,
}: CustomSelectProps<T>) {
  const [isOpen, setIsOpen] = useState(false);
  const [focusIndex, setFocusIndex] = useState(-1);
  const containerRef = useRef<HTMLDivElement>(null);
  const optionRefs = useRef<(HTMLButtonElement | null)[]>([]);

  const active = isActive ?? value !== options[0]?.value;
  const selectedLabel = options.find((o) => o.value === value)?.label ?? "";

  // 외부 클릭 닫기
  useEffect(() => {
    if (!isOpen) return;
    const handler = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setIsOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [isOpen]);

  // 키보드 네비게이션
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (!isOpen) {
        if (e.key === "Enter" || e.key === " " || e.key === "ArrowDown") {
          e.preventDefault();
          setIsOpen(true);
          setFocusIndex(options.findIndex((o) => o.value === value));
        }
        return;
      }

      switch (e.key) {
        case "ArrowDown":
          e.preventDefault();
          setFocusIndex((prev) => Math.min(prev + 1, options.length - 1));
          break;
        case "ArrowUp":
          e.preventDefault();
          setFocusIndex((prev) => Math.max(prev - 1, 0));
          break;
        case "Enter":
        case " ":
          e.preventDefault();
          if (focusIndex >= 0 && focusIndex < options.length) {
            onChange(options[focusIndex].value);
            setIsOpen(false);
          }
          break;
        case "Escape":
          e.preventDefault();
          setIsOpen(false);
          break;
        case "Home":
          e.preventDefault();
          setFocusIndex(0);
          break;
        case "End":
          e.preventDefault();
          setFocusIndex(options.length - 1);
          break;
      }
    },
    [isOpen, focusIndex, options, onChange, value],
  );

  // 포커스 인덱스 변경 시 옵션에 포커스
  useEffect(() => {
    if (isOpen && focusIndex >= 0) {
      optionRefs.current[focusIndex]?.focus();
    }
  }, [isOpen, focusIndex]);

  const py = compact ? "py-0.5" : "py-1.5";
  const px = compact ? "pl-2 pr-5" : "px-3 pr-7";
  const textSize = compact ? "" : "text-sm";

  return (
    <div ref={containerRef} className="relative inline-block" onKeyDown={handleKeyDown}>
      {/* 트리거 버튼 */}
      <button
        type="button"
        onClick={() => {
          setIsOpen(!isOpen);
          if (!isOpen) {
            setFocusIndex(options.findIndex((o) => o.value === value));
          }
        }}
        className={`${px} ${py} ${textSize} rounded border cursor-pointer font-medium
          transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-[var(--color-accent)] focus-visible:ring-offset-1
          text-left whitespace-nowrap`}
        style={{
          backgroundColor: active ? "var(--color-accent-light)" : "var(--color-bg-secondary)",
          borderColor: active ? "var(--color-accent)" : "var(--color-border)",
          color: active ? "var(--color-accent)" : "var(--color-text-secondary)",
        }}
        aria-haspopup="listbox"
        aria-expanded={isOpen}
        aria-label={ariaLabel}
      >
        {selectedLabel}
      </button>

      {/* 드롭다운 아이콘 */}
      <svg
        className={`absolute right-1.5 top-1/2 -translate-y-1/2 w-3 h-3 pointer-events-none transition-transform ${isOpen ? "rotate-180" : ""}`}
        style={{ color: active ? "var(--color-accent)" : "var(--color-text-muted)" }}
        fill="none"
        stroke="currentColor"
        viewBox="0 0 24 24"
      >
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
      </svg>

      {/* 옵션 목록 */}
      {isOpen && (
        <div
          className="absolute left-0 top-full mt-1 min-w-full rounded-md border shadow-lg z-50 overflow-hidden animate-scale-in"
          style={{
            backgroundColor: "var(--color-bg-secondary)",
            borderColor: "var(--color-border)",
          }}
          role="listbox"
          aria-activedescendant={focusIndex >= 0 ? `option-${focusIndex}` : undefined}
        >
          {options.map((option, index) => {
            const isSelected = option.value === value;
            return (
              <button
                key={option.value}
                id={`option-${index}`}
                ref={(el) => {
                  optionRefs.current[index] = el;
                }}
                role="option"
                aria-selected={isSelected}
                tabIndex={-1}
                onClick={() => {
                  onChange(option.value);
                  setIsOpen(false);
                }}
                className={`w-full px-3 ${compact ? "py-1" : "py-1.5"} ${textSize} text-left transition-colors
                  ${index === focusIndex ? "bg-[var(--color-bg-tertiary)]" : ""}
                  hover:bg-[var(--color-bg-tertiary)]`}
                style={{
                  color: isSelected ? "var(--color-accent)" : "var(--color-text-secondary)",
                  fontWeight: isSelected ? 600 : 400,
                }}
              >
                {option.label}
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}

export const CustomSelect = memo(CustomSelectInner) as typeof CustomSelectInner;
