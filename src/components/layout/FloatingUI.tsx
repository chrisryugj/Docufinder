import { memo } from "react";

interface FloatingUIProps {
  showScrollTop: boolean;
  onScrollToTop: () => void;
}

export const FloatingUI = memo(function FloatingUI({
  showScrollTop,
  onScrollToTop,
}: FloatingUIProps) {
  if (!showScrollTop) return null;

  return (
    <button
      onClick={onScrollToTop}
      className="fixed bottom-20 right-6 w-10 h-10 rounded-full flex items-center justify-center transition-all duration-200 hover:scale-105 z-40"
      style={{
        backgroundColor: "var(--color-bg-secondary)",
        border: "1px solid var(--color-border)",
        boxShadow: "0 2px 8px rgba(0,0,0,0.15)",
      }}
      aria-label="맨 위로 스크롤"
    >
      <svg
        className="w-5 h-5"
        fill="none"
        stroke="currentColor"
        strokeWidth={2}
        viewBox="0 0 24 24"
        style={{ color: "var(--color-text-muted)" }}
      >
        <path
          strokeLinecap="round"
          strokeLinejoin="round"
          d="M5 15l7-7 7 7"
        />
      </svg>
    </button>
  );
});
