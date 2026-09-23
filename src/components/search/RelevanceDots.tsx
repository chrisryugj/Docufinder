/** 관련도 점 신호(●●○) — 시맨틱/하이브리드 매칭에만 쓴다. 숫자 % 대신 3단계로 보여 불안을 줄이고,
 *  정확한 값은 툴팁·낭독기에 남긴다. 목록·그룹 보기가 같은 표시를 쓴다 */
export function RelevanceDots({ confidence }: { confidence: number }) {
  const level = confidence >= 70 ? 3 : confidence >= 40 ? 2 : 1;
  const levelLabel = level === 3 ? "높음" : level === 2 ? "보통" : "낮음";
  const dotColor = level === 3
    ? "var(--color-success)"
    : level === 2
      ? "var(--color-accent-warm)"
      : "var(--color-text-muted)";
  return (
    <span
      className="inline-flex items-center gap-0.5 leading-none"
      aria-label={`관련도 ${levelLabel} (${Math.round(confidence)}%)`}
      title={`관련도: ${levelLabel} (${Math.round(confidence)}%)`}
    >
      {[0, 1, 2].map((i) => (
        <span
          key={i}
          className="w-1.5 h-1.5 rounded-full"
          style={{ backgroundColor: i < level ? dotColor : "var(--color-border)" }}
        />
      ))}
    </span>
  );
}
