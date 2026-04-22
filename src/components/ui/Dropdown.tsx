import { useState, useRef, useEffect, ReactNode, useCallback } from "react";
import { createPortal } from "react-dom";
import type { CSSPropertiesWithVars } from "../../types/css";

interface DropdownOption<T> {
  value: T;
  label: string;
  description?: string;
}

interface DropdownProps<T> {
  options: DropdownOption<T>[];
  value: T;
  onChange: (value: T) => void;
  placeholder?: string;
  disabled?: boolean;
  className?: string;
  renderTrigger?: (selected: DropdownOption<T> | undefined) => ReactNode;
}

export function Dropdown<T extends string | number>({
  options,
  value,
  onChange,
  placeholder = "선택",
  disabled = false,
  className = "",
  renderTrigger,
}: DropdownProps<T>) {
  const [isOpen, setIsOpen] = useState(false);
  const [focusedIndex, setFocusedIndex] = useState(-1);
  const [menuPos, setMenuPos] = useState<{ top: number; left: number; width: number }>({ top: 0, left: 0, width: 0 });
  const dropdownRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const optionRefs = useRef<(HTMLButtonElement | null)[]>([]);

  const selected = options.find((opt) => opt.value === value);
  const selectedIndex = options.findIndex((opt) => opt.value === value);

  // 메뉴 열릴 때 위치 계산 + 선택된 항목으로 포커스 이동
  useEffect(() => {
    if (isOpen && triggerRef.current) {
      const rect = triggerRef.current.getBoundingClientRect();
      setMenuPos({ top: rect.bottom + 4, left: rect.left, width: rect.width });
      setFocusedIndex(selectedIndex >= 0 ? selectedIndex : 0);
    } else {
      setFocusedIndex(-1);
    }
  }, [isOpen, selectedIndex]);

  // 포커스된 옵션으로 스크롤
  useEffect(() => {
    if (isOpen && focusedIndex >= 0 && optionRefs.current[focusedIndex]) {
      optionRefs.current[focusedIndex]?.focus();
    }
  }, [isOpen, focusedIndex]);

  // 외부 클릭 시 닫기 (portal 메뉴 포함)
  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      const target = e.target as Node;
      if (
        dropdownRef.current && !dropdownRef.current.contains(target) &&
        menuRef.current && !menuRef.current.contains(target)
      ) {
        setIsOpen(false);
      }
    }

    if (isOpen) {
      document.addEventListener("mousedown", handleClickOutside);
    }
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [isOpen]);

  // 키보드 네비게이션
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (!isOpen) {
        if (e.key === "ArrowDown" || e.key === "ArrowUp" || e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          setIsOpen(true);
        }
        return;
      }

      switch (e.key) {
        case "Escape":
          e.preventDefault();
          setIsOpen(false);
          break;
        case "ArrowDown":
          e.preventDefault();
          setFocusedIndex((prev) => (prev < options.length - 1 ? prev + 1 : 0));
          break;
        case "ArrowUp":
          e.preventDefault();
          setFocusedIndex((prev) => (prev > 0 ? prev - 1 : options.length - 1));
          break;
        case "Home":
          e.preventDefault();
          setFocusedIndex(0);
          break;
        case "End":
          e.preventDefault();
          setFocusedIndex(options.length - 1);
          break;
        case "Enter":
        case " ":
          e.preventDefault();
          if (focusedIndex >= 0 && focusedIndex < options.length) {
            onChange(options[focusedIndex].value);
            setIsOpen(false);
          }
          break;
        case "Tab":
          setIsOpen(false);
          break;
      }
    },
    [isOpen, focusedIndex, options, onChange]
  );

  return (
    <div ref={dropdownRef} className={`relative ${className}`} onKeyDown={handleKeyDown}>
      {/* Trigger */}
      <button
        ref={triggerRef}
        type="button"
        onClick={() => !disabled && setIsOpen(!isOpen)}
        disabled={disabled}
        className="dropdown-trigger flex items-center justify-between gap-2 px-3 py-2 text-sm rounded-md transition-all disabled:opacity-50 disabled:cursor-not-allowed focus:outline-none focus-visible:ring-2 focus-visible:ring-offset-1"
        style={{
          backgroundColor: "var(--color-bg-tertiary)",
          border: `1px solid ${isOpen ? "var(--color-accent)" : "var(--color-border)"}`,
          boxShadow: isOpen ? "0 0 0 2px var(--color-accent-muted)" : undefined,
          "--tw-ring-color": "var(--color-accent)",
        } as CSSPropertiesWithVars}
        aria-haspopup="listbox"
        aria-expanded={isOpen}
        aria-activedescendant={focusedIndex >= 0 ? `dropdown-option-${focusedIndex}` : undefined}
      >
        {renderTrigger ? (
          renderTrigger(selected)
        ) : (
          <span style={{ color: selected ? "var(--color-text-primary)" : "var(--color-text-muted)" }}>
            {selected?.label || placeholder}
          </span>
        )}
        <svg
          className={`w-4 h-4 transition-transform ${isOpen ? "rotate-180" : ""}`}
          style={{ color: "var(--color-text-muted)" }}
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={2}
            d="M19 9l-7 7-7-7"
          />
        </svg>
      </button>

      {/* Menu — portal로 body에 렌더링 (overflow hidden 부모에서 잘림 방지) */}
      {isOpen && createPortal(
        <div
          ref={menuRef}
          className="fixed z-[9999] min-w-[160px] rounded-md py-1"
          style={{
            top: menuPos.top,
            left: menuPos.left,
            width: Math.max(menuPos.width, 160),
            backgroundColor: "var(--color-bg-secondary)",
            border: "1px solid var(--color-border)",
            boxShadow: "var(--shadow-lg)",
          }}
          role="listbox"
        >
          {options.map((option, index) => (
            <button
              key={String(option.value)}
              ref={(el) => { optionRefs.current[index] = el; }}
              id={`dropdown-option-${index}`}
              type="button"
              onClick={() => {
                onChange(option.value);
                setIsOpen(false);
              }}
              onMouseEnter={() => setFocusedIndex(index)}
              className="w-full text-left px-3 py-2 text-sm transition-colors focus:outline-none"
              style={{
                color: option.value === value ? "var(--color-accent)" : "var(--color-text-primary)",
                backgroundColor: focusedIndex === index ? "var(--color-bg-tertiary)" : "transparent",
              }}
              role="option"
              aria-selected={option.value === value}
              tabIndex={-1}
            >
              <div className="font-medium">{option.label}</div>
              {option.description && (
                <div className="text-xs" style={{ color: "var(--color-text-muted)" }}>
                  {option.description}
                </div>
              )}
            </button>
          ))}
        </div>,
        document.body
      )}
    </div>
  );
}
