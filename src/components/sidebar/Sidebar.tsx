import { FolderTree } from "./FolderTree";
import { RecentSearches } from "./RecentSearches";

interface SidebarProps {
  isOpen: boolean;
  onToggle: () => void;
  // 폴더 관련
  watchedFolders: string[];
  onRemoveFolder?: (path: string) => void;
  onAddFolder: () => void;
  // 최근 검색 관련
  recentSearches: string[];
  onSelectSearch: (query: string) => void;
  onRemoveSearch: (query: string) => void;
  onClearSearches: () => void;
}

/**
 * 사이드바 컴포넌트
 * - 인덱싱된 폴더 목록
 * - 최근 검색 기록
 */
export function Sidebar({
  isOpen,
  onToggle,
  watchedFolders,
  onRemoveFolder,
  onAddFolder,
  recentSearches,
  onSelectSearch,
  onRemoveSearch,
  onClearSearches,
}: SidebarProps) {
  return (
    <>
      {/* 사이드바 */}
      <aside
        className={`sidebar fixed left-0 top-0 h-full bg-gray-900 border-r border-gray-800 z-40 overflow-hidden
          ${isOpen ? "w-[var(--sidebar-width)]" : "w-0"}`}
        aria-label="사이드바"
        aria-hidden={!isOpen}
      >
        <div className="flex flex-col h-full w-[var(--sidebar-width)]">
          {/* 헤더 */}
          <div className="flex items-center justify-between px-4 py-3 border-b border-gray-800">
            <h2 className="text-sm font-medium text-gray-300">탐색</h2>
            <button
              onClick={onToggle}
              className="p-1 text-gray-500 hover:text-gray-300 rounded transition-colors"
              aria-label="사이드바 닫기"
            >
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M11 19l-7-7 7-7m8 14l-7-7 7-7" />
              </svg>
            </button>
          </div>

          {/* 콘텐츠 */}
          <div className="flex-1 overflow-y-auto">
            {/* 폴더 섹션 */}
            <section className="py-3">
              <div className="flex items-center justify-between px-4 mb-2">
                <h3 className="text-xs font-medium text-gray-500 uppercase tracking-wider">
                  인덱스 폴더
                </h3>
                <button
                  onClick={onAddFolder}
                  className="p-0.5 text-gray-500 hover:text-blue-400 rounded transition-colors"
                  aria-label="폴더 추가"
                >
                  <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
                  </svg>
                </button>
              </div>
              <FolderTree folders={watchedFolders} onRemoveFolder={onRemoveFolder} />
            </section>

            {/* 구분선 */}
            <div className="border-t border-gray-800 mx-4" />

            {/* 최근 검색 섹션 */}
            <section className="py-3">
              <h3 className="px-4 mb-2 text-xs font-medium text-gray-500 uppercase tracking-wider">
                최근 검색
              </h3>
              <RecentSearches
                searches={recentSearches}
                onSelect={onSelectSearch}
                onRemove={onRemoveSearch}
                onClear={onClearSearches}
              />
            </section>
          </div>

          {/* 푸터 - 단축키 힌트 */}
          <div className="px-4 py-2 border-t border-gray-800">
            <div className="flex items-center gap-2 text-xs text-gray-600">
              <kbd className="px-1.5 py-0.5 bg-gray-800 rounded text-gray-500">Ctrl+B</kbd>
              <span>사이드바 토글</span>
            </div>
          </div>
        </div>
      </aside>

      {/* 토글 버튼 (닫힌 상태에서만) */}
      {!isOpen && (
        <button
          onClick={onToggle}
          className="fixed left-4 top-20 z-30 p-2 bg-gray-800 hover:bg-gray-700 rounded-md shadow-lg transition-colors"
          aria-label="사이드바 열기"
        >
          <svg className="w-4 h-4 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 5l7 7-7 7M5 5l7 7-7 7" />
          </svg>
        </button>
      )}
    </>
  );
}
