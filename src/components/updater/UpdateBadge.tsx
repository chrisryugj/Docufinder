import { useState } from "react";
import type { UpdateState } from "../../hooks/useUpdater";

interface UpdateBadgeProps {
  state: UpdateState;
  onClick: () => void;
}

export function UpdateBadge({ state, onClick }: UpdateBadgeProps) {
  const { phase, version } = state;
  const [hovered, setHovered] = useState(false);

  const visible =
    phase === "available" ||
    phase === "downloading" ||
    phase === "installing" ||
    phase === "ready-to-restart";

  if (!visible) return null;

  const label =
    phase === "available"
      ? `업데이트 ${version}`
      : phase === "downloading"
      ? "다운로드 중"
      : phase === "installing"
      ? "설치 중"
      : "재시작 필요";

  return (
    <button
      onClick={onClick}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-medium transition-all duration-150"
      style={{
        backgroundColor: hovered ? "var(--color-accent)" : "var(--color-accent-light)",
        color: hovered ? "#fff" : "var(--color-accent)",
        border: "1px solid var(--color-accent)",
        transform: hovered ? "translateY(-1px)" : "translateY(0)",
        boxShadow: hovered
          ? "0 4px 12px rgba(0, 0, 0, 0.15)"
          : "0 1px 2px rgba(0, 0, 0, 0.06)",
      }}
      title="업데이트 정보 보기"
    >
      <span
        className="w-1.5 h-1.5 rounded-full animate-pulse"
        style={{ backgroundColor: hovered ? "#fff" : "var(--color-accent)" }}
      />
      {label}
    </button>
  );
}
