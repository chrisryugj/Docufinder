import { useState, memo } from "react";
import { ChevronLeft, ChevronRight, Plus, Clock, Folder, Trash2, Bookmark } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { FolderTree } from "./FolderTree";
import { RecentSearches } from "./RecentSearches";
import { SmartFolders } from "./SmartFolders";
import { SuggestedFolders } from "./SuggestedFolders";
import { BookmarkList } from "./BookmarkList";
import { DriveIndexingPanel } from "./DriveIndexingPanel";
import { Tooltip } from "../ui/Tooltip";
import type { RecentSearch } from "../../types/search";
import type { SmartFolder } from "../../hooks/useSmartFolders";
import type { BookmarkInfo } from "../../hooks/useBookmarks";
import type { BatchState } from "../../types/index";

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
  smartFolders?: SmartFolder[];
  onApplySmartFolder?: (folder: SmartFolder) => void;
  onRemoveSmartFolder?: (id: string) => void;
  onSaveSmartFolder?: () => void;
  currentQuery?: string;
  bookmarks?: BookmarkInfo[];
  onBookmarkSelect?: (filePath: string, pageNumber?: number | null) => void;
  onBookmarkRemove?: (id: number) => void;
  isAutoIndexing?: React.RefObject<boolean>;
  batch?: BatchState | null;
  onCancelBatch?: () => Promise<void> | void;
  onDismissBatch?: () => void;
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
  smartFolders = [],
  onApplySmartFolder,
  onRemoveSmartFolder,
  onSaveSmartFolder,
  currentQuery,
  bookmarks = [],
  onBookmarkSelect,
  onBookmarkRemove,
  isAutoIndexing,
  batch,
  onCancelBatch,
  onDismissBatch,
}: SidebarProps) {
  const hasFolders = watchedFolders.length > 0;
  const [isFoldersExpanded, setIsFoldersExpanded] = useState(true);
  const [isSearchesExpanded, setIsSearchesExpanded] = useState(true);
  const [isSmartFoldersExpanded, setIsSmartFoldersExpanded] = useState(true);

  return (
    <>
      {/* 데스크톱 앱이라 mobile backdrop 불필요 — Sidebar 는 App.tsx 의 paddingLeft 로
         항상 메인 컨텐츠를 밀어내는 push 모드. 이전엔 `lg:hidden bg-black/30` overlay 가
         viewport ≥1024px 에서만 숨도록 설계돼 있었으나, macOS WebKit 환경의 일부 viewport
         계산(타이틀바·신호등 영역 + Retina DPR)에서 tailwind `lg` breakpoint 적용이 어긋나
         1024px 근처 창 크기에서 메인 화면 전체가 30% 어둡게 깔리는 회귀 발생 (이슈 #22). */}

      {/* Sidebar */}
      <aside
        className="absolute left-0 top-0 h-full z-40 overflow-hidden transition-all duration-200 ease-out flex flex-col"
        style={{
          width: isOpen ? "var(--sidebar-width)" : "var(--sidebar-collapsed-width)",
          backgroundColor: "var(--color-sidebar-bg)",
          borderRight: "1px solid var(--color-sidebar-border)",
        }}
        aria-label="사이드바"
      >
        {/* Header */}
        <div className="flex items-center justify-end shrink-0" style={{ height: "44px", padding: isOpen ? "0 8px 0 12px" : "0 8px" }}>
          {isOpen ? (
            <button
              onClick={onToggle}
              className="p-1.5 rounded-md btn-icon-hover"
              aria-label="사이드바 축소"
            >
              <ChevronLeft className="w-4 h-4" style={{ color: "var(--color-sidebar-muted)" }} />
            </button>
          ) : (
            <button
              onClick={onToggle}
              className="w-full flex justify-center p-1.5 rounded-md btn-icon-hover"
              aria-label="사이드바 확장"
            >
              <ChevronRight className="w-4 h-4" style={{ color: "var(--color-sidebar-muted)" }} />
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
              <Plus className="w-4 h-4" style={{ color: "var(--color-sidebar-muted)" }} />
            </button>
            <button
              onClick={onToggle}
              className="p-2 rounded-md btn-icon-hover"
              title="최근 검색 (사이드바 펼치기)"
              aria-label="최근 검색"
            >
              <Clock className="w-4 h-4" style={{ color: "var(--color-sidebar-muted)" }} />
            </button>
            {bookmarks.length > 0 && (
              <button
                onClick={() => onBookmarkSelect?.(bookmarks[0]?.file_path, bookmarks[0]?.page_number)}
                className="p-2 rounded-md btn-icon-hover"
                title={`북마크 (${bookmarks.length})`}
                aria-label="북마크"
              >
                <Bookmark className="w-4 h-4" style={{ color: "var(--color-sidebar-muted)" }} />
              </button>
            )}
            {/* Folder count indicator */}
            {watchedFolders.length > 0 && (
              <div className="mt-1 flex flex-col items-center">
                <span
                  className="text-[10px] font-bold tabular-nums"
                  style={{ color: "var(--color-sidebar-muted)" }}
                >
                  {watchedFolders.length}
                </span>
                <Folder className="w-3.5 h-3.5" style={{ color: "var(--color-sidebar-muted)" }} />
              </div>
            )}
          </div>
        )}

        {/* Expanded content */}
        {isOpen && (
          <>
            <div className="flex-1 overflow-y-auto overflow-x-hidden px-3 py-1">
              {/* Batch indexing panel (전체 드라이브 인덱싱 진행 상태) */}
              {batch && (
                <DriveIndexingPanel
                  batch={batch}
                  onCancel={() => onCancelBatch?.()}
                  onDismiss={() => onDismissBatch?.()}
                />
              )}

              {/* Section: Folders */}
              <section className="pb-3" data-tour="sidebar-folders">
                <div
                  className="flex items-center justify-between px-1 pb-1 mb-1"
                >
                  <button
                    onClick={() => setIsFoldersExpanded(!isFoldersExpanded)}
                    className="flex items-center gap-1.5 text-xs font-semibold hover-sidebar-section"
                    aria-expanded={isFoldersExpanded}
                  >
                    <ChevronRight
                      className={`w-3.5 h-3.5 transition-transform duration-150 ${isFoldersExpanded ? "rotate-90" : ""}`}
                    />
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
                    <Plus className="w-3.5 h-3.5" />
                  </button>
                </div>

                {isFoldersExpanded && (
                  <>
                    {!hasFolders && (
                      <div className="px-2 py-4 text-center">
                        <Folder className="w-8 h-8 mx-auto mb-2 opacity-50" style={{ color: "var(--color-sidebar-muted)" }} />
                        <p className="text-xs mb-3 leading-relaxed" style={{ color: "var(--color-sidebar-muted)" }}>
                          검색하려면 먼저<br />폴더를 추가하세요
                        </p>
                        <button
                          onClick={onAddFolder}
                          className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-semibold text-white transition-opacity hover:opacity-90"
                          style={{ backgroundColor: "var(--color-accent)" }}
                        >
                          <Plus className="w-3.5 h-3.5" /> 폴더 추가
                        </button>
                      </div>
                    )}
                    <FolderTree
                      folders={watchedFolders}
                      onRemoveFolder={onRemoveFolder}
                      isIndexing={isIndexing}
                      isAutoIndexing={isAutoIndexing}
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

              {/* Section: Smart Folders / Recent — 폴더 있을 때만 (첫 사용자 빈 섹션 노이즈 제거) */}
              {hasFolders && (<>
              <section className="pt-1 pb-3">
                <div
                  className="flex items-center justify-between px-1 pb-1 mb-1"
                >
                  <button
                    onClick={() => setIsSmartFoldersExpanded(!isSmartFoldersExpanded)}
                    className="flex items-center gap-1.5 text-xs font-semibold hover-sidebar-section"
                    aria-expanded={isSmartFoldersExpanded}
                  >
                    <ChevronRight
                      className={`w-3.5 h-3.5 transition-transform duration-150 ${isSmartFoldersExpanded ? "rotate-90" : ""}`}
                    />
                    스마트 폴더
                    <span className="font-normal" style={{ color: "var(--color-sidebar-muted)" }}>
                      ({smartFolders.length})
                    </span>
                  </button>
                  {currentQuery?.trim() && onSaveSmartFolder && (
                    <button
                      onClick={onSaveSmartFolder}
                      className="p-1 rounded hover-sidebar-item"
                      aria-label="현재 검색을 스마트 폴더로 저장"
                      title="현재 검색을 스마트 폴더로 저장"
                    >
                      <Plus className="w-3.5 h-3.5" />
                    </button>
                  )}
                </div>

                {isSmartFoldersExpanded && (
                  <SmartFolders
                    folders={smartFolders}
                    onApply={onApplySmartFolder ?? (() => {})}
                    onRemove={onRemoveSmartFolder ?? (() => {})}
                  />
                )}
              </section>

              {/* Section: Recent Searches */}
              <section className="pt-1 pb-3">
                <div
                  className="flex items-center justify-between px-1 pb-1 mb-1"
                >
                  <button
                    onClick={() => setIsSearchesExpanded(!isSearchesExpanded)}
                    className="flex items-center gap-1.5 text-xs font-semibold hover-sidebar-section"
                    aria-expanded={isSearchesExpanded}
                  >
                    <ChevronRight
                      className={`w-3.5 h-3.5 transition-transform duration-150 ${isSearchesExpanded ? "rotate-90" : ""}`}
                    />
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
                      <Trash2 className="w-3 h-3" />
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
              </>)}

              {/* Section: Bookmarks */}
              {onBookmarkSelect && (
                <BookmarkList
                  bookmarks={bookmarks}
                  onSelect={onBookmarkSelect}
                  onRemove={onBookmarkRemove || (() => {})}
                />
              )}
            </div>

            {/* Footer — 우측 StatusBar(50px)와 높이 정합 */}
            <div
              className="px-3 shrink-0 flex items-center justify-center"
              style={{
                borderTop: "1px solid var(--color-sidebar-border)",
                height: "50px",
              }}
            >
              <Tooltip
                usePortal
                position="top"
                maxWidth={220}
                content={
                  <div className="flex flex-col gap-0.5 leading-tight">
                    <span>
                      <span style={{ color: "var(--color-text-muted)" }}>Developer</span>
                      <span className="mx-1 opacity-50">·</span>
                      딴짓하는 류주임
                    </span>
                    <span>
                      <span style={{ color: "var(--color-text-muted)" }}>Icon designer</span>
                      <span className="mx-1 opacity-50">·</span>
                      @nellyskykim
                    </span>
                  </div>
                }
              >
                <div
                  className="text-center leading-[1.35] select-none flex flex-col items-center"
                  style={{ color: "var(--color-sidebar-muted)" }}
                >
                  <p className="text-[10.5px] whitespace-nowrap">
                    &copy; 2025&ndash;2026 딴짓하는 류주임
                  </p>
                  <a
                    href="#"
                    onClick={(e) => {
                      e.preventDefault();
                      invoke("open_url", { url: "https://www.threads.net/@chris_gomdori" });
                    }}
                    className="text-[11px] font-semibold tracking-tight transition-colors hover:underline whitespace-nowrap"
                    style={{ color: "var(--color-accent)" }}
                    aria-label="Threads @chris_gomdori 열기"
                  >
                    @chris_gomdori
                  </a>
                </div>
              </Tooltip>
            </div>
          </>
        )}
      </aside>
    </>
  );
});
