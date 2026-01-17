interface RecentSearchesProps {
  searches: string[];
  onSelect: (query: string) => void;
  onRemove: (query: string) => void;
}

/**
 * 최근 검색어 목록
 */
export function RecentSearches({
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
        {searches.map((query, index) => (
          <li key={`${query}-${index}`}>
            <div className="group flex items-center gap-3 px-3 py-2 mx-2 rounded-lg transition-all duration-200 hover:bg-white/10 cursor-pointer">
              {/* 검색 아이콘 */}
              <svg
                className="w-3.5 h-3.5 flex-shrink-0 text-[#64748B] group-hover:text-blue-400 transition-colors"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
                aria-hidden="true"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z"
                />
              </svg>

              {/* 검색어 */}
              <button
                onClick={() => onSelect(query)}
                className="flex-1 text-left text-sm truncate text-slate-400 group-hover:text-white transition-colors"
                title={query}
              >
                {query}
              </button>

              {/* 삭제 버튼 */}
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  onRemove(query);
                }}
                className="opacity-0 group-hover:opacity-100 p-1 rounded hover:bg-white/10 text-slate-500 hover:text-red-400 transition-all duration-200 scale-90 hover:scale-100"
                aria-label={`"${query}" 검색 기록 삭제`}
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
}
