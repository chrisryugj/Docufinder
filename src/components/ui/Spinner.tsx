import { memo } from "react";

type SpinnerSize = "xs" | "sm" | "md" | "lg";

const sizeMap: Record<SpinnerSize, string> = {
  xs: "w-3 h-3 border",
  sm: "w-4 h-4 border-2",
  md: "w-6 h-6 border-2",
  lg: "w-8 h-8 border-2",
};

interface SpinnerProps {
  size?: SpinnerSize;
  className?: string;
}

/** 통일 스피너 컴포넌트 — animate-spin 기반, 기능적 애니메이션 (reduced-motion에서도 동작) */
function SpinnerBase({ size = "sm", className = "" }: SpinnerProps) {
  return (
    <div
      className={`rounded-full animate-spin border-current border-t-transparent shrink-0 ${sizeMap[size]} ${className}`}
      style={{ opacity: 0.6 }}
      role="status"
      aria-label="로딩 중"
    >
      <span className="sr-only">로딩 중</span>
    </div>
  );
}

export const Spinner = memo(SpinnerBase);
