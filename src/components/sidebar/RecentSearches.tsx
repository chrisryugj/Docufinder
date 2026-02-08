import { memo } from "react";
import type { RecentSearch } from "../../types/search";
import { formatRelativeTime } from "../../utils/formatRelativeTime";

interface RecentSearchesProps {
  searches: RecentSearch[];
  onSelect: (query: string) => void;
  onRemove: (query: string) => void;
}

/**
 * 최근 검색어 목록
 */
export const RecentSearches = memo(function RecentSearches({
  searches,
  onSelect,
  onRemove,
}: RecentSearchesProps) {
  if (searches.length === 0) {
    return (
      <div
        className="text-sm py-2 px-3"
        style={{ color: "var(--color-sidebar-muted)" }}
      >
        최근 검색 기록이 없습니다
      </div>
    );
  }

  return (
    <div>
      <ul className="space-y-0.5" role="list" aria-label="최근 검색어">
        {searches.map((search, index) => (
          <li key={`${search.query}-${index}`}>
            <div className="group flex items-center gap-2 px-3 py-2 mx-2 rounded-lg transition-all duration-200 cursor-pointer" style={{ ["--hover-bg" as string]: "var(--color-sidebar-hover)" }}>
              {/* 검색 아이콘 */}
              <svg
                className="w-3.5 h-3.5 flex-shrink-0 transition-colors"
                style={{ color: "var(--color-sidebar-muted)" }}
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
                aria-hidden="true"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
                />
              </svg>

              {/* 검색어 */}
              <button
                onClick={() => onSelect(search.query)}
                className="flex-1 text-left text-sm truncate transition-colors"
                style={{ color: "var(--color-sidebar-text)" }}
                title={search.query}
              >
                {search.query}
              </button>

              {/* 시간 배지 */}
              <span
                className="text-[10px] px-1.5 py-0.5 rounded flex-shrink-0"
                style={{ backgroundColor: "var(--color-sidebar-hover)", color: "var(--color-sidebar-muted)" }}
                title={new Date(search.timestamp).toLocaleString("ko-KR")}
              >
                {formatRelativeTime(search.timestamp)}
              </span>

              {/* 삭제 버튼 */}
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  onRemove(search.query);
                }}
                className="opacity-0 group-hover:opacity-100 p-1 rounded transition-all duration-200 scale-90 hover:scale-100"
                style={{ color: "var(--color-sidebar-muted)" }}
                aria-label={`"${search.query}" 검색 기록 삭제`}
              >
                <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                </svg>
              </button>
            </div>
          </li>
        ))}
      </ul>
    </div>
  );
});
