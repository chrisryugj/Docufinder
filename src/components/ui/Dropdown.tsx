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
        className={`
          flex items-center justify-between gap-2 px-3 py-1.5 text-sm
          bg-gray-800 border border-gray-700 rounded-lg
          hover:bg-gray-700 focus:outline-none focus:ring-2 focus:ring-blue-500
          disabled:opacity-50 disabled:cursor-not-allowed
          ${isOpen ? "ring-2 ring-blue-500" : ""}
        `}
        aria-haspopup="listbox"
        aria-expanded={isOpen}
      >
        {renderTrigger ? (
          renderTrigger(selected)
        ) : (
          <span className={selected ? "text-gray-200" : "text-gray-500"}>
            {selected?.label || placeholder}
          </span>
        )}
        <svg
          className={`w-4 h-4 text-gray-500 transition-transform ${
            isOpen ? "rotate-180" : ""
          }`}
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
          className="absolute z-50 mt-1 w-full min-w-[160px] bg-gray-800 border border-gray-700 rounded-lg shadow-lg py-1"
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
              className={`
                w-full text-left px-3 py-2 text-sm
                hover:bg-gray-700 focus:bg-gray-700 focus:outline-none
                ${option.value === value ? "text-blue-400" : "text-gray-300"}
              `}
              role="option"
              aria-selected={option.value === value}
            >
              <div className="font-medium">{option.label}</div>
              {option.description && (
                <div className="text-xs text-gray-500">{option.description}</div>
              )}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
