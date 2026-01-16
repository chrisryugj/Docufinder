import { useState, ReactNode, useRef } from "react";

interface TooltipProps {
  content: ReactNode;
  children: ReactNode;
  position?: "top" | "bottom" | "left" | "right";
  delay?: number;
}

const positionStyles = {
  top: "bottom-full left-1/2 -translate-x-1/2 mb-2",
  bottom: "top-full left-1/2 -translate-x-1/2 mt-2",
  left: "right-full top-1/2 -translate-y-1/2 mr-2",
  right: "left-full top-1/2 -translate-y-1/2 ml-2",
};

const arrowStyles = {
  top: "top-full left-1/2 -translate-x-1/2 border-t-gray-700 border-x-transparent border-b-transparent",
  bottom:
    "bottom-full left-1/2 -translate-x-1/2 border-b-gray-700 border-x-transparent border-t-transparent",
  left: "left-full top-1/2 -translate-y-1/2 border-l-gray-700 border-y-transparent border-r-transparent",
  right:
    "right-full top-1/2 -translate-y-1/2 border-r-gray-700 border-y-transparent border-l-transparent",
};

export function Tooltip({
  content,
  children,
  position = "top",
  delay = 300,
}: TooltipProps) {
  const [isVisible, setIsVisible] = useState(false);
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const showTooltip = () => {
    timeoutRef.current = setTimeout(() => setIsVisible(true), delay);
  };

  const hideTooltip = () => {
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current);
    }
    setIsVisible(false);
  };

  return (
    <div
      className="relative inline-block"
      onMouseEnter={showTooltip}
      onMouseLeave={hideTooltip}
      onFocus={showTooltip}
      onBlur={hideTooltip}
    >
      {children}

      {isVisible && content && (
        <div
          className={`
            absolute z-50 px-2 py-1
            bg-gray-700 text-gray-200 text-xs rounded shadow-lg
            whitespace-nowrap pointer-events-none
            ${positionStyles[position]}
          `}
          role="tooltip"
        >
          {content}
          {/* Arrow */}
          <div
            className={`absolute border-4 ${arrowStyles[position]}`}
            aria-hidden="true"
          />
        </div>
      )}
    </div>
  );
}

// 단축키 표시용 특화 Tooltip
export function ShortcutTooltip({
  shortcut,
  children,
  position = "bottom",
}: {
  shortcut: string;
  children: ReactNode;
  position?: "top" | "bottom" | "left" | "right";
}) {
  return (
    <Tooltip
      content={
        <span className="flex items-center gap-1">
          <kbd className="px-1.5 py-0.5 bg-gray-600 rounded text-[10px] font-mono">
            {shortcut}
          </kbd>
        </span>
      }
      position={position}
    >
      {children}
    </Tooltip>
  );
}
