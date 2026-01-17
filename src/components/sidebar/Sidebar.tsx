import { useState } from "react";
import { FolderTree } from "./FolderTree";
import { RecentSearches } from "./RecentSearches";
import type { RecentSearch } from "../../types/search";

interface SidebarProps {
  isOpen: boolean;
  onToggle: () => void;
  // 폴더 관련
  watchedFolders: string[];
  onRemoveFolder?: (path: string) => void;
  onAddFolder: () => void;
  // 최근 검색 관련
  recentSearches: RecentSearch[];
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
  // 섹션 접기/펼치기 상태
  const [isFoldersExpanded, setIsFoldersExpanded] = useState(true);
  const [isSearchesExpanded, setIsSearchesExpanded] = useState(true);

  return (
    <>
      {/* 백드롭 (모바일/오버레이) */}
      {isOpen && (
        <div
          className="fixed inset-0 z-30 lg:hidden bg-black/50 backdrop-blur-sm transition-opacity"
          onClick={onToggle}
        />
      )}

      {/* 사이드바 */}
      <aside
        className={`fixed left-0 top-0 h-full z-40 overflow-hidden transition-all duration-300 ease-out flex flex-col
          ${isOpen ? "w-[var(--sidebar-width)] translate-x-0" : "w-[0px] -translate-x-full"}`}
        style={{
          backgroundColor: "var(--color-sidebar-bg)",
          borderRight: "1px solid var(--color-sidebar-border)",
          boxShadow: isOpen ? "var(--shadow-2xl)" : "none",
        }}
        aria-label="사이드바"
        aria-hidden={!isOpen}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-6 shrink-0">
          <h2 className="text-sm font-bold tracking-widest text-[#94A3B8] uppercase">
            메뉴
          </h2>
          <button
            onClick={onToggle}
            className="p-2 rounded-lg transition-all duration-200 text-[#64748B] hover:text-white hover:bg-white/10 active:scale-95"
            aria-label="사이드바 닫기"
          >
            <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M11 19l-7-7 7-7m8 14l-7-7 7-7" />
            </svg>
          </button>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto overflow-x-hidden px-3 py-2 space-y-8 scrollbar-thin scrollbar-thumb-slate-700 scrollbar-track-transparent">

          {/* Section: Indexed Folders */}
          <section>
            <div className="flex items-center justify-between px-2 mb-3">
              <button
                onClick={() => setIsFoldersExpanded(!isFoldersExpanded)}
                className="flex items-center gap-2 text-xs font-semibold text-[#64748B] uppercase tracking-wider hover:text-white transition-colors"
                aria-expanded={isFoldersExpanded}
              >
                <svg
                  className={`w-3 h-3 transition-transform duration-200 ${isFoldersExpanded ? "rotate-90" : ""}`}
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                </svg>
                인덱싱된 폴더
                <span className="text-[10px] font-normal text-slate-500">({watchedFolders.length})</span>
              </button>
              <button
                onClick={onAddFolder}
                className="p-1.5 rounded-md text-[#64748B] hover:text-white hover:bg-white/10 transition-all duration-200"
                aria-label="폴더 추가"
                title="폴더 추가"
              >
                <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
                </svg>
              </button>
            </div>

            {isFoldersExpanded && (
              <FolderTree
                folders={watchedFolders}
                onRemoveFolder={onRemoveFolder}
              />
            )}
          </section>

          {/* Section: Recent Searches */}
          <section>
            <div className="flex items-center justify-between px-2 mb-3">
              <button
                onClick={() => setIsSearchesExpanded(!isSearchesExpanded)}
                className="flex items-center gap-2 text-xs font-semibold text-[#64748B] uppercase tracking-wider hover:text-white transition-colors"
                aria-expanded={isSearchesExpanded}
              >
                <svg
                  className={`w-3 h-3 transition-transform duration-200 ${isSearchesExpanded ? "rotate-90" : ""}`}
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                </svg>
                최근 검색
                <span className="text-[10px] font-normal text-slate-500">({recentSearches.length})</span>
              </button>
              {recentSearches.length > 0 && (
                <button
                  onClick={onClearSearches}
                  className="p-1.5 rounded-md text-[#64748B] hover:text-red-400 hover:bg-white/10 transition-all duration-200"
                  aria-label="전체 삭제"
                  title="검색 기록 전체 삭제"
                >
                  <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
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

        {/* Footer - 저작권 */}
        <div className="p-4 border-t border-white/5 bg-black/20 shrink-0">
          <div className="text-center text-xs text-slate-500 space-y-0.5">
            <p>© 2025 개친절한 류주임</p>
            <p>광진구청 AI 동호회 (AI.Do)</p>
          </div>
        </div>
      </aside>


      {/* Floating Toggle Button (Visible only when sidebar is closed) */}
      {!isOpen && (
        <button
          onClick={onToggle}
          className="fixed left-6 top-4 z-50 p-2.5 rounded-xl bg-white/80 backdrop-blur-md shadow-[0_4px_12px_rgba(0,0,0,0.1)] border border-white/50 text-slate-600 hover:text-blue-600 hover:scale-105 active:scale-95 transition-all duration-300"
          aria-label="사이드바 열기"
        >
          <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2.5} d="M4 6h16M4 12h16M4 18h16" />
          </svg>
        </button>
      )}
    </>
  );
}
