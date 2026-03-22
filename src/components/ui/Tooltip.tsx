import { useState, useEffect, ReactNode, useRef, useId } from "react";
import { createPortal } from "react-dom";

interface TooltipProps {
  content: ReactNode;
  children: ReactNode;
  position?: "top" | "bottom" | "left" | "right";
  delay?: number;
  maxWidth?: number;
  /** overflow: hidden 부모 안에서 잘리는 문제 방지 (Portal 사용) */
  usePortal?: boolean;
}

const positionStyles = {
  top: "bottom-full left-1/2 -translate-x-1/2 mb-2",
  bottom: "top-full left-1/2 -translate-x-1/2 mt-2",
  left: "right-full top-1/2 -translate-y-1/2 mr-2",
  right: "left-full top-1/2 -translate-y-1/2 ml-2",
};

const arrowStyles = {
  top: "top-full left-1/2 -translate-x-1/2 border-x-transparent border-b-transparent",
  bottom: "bottom-full left-1/2 -translate-x-1/2 border-x-transparent border-t-transparent",
  left: "left-full top-1/2 -translate-y-1/2 border-y-transparent border-r-transparent",
  right: "right-full top-1/2 -translate-y-1/2 border-y-transparent border-l-transparent",
};

const arrowColorStyles: Record<string, React.CSSProperties> = {
  top: { borderTopColor: "var(--color-bg-tertiary)" },
  bottom: { borderBottomColor: "var(--color-bg-tertiary)" },
  left: { borderLeftColor: "var(--color-bg-tertiary)" },
  right: { borderRightColor: "var(--color-bg-tertiary)" },
};

export function Tooltip({
  content,
  children,
  position = "top",
  delay = 300,
  maxWidth,
  usePortal = false,
}: TooltipProps) {
  const tooltipId = useId();
  const [isVisible, setIsVisible] = useState(false);
  const [portalPos, setPortalPos] = useState<{ top: number; left: number } | null>(null);
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const triggerRef = useRef<HTMLDivElement>(null);

  // 언마운트 시 타이머 정리 (메모리 누수 방지)
  useEffect(() => () => {
    if (timeoutRef.current) clearTimeout(timeoutRef.current);
  }, []);

  const showTooltip = () => {
    timeoutRef.current = setTimeout(() => {
      if (usePortal && triggerRef.current) {
        const rect = triggerRef.current.getBoundingClientRect();
        const gap = 8;
        let top = 0, left = 0;
        switch (position) {
          case "top":    top = rect.top - gap;    left = rect.left + rect.width / 2; break;
          case "bottom": top = rect.bottom + gap;  left = rect.left + rect.width / 2; break;
          case "left":   top = rect.top + rect.height / 2; left = rect.left - gap; break;
          case "right":  top = rect.top + rect.height / 2; left = rect.right + gap; break;
        }
        // 뷰포트 경계 클램핑 (화면 밖 방지)
        const margin = 8;
        left = Math.max(margin, Math.min(left, window.innerWidth - margin));
        top = Math.max(margin, Math.min(top, window.innerHeight - margin));
        setPortalPos({ top, left });
      }
      setIsVisible(true);
    }, delay);
  };

  const hideTooltip = () => {
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current);
    }
    setIsVisible(false);
  };

  const portalTransform: Record<string, string> = {
    top: "translateX(-50%) translateY(-100%)",
    bottom: "translateX(-50%)",
    left: "translateY(-50%) translateX(-100%)",
    right: "translateY(-50%)",
  };

  const tooltipContent = isVisible && content && (usePortal ? (
    portalPos && createPortal(
      <div
        id={tooltipId}
        className={`fixed z-[9999] px-2 py-1 text-xs rounded shadow-lg pointer-events-none ${maxWidth ? "" : "whitespace-nowrap"}`}
        style={{
          top: portalPos.top,
          left: portalPos.left,
          transform: portalTransform[position],
          backgroundColor: "var(--color-bg-tertiary)",
          color: "var(--color-text-secondary)",
          ...(maxWidth ? { width: maxWidth, maxWidth, whiteSpace: "normal" as const } : {}),
        }}
        role="tooltip"
      >
        {content}
        <div
          className={`absolute border-4 ${arrowStyles[position]}`}
          style={arrowColorStyles[position]}
          aria-hidden="true"
        />
      </div>,
      document.body
    )
  ) : (
    <div
      id={tooltipId}
      className={`
        absolute z-50 px-2 py-1
        text-xs rounded shadow-lg
        pointer-events-none
        ${maxWidth ? "" : "whitespace-nowrap"}
        ${positionStyles[position]}
      `}
      style={{
        backgroundColor: "var(--color-bg-tertiary)",
        color: "var(--color-text-secondary)",
        ...(maxWidth ? { width: maxWidth, maxWidth, whiteSpace: "normal" as const } : {}),
      }}
      role="tooltip"
    >
      {content}
      <div
        className={`absolute border-4 ${arrowStyles[position]}`}
        style={arrowColorStyles[position]}
        aria-hidden="true"
      />
    </div>
  ));

  return (
    <div
      ref={triggerRef}
      className="relative inline-flex items-center overflow-visible"
      onMouseEnter={showTooltip}
      onMouseLeave={hideTooltip}
      onFocus={showTooltip}
      onBlur={hideTooltip}
      aria-describedby={isVisible ? tooltipId : undefined}
    >
      {children}
      {tooltipContent}
    </div>
  );
}

// 정보 아이콘 + 툴팁 조합
export function InfoTooltip({
  content,
  position = "right",
  maxWidth = 280,
}: {
  content: ReactNode;
  position?: "top" | "bottom" | "left" | "right";
  maxWidth?: number;
}) {
  return (
    <Tooltip content={content} position={position} maxWidth={maxWidth} delay={200}>
      <button
        type="button"
        className="inline-flex items-center justify-center w-4 h-4 ml-1.5 rounded-full text-[10px] font-medium transition-colors"
        style={{
          backgroundColor: "var(--color-bg-tertiary)",
          color: "var(--color-text-muted)",
        }}
        aria-label="도움말"
      >
        ?
      </button>
    </Tooltip>
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
          <kbd className="px-1.5 py-0.5 rounded text-[10px] font-mono" style={{ backgroundColor: "var(--color-bg-secondary)" }}>
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
