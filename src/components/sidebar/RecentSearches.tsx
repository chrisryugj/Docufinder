interface RecentSearchesProps {
  searches: string[];
  onSelect: (query: string) => void;
  onRemove: (query: string) => void;
  onClear: () => void;
}

/**
 * 최근 검색어 목록
 */
export function RecentSearches({
  searches,
  onSelect,
  onRemove,
  onClear,
}: RecentSearchesProps) {
  if (searches.length === 0) {
    return (
      <div className="text-sm text-gray-500 py-2 px-3">
        최근 검색 기록이 없습니다
      </div>
    );
  }

  return (
    <div>
      <ul className="space-y-0.5" role="list" aria-label="최근 검색어">
        {searches.map((query, index) => (
          <li key={`${query}-${index}`}>
            <div className="group flex items-center gap-2 px-3 py-1.5 rounded-md hover:bg-gray-800/50 transition-colors">
              {/* 검색 아이콘 */}
              <svg
                className="w-3.5 h-3.5 text-gray-500 flex-shrink-0"
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
                className="flex-1 text-left text-sm text-gray-300 truncate hover:text-white"
                title={query}
              >
                {query}
              </button>

              {/* 삭제 버튼 */}
              <button
                onClick={() => onRemove(query)}
                className="opacity-0 group-hover:opacity-100 p-0.5 text-gray-500 hover:text-red-400 transition-opacity"
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

      {/* 전체 삭제 */}
      <button
        onClick={onClear}
        className="w-full mt-2 px-3 py-1.5 text-xs text-gray-500 hover:text-gray-400 text-left transition-colors"
      >
        전체 삭제
      </button>
    </div>
  );
}
