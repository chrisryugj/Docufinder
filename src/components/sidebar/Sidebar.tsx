import { useState, memo } from "react";
import { FolderTree } from "./FolderTree";
import { RecentSearches } from "./RecentSearches";
import { SuggestedFolders } from "./SuggestedFolders";
import type { RecentSearch } from "../../types/search";

interface SidebarProps {
  isOpen: boolean;
  onToggle: () => void;
  watchedFolders: string[];
  onRemoveFolder?: (path: string) => void;
  onAddFolder: () => void;
  onAddFolderByPath?: (path: string) => void;
  isIndexing?: boolean;
  onFoldersChange?: () => void;
  recentSearches: RecentSearch[];
  onSelectSearch: (query: string) => void;
  onRemoveSearch: (query: string) => void;
  onClearSearches: () => void;
}

export const Sidebar = memo(function Sidebar({
  isOpen,
  onToggle,
  watchedFolders,
  onRemoveFolder,
  onAddFolder,
  onAddFolderByPath,
  isIndexing,
  onFoldersChange,
  recentSearches,
  onSelectSearch,
  onRemoveSearch,
  onClearSearches,
}: SidebarProps) {
  const [isFoldersExpanded, setIsFoldersExpanded] = useState(true);
  const [isSearchesExpanded, setIsSearchesExpanded] = useState(true);

  return (
    <>
      {/* Mobile backdrop */}
      {isOpen && (
        <div
          className="fixed inset-0 z-30 lg:hidden bg-black/30 transition-opacity"
          onClick={onToggle}
        />
      )}

      {/* Sidebar */}
      <aside
        className="fixed left-0 top-0 h-full z-40 overflow-hidden transition-all duration-200 ease-out flex flex-col"
        style={{
          width: isOpen ? "var(--sidebar-width)" : "var(--sidebar-collapsed-width)",
          backgroundColor: "var(--color-sidebar-bg)",
          borderRight: "1px solid var(--color-sidebar-border)",
        }}
        aria-label="사이드바"
      >
        {/* Header */}
        <div className="flex items-center justify-between shrink-0" style={{ height: "44px", padding: isOpen ? "0 12px 0 16px" : "0 8px" }}>
          {isOpen ? (
            <>
              <span
                className="text-xs font-bold tracking-[0.1em] uppercase"
                style={{ color: "var(--color-sidebar-section)" }}
              >
                메뉴
              </span>
              <button
                onClick={onToggle}
                className="p-1.5 rounded-md btn-icon-hover"
                aria-label="사이드바 축소"
              >
                <svg className="w-4 h-4" style={{ color: "var(--color-sidebar-muted)" }} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M11 19l-7-7 7-7" />
                </svg>
              </button>
            </>
          ) : (
            <button
              onClick={onToggle}
              className="w-full flex justify-center p-1.5 rounded-md btn-icon-hover"
              aria-label="사이드바 확장"
            >
              <svg className="w-4 h-4" style={{ color: "var(--color-sidebar-muted)" }} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 5l7 7-7 7" />
              </svg>
            </button>
          )}
        </div>

        {/* Collapsed: icon-only buttons */}
        {!isOpen && (
          <div className="flex flex-col items-center gap-1 px-1 py-2">
            <button
              onClick={onAddFolder}
              className="p-2 rounded-md btn-icon-hover"
              title="폴더 추가"
              aria-label="폴더 추가"
            >
              <svg className="w-4 h-4" style={{ color: "var(--color-sidebar-muted)" }} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
              </svg>
            </button>
            <button
              onClick={() => onSelectSearch("")}
              className="p-2 rounded-md btn-icon-hover"
              title="최근 검색"
              aria-label="최근 검색"
            >
              <svg className="w-4 h-4" style={{ color: "var(--color-sidebar-muted)" }} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
            </button>
            {/* Folder count indicator */}
            {watchedFolders.length > 0 && (
              <div className="mt-1 flex flex-col items-center">
                <span
                  className="text-[10px] font-bold tabular-nums"
                  style={{ color: "var(--color-sidebar-muted)" }}
                >
                  {watchedFolders.length}
                </span>
                <svg className="w-3.5 h-3.5" style={{ color: "var(--color-sidebar-muted)" }} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
                </svg>
              </div>
            )}
          </div>
        )}

        {/* Expanded content */}
        {isOpen && (
          <>
            <div className="flex-1 overflow-y-auto overflow-x-hidden px-3 py-1">
              {/* Section: Folders */}
              <section className="pb-3">
                <div
                  className="flex items-center justify-between px-1 pb-1.5 mb-1.5"
                  style={{ borderBottom: "1px solid var(--color-sidebar-border)" }}
                >
                  <button
                    onClick={() => setIsFoldersExpanded(!isFoldersExpanded)}
                    className="flex items-center gap-1.5 text-xs font-semibold uppercase tracking-[0.06em] hover-sidebar-section"
                    aria-expanded={isFoldersExpanded}
                  >
                    <svg
                      className={`w-3.5 h-3.5 transition-transform duration-150 ${isFoldersExpanded ? "rotate-90" : ""}`}
                      fill="none"
                      stroke="currentColor"
                      viewBox="0 0 24 24"
                    >
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                    </svg>
                    인덱싱된 폴더
                    <span className="font-normal" style={{ color: "var(--color-sidebar-muted)" }}>
                      ({watchedFolders.length})
                    </span>
                  </button>
                  <button
                    onClick={onAddFolder}
                    className="p-1 rounded hover-sidebar-item"
                    aria-label="폴더 추가"
                    title="폴더 추가"
                  >
                    <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
                    </svg>
                  </button>
                </div>

                {isFoldersExpanded && (
                  <>
                    <FolderTree
                      folders={watchedFolders}
                      onRemoveFolder={onRemoveFolder}
                      isIndexing={isIndexing}
                      onFoldersChange={onFoldersChange}
                    />
                    {onAddFolderByPath && (
                      <SuggestedFolders
                        watchedFolders={watchedFolders}
                        onAddFolder={onAddFolderByPath}
                      />
                    )}
                  </>
                )}
              </section>

              {/* Section: Recent Searches */}
              <section className="pt-1 pb-3">
                <div
                  className="flex items-center justify-between px-1 pb-1.5 mb-1.5"
                  style={{ borderBottom: "1px solid var(--color-sidebar-border)" }}
                >
                  <button
                    onClick={() => setIsSearchesExpanded(!isSearchesExpanded)}
                    className="flex items-center gap-1.5 text-xs font-semibold uppercase tracking-[0.06em] hover-sidebar-section"
                    aria-expanded={isSearchesExpanded}
                  >
                    <svg
                      className={`w-3.5 h-3.5 transition-transform duration-150 ${isSearchesExpanded ? "rotate-90" : ""}`}
                      fill="none"
                      stroke="currentColor"
                      viewBox="0 0 24 24"
                    >
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                    </svg>
                    최근 검색
                    <span className="font-normal" style={{ color: "var(--color-sidebar-muted)" }}>
                      ({recentSearches.length})
                    </span>
                  </button>
                  {recentSearches.length > 0 && (
                    <button
                      onClick={onClearSearches}
                      className="p-1 rounded hover-sidebar-danger"
                      aria-label="전체 삭제"
                      title="검색 기록 전체 삭제"
                    >
                      <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                      </svg>
                    </button>
                  )}
                </div>

                {isSearchesExpanded && (
                  <RecentSearches
                    searches={recentSearches}
                    onSelect={onSelectSearch}
                    onRemove={onRemoveSearch}
                  />
                )}
              </section>
            </div>

            {/* Footer */}
            <div
              className="px-3 py-3 shrink-0"
              style={{
                borderTop: "1px solid var(--color-sidebar-border)",
              }}
            >
              <div
                className="text-center text-xs space-y-0.5"
                style={{ color: "var(--color-sidebar-muted)" }}
              >
                <p>&copy; 2025&ndash;{new Date().getFullYear()} Chris Ryu</p>
                <p>AI.Do · 서울특별시 광진구청</p>
              </div>
            </div>
          </>
        )}
      </aside>
    </>
  );
});
