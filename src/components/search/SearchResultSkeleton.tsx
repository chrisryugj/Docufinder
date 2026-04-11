import { memo } from "react";

interface SearchResultSkeletonProps {
  count?: number;
}

/** 검색 결과 스켈레톤 로더 — shimmer 효과 + 좌측 보더 */
export const SearchResultSkeleton = memo(function SearchResultSkeleton({
  count = 5,
}: SearchResultSkeletonProps) {
  return (
    <div className="space-y-0.5 content-column" aria-busy="true" aria-label="검색 결과 로딩 중">
      {Array.from({ length: count }, (_, i) => (
        <div
          key={i}
          className="rounded-md"
          style={{
            padding: "0.75rem 0.875rem",
            borderLeft: "3px solid var(--color-bg-subtle)",
            opacity: 1 - i * 0.08,
            animationDelay: `${i * 100}ms`,
          }}
        >
          {/* Row 1: 파일 아이콘 + 파일명 + 배지 */}
          <div className="flex items-center justify-between mb-2">
            <div className="flex items-center gap-2 flex-1">
              <div
                className="w-5 h-5 rounded skeleton-shimmer"
              />
              <div
                className="h-3.5 rounded skeleton-shimmer"
                style={{ width: `${140 + (i % 3) * 60}px` }}
              />
            </div>
            <div className="flex items-center gap-2">
              <div
                className="w-8 h-3 rounded skeleton-shimmer"
              />
              <div
                className="w-10 h-4 rounded skeleton-shimmer"
              />
            </div>
          </div>

          {/* Row 2: 본문 미리보기 2줄 */}
          <div className="space-y-1.5 ml-7">
            <div
              className="h-3 rounded skeleton-shimmer"
              style={{ width: "100%" }}
            />
            <div
              className="h-3 rounded skeleton-shimmer"
              style={{ width: `${70 + (i % 4) * 8}%` }}
            />
          </div>

          {/* Row 3: 경로 */}
          <div className="mt-2 ml-7">
            <div
              className="h-2.5 rounded skeleton-shimmer"
              style={{ width: `${50 + (i % 3) * 15}%`, opacity: 0.7 }}
            />
          </div>
        </div>
      ))}
    </div>
  );
});
