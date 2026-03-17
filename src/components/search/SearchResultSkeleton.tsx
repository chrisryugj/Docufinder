import { memo } from "react";

interface SearchResultSkeletonProps {
  count?: number;
}

/** 검색 결과 스켈레톤 로더 — 실제 결과 레이아웃과 동일한 형태 */
export const SearchResultSkeleton = memo(function SearchResultSkeleton({
  count = 5,
}: SearchResultSkeletonProps) {
  return (
    <div className="space-y-0.5" aria-busy="true" aria-label="검색 결과 로딩 중">
      {Array.from({ length: count }, (_, i) => (
        <div
          key={i}
          className="rounded-md animate-pulse"
          style={{
            padding: "0.75rem 0.875rem",
            animationDelay: `${i * 80}ms`,
            animationFillMode: "both",
          }}
        >
          {/* Row 1: 파일 아이콘 + 파일명 + 배지 */}
          <div className="flex items-center justify-between mb-2">
            <div className="flex items-center gap-2 flex-1">
              <div
                className="w-5 h-5 rounded"
                style={{ backgroundColor: "var(--color-bg-subtle)" }}
              />
              <div
                className="h-3.5 rounded"
                style={{
                  backgroundColor: "var(--color-bg-subtle)",
                  width: `${140 + (i % 3) * 60}px`,
                }}
              />
            </div>
            <div className="flex items-center gap-2">
              <div
                className="w-8 h-3 rounded"
                style={{ backgroundColor: "var(--color-bg-subtle)" }}
              />
              <div
                className="w-10 h-4 rounded"
                style={{ backgroundColor: "var(--color-bg-subtle)" }}
              />
            </div>
          </div>

          {/* Row 2: 본문 미리보기 2줄 */}
          <div className="space-y-1.5 ml-7">
            <div
              className="h-3 rounded"
              style={{
                backgroundColor: "var(--color-bg-subtle)",
                width: "100%",
                opacity: 0.7,
              }}
            />
            <div
              className="h-3 rounded"
              style={{
                backgroundColor: "var(--color-bg-subtle)",
                width: `${70 + (i % 4) * 8}%`,
                opacity: 0.5,
              }}
            />
          </div>

          {/* Row 3: 경로 */}
          <div className="mt-2 ml-7">
            <div
              className="h-2.5 rounded"
              style={{
                backgroundColor: "var(--color-bg-subtle)",
                width: `${50 + (i % 3) * 15}%`,
                opacity: 0.4,
              }}
            />
          </div>
        </div>
      ))}
    </div>
  );
});
