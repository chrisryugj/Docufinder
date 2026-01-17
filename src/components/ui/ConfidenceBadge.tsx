interface ConfidenceBadgeProps {
  /** 신뢰도 (0-100) */
  confidence: number;
  /** 막대바 표시 여부 */
  showBar?: boolean;
  /** 컴팩트 모드 (숫자만) */
  compact?: boolean;
}

type ConfidenceLevel = "high" | "medium" | "low";

function getLevel(confidence: number): ConfidenceLevel {
  if (confidence >= 70) return "high";
  if (confidence >= 40) return "medium";
  return "low";
}

const levelColors: Record<ConfidenceLevel, string> = {
  high: "var(--color-success, #22c55e)",
  medium: "var(--color-warning, #f59e0b)",
  low: "var(--color-text-muted, #9ca3af)",
};

/**
 * 검색 결과 신뢰도 표시 뱃지
 * - 막대바 + 퍼센트 형태
 * - 높음(녹색) / 중간(주황) / 낮음(회색) 구분
 */
export function ConfidenceBadge({
  confidence,
  showBar = true,
  compact = false,
}: ConfidenceBadgeProps) {
  const level = getLevel(confidence);
  const color = levelColors[level];

  if (compact) {
    return (
      <span
        className="text-xs font-medium tabular-nums"
        style={{ color }}
        title={`신뢰도 ${confidence}%`}
      >
        {confidence}%
      </span>
    );
  }

  return (
    <div className="flex items-center gap-1.5" title={`신뢰도 ${confidence}%`}>
      {showBar && (
        <div
          className="w-10 h-1.5 rounded-full overflow-hidden"
          style={{ backgroundColor: "var(--color-bg-tertiary, #e5e7eb)" }}
        >
          <div
            className="h-full rounded-full transition-all duration-300"
            style={{
              width: `${confidence}%`,
              backgroundColor: color,
            }}
          />
        </div>
      )}
      <span
        className="text-xs font-medium tabular-nums min-w-[2.5rem]"
        style={{ color }}
      >
        {confidence}%
      </span>
    </div>
  );
}
