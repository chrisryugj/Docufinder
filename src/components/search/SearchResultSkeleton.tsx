import { memo } from "react";

interface SearchResultSkeletonProps {
  count?: number;
}

/** 검색 결과 스켈레톤 로더 — 실제 SearchResultItem의 카드 구조/스타일과 일치 */
export const SearchResultSkeleton = memo(function SearchResultSkeleton({
  count = 6,
}: SearchResultSkeletonProps) {
  return (
    <div className="space-y-1.5" aria-busy="true" aria-label="검색 결과 로딩 중">
      {Array.from({ length: count }, (_, i) => (
        <div
          key={i}
          className="result-card"
          style={{
            padding: "0.75rem 0.875rem",
            opacity: 1 - i * 0.08,
            animationDelay: `${i * 100}ms`,
          }}
        >
          {/* Row 1: 파일 아이콘 + 파일명 + (우측) 신뢰도 / 날짜 / 타입 뱃지 */}
          <div className="flex items-center justify-between mb-1.5">
            <div className="flex items-center gap-2 flex-1 min-w-0">
              <div className="w-4 h-4 rounded skeleton-shimmer shrink-0" />
              <div
                className="h-4 rounded skeleton-shimmer"
                style={{ width: `${180 + (i % 3) * 80}px` }}
              />
            </div>
            <div className="flex items-center gap-2 ml-2 shrink-0">
              <div className="h-3 w-7 rounded skeleton-shimmer" />
              <div className="h-3 w-10 rounded skeleton-shimmer" />
              <div className="h-4 w-10 rounded skeleton-shimmer" />
            </div>
          </div>

          {/* Row 2: 폴더 경로 (pl-6로 파일 아이콘 정렬 맞춤) */}
          <div className="pl-6">
            <div
              className="h-2.5 rounded skeleton-shimmer"
              style={{ width: `${45 + (i % 4) * 12}%`, opacity: 0.65 }}
            />
          </div>
        </div>
      ))}
    </div>
  );
});
