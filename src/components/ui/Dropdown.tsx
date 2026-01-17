import { useState, useRef, useEffect, ReactNode } from "react";

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
  const dropdownRef = useRef<HTMLDivElement>(null);

  const selected = options.find((opt) => opt.value === value);

  // 외부 클릭 시 닫기
  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (
        dropdownRef.current &&
        !dropdownRef.current.contains(e.target as Node)
      ) {
        setIsOpen(false);
      }
    }

    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  // 키보드 네비게이션
  useEffect(() => {
    if (!isOpen) return;

    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") {
        setIsOpen(false);
      }
    }

    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [isOpen]);

  return (
    <div ref={dropdownRef} className={`relative ${className}`}>
      {/* Trigger */}
      <button
        type="button"
        onClick={() => !disabled && setIsOpen(!isOpen)}
        disabled={disabled}
        className="flex items-center justify-between gap-2 px-3 py-2 text-sm rounded-lg transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
        style={{
          backgroundColor: "var(--color-bg-tertiary)",
          border: `1px solid ${isOpen ? "var(--color-accent)" : "var(--color-border)"}`,
          boxShadow: isOpen ? "0 0 0 3px var(--color-accent-light)" : undefined,
        }}
        aria-haspopup="listbox"
        aria-expanded={isOpen}
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

      {/* Menu */}
      {isOpen && (
        <div
          className="absolute z-50 mt-1 w-full min-w-[160px] rounded-lg py-1"
          style={{
            backgroundColor: "var(--color-bg-secondary)",
            border: "1px solid var(--color-border)",
            boxShadow: "var(--shadow-lg)",
          }}
          role="listbox"
        >
          {options.map((option) => (
            <button
              key={String(option.value)}
              type="button"
              onClick={() => {
                onChange(option.value);
                setIsOpen(false);
              }}
              className="w-full text-left px-3 py-2 text-sm transition-colors focus:outline-none"
              style={{
                color: option.value === value ? "var(--color-accent)" : "var(--color-text-primary)",
              }}
              onMouseEnter={(e) => {
                e.currentTarget.style.backgroundColor = "var(--color-bg-hover)";
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.backgroundColor = "transparent";
              }}
              role="option"
              aria-selected={option.value === value}
            >
              <div className="font-medium">{option.label}</div>
              {option.description && (
                <div className="text-xs" style={{ color: "var(--color-text-muted)" }}>
                  {option.description}
                </div>
              )}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
