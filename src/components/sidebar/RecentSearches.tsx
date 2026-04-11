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
            <div
              className="group flex items-center gap-2 px-2 py-1.5 mx-1 rounded-lg cursor-pointer hover-sidebar-item"
              onClick={() => onSelect(search.query)}
            >
              {/* 불릿 */}
              <span
                className="flex-shrink-0 text-[8px] leading-none"
                style={{ color: "var(--color-sidebar-muted)" }}
                aria-hidden="true"
              >
                ·
              </span>

              {/* 검색어 */}
              <span
                className="flex-1 text-left text-sm truncate"
                style={{ color: "var(--color-sidebar-text)" }}
                title={search.query}
              >
                {search.query}
              </span>

              {/* 시간 표시 — 호버 시 숨김 */}
              <span
                className="text-[11px] flex-shrink-0 whitespace-nowrap group-hover:hidden"
                style={{ color: "var(--color-sidebar-muted)" }}
                title={new Date(search.timestamp).toLocaleString("ko-KR")}
              >
                {formatRelativeTime(search.timestamp, true)}
              </span>

              {/* 삭제 버튼 — 호버 시 표시 */}
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  onRemove(search.query);
                }}
                className="hidden group-hover:flex flex-shrink-0 p-0.5 rounded hover-sidebar-danger"
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
