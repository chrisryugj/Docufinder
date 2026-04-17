import { memo, useMemo } from "react";
import type { SearchResult } from "../../types/search";

interface MatchDensityBarProps {
  chunks: SearchResult[];
  onJump?: (chunk: SearchResult) => void;
  compact?: boolean;
}

/**
 * 매칭 밀도 히트맵: 파일 내 청크 매칭 위치를 1줄 SVG 바로 시각화.
 *
 * - X축: chunk_index (정규화 0~1)
 * - 각 dot: 매칭 청크 하나
 * - 색: 신뢰도 tier (success/warning/muted)
 * - 호버: 위치 + 신뢰도 툴팁
 * - 클릭: onJump 콜백 (해당 페이지로 점프)
 *
 * v1: chunk_index 상대 정규화 (총 청크 수 미제공). 추후 backend total_chunks 추가 시 절대 스케일로 업그레이드.
 */
export const MatchDensityBar = memo(function MatchDensityBar({
  chunks,
  onJump,
  compact = false,
}: MatchDensityBarProps) {
  const { items, isAbsolute } = useMemo(() => {
    if (chunks.length === 0) return { items: [], isAbsolute: false };
    // total_chunks가 제공되면 절대 스케일 (문서 전체 대비 위치)
    const total = chunks[0]?.total_chunks ?? 0;
    if (total > 1) {
      const denom = total - 1; // chunk_index는 0-based → 0..total-1 매핑
      return {
        items: chunks.map((chunk, i) => ({
          key: `${chunk.chunk_index}-${i}`,
          pct: Math.max(0, Math.min(100, (chunk.chunk_index / denom) * 100)),
          chunk,
        })),
        isAbsolute: true,
      };
    }
    // fallback: 매칭 간 상대 스케일 (max + 10% 패딩)
    const indices = chunks.map((c) => c.chunk_index);
    const maxIdx = Math.max(...indices);
    const scale = maxIdx === 0 ? 1 : maxIdx * 1.1;
    return {
      items: chunks.map((chunk, i) => ({
        key: `${chunk.chunk_index}-${i}`,
        pct: scale > 0 ? (chunk.chunk_index / scale) * 100 : 50,
        chunk,
      })),
      isAbsolute: false,
    };
  }, [chunks]);

  if (chunks.length <= 1) return null;

  const height = compact ? 6 : 8;
  const dotWidth = compact ? 3 : 4;
  const total = chunks[0]?.total_chunks ?? 0;
  const title = isAbsolute
    ? `${chunks.length}개 매칭 · 전체 ${total}개 청크 중`
    : `${chunks.length}개 매칭 분포`;

  return (
    <div
      className="relative w-full"
      style={{
        height: `${height}px`,
        borderRadius: `${height / 2}px`,
        background: "color-mix(in srgb, var(--color-border) 50%, transparent)",
        overflow: "hidden",
      }}
      title={title}
    >
      {items.map(({ key, pct, chunk }) => {
        const color =
          chunk.confidence >= 70
            ? "var(--color-success)"
            : chunk.confidence >= 40
              ? "var(--color-warning)"
              : "var(--color-text-muted)";
        const tooltip = `${
          chunk.location_hint || (chunk.page_number ? `${chunk.page_number}p` : `청크 ${chunk.chunk_index + 1}`)
        } · 신뢰도 ${Math.round(chunk.confidence)}%`;
        return (
          <button
            key={key}
            onClick={(e) => {
              e.stopPropagation();
              onJump?.(chunk);
            }}
            className="absolute top-0 bottom-0 transition-transform hover:scale-y-150"
            style={{
              left: `calc(${pct}% - ${dotWidth / 2}px)`,
              width: `${dotWidth}px`,
              background: color,
              borderRadius: "1px",
              cursor: onJump ? "pointer" : "default",
              opacity: 0.85,
            }}
            title={tooltip}
            aria-label={tooltip}
          />
        );
      })}
    </div>
  );
});
