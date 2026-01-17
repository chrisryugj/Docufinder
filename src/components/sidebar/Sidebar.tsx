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
      {/* 백드롭 (모바일/오버레이) */}
      {isOpen && (
        <div
          className="fixed inset-0 z-30 lg:hidden"
          style={{ backgroundColor: "rgba(0, 0, 0, 0.5)" }}
          onClick={onToggle}
        />
      )}

      {/* 사이드바 */}
      <aside
        className={`sidebar fixed left-0 top-0 h-full z-40 overflow-hidden transition-all duration-300 ease-out
          ${isOpen ? "w-[var(--sidebar-width)] translate-x-0" : "w-[var(--sidebar-width)] -translate-x-full"}`}
        style={{
          backgroundColor: "var(--color-sidebar-bg)",
          borderRight: "1px solid var(--color-sidebar-active)",
          boxShadow: isOpen ? "var(--shadow-xl)" : "none",
          color: "var(--color-sidebar-text)",
        }}
        aria-label="사이드바"
        aria-hidden={!isOpen}
      >
        <div className="flex flex-col h-full">
          {/* 헤더 */}
          <div
            className="flex items-center justify-between px-4 py-4 border-b"
            style={{ borderColor: "var(--color-sidebar-active)" }}
          >
            <h2 className="text-sm font-bold tracking-wide text-white opacity-90">
              탐색
            </h2>
            <button
              onClick={onToggle}
              className="p-1.5 rounded transition-colors duration-200"
              style={{ color: "var(--color-sidebar-muted)" }}
              onMouseEnter={(e) => {
                e.currentTarget.style.backgroundColor = "var(--color-sidebar-hover)";
                e.currentTarget.style.color = "white";
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.backgroundColor = "transparent";
                e.currentTarget.style.color = "var(--color-sidebar-muted)";
              }}
              aria-label="사이드바 닫기"
            >
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M11 19l-7-7 7-7m8 14l-7-7 7-7" />
              </svg>
            </button>
          </div>

          {/* 콘텐츠 */}
          <div className="flex-1 overflow-y-auto no-scrollbar">
            {/* 폴더 섹션 */}
            <section className="py-4">
              <div className="flex items-center justify-between px-4 mb-3">
                <h3
                  className="text-xs font-semibold uppercase tracking-wider opacity-70"
                  style={{ color: "var(--color-sidebar-text)" }}
                >
                  인덱스 폴더
                </h3>
                <button
                  onClick={onAddFolder}
                  className="p-1 rounded transition-colors duration-200"
                  style={{ color: "var(--color-sidebar-muted)" }}
                  onMouseEnter={(e) => {
                    e.currentTarget.style.backgroundColor = "var(--color-sidebar-hover)";
                    e.currentTarget.style.color = "white";
                  }}
                  onMouseLeave={(e) => {
                    e.currentTarget.style.backgroundColor = "transparent";
                    e.currentTarget.style.color = "var(--color-sidebar-muted)";
                  }}
                  aria-label="폴더 추가"
                >
                  <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
                  </svg>
                </button>
              </div>
              {/* Note: FolderTree might need text color adjustment via CSS or Props if it assumes light theme. 
                  Assuming it uses inherited colors or CSS variables which we might need to override globally or contextually.
                  For now, assuming it inherits color. 
              */}
              <div style={{ color: "var(--color-sidebar-text)" }}>
                <FolderTree folders={watchedFolders} onRemoveFolder={onRemoveFolder} />
              </div>
            </section>

            {/* 구분선 */}
            <div className="mx-4" style={{ borderTop: "1px solid var(--color-sidebar-active)" }} />

            {/* 최근 검색 섹션 */}
            <section className="py-4">
              <h3
                className="px-4 mb-3 text-xs font-semibold uppercase tracking-wider opacity-70"
                style={{ color: "var(--color-sidebar-text)" }}
              >
                최근 검색
              </h3>
              <div style={{ color: "var(--color-sidebar-text)" }}>
                <RecentSearches
                  searches={recentSearches}
                  onSelect={onSelectSearch}
                  onRemove={onRemoveSearch}
                  onClear={onClearSearches}
                />
              </div>
            </section>
          </div>

          {/* 푸터 - 단축키 힌트 */}
          <div
            className="px-4 py-4 border-t"
            style={{
              borderColor: "var(--color-sidebar-active)",
              backgroundColor: "rgba(0,0,0,0.2)"
            }}
          >
            <div className="flex items-center gap-2 text-xs" style={{ color: "var(--color-sidebar-muted)" }}>
              <kbd
                className="px-1.5 py-0.5 rounded text-[10px] font-mono border"
                style={{
                  backgroundColor: "var(--color-sidebar-hover)",
                  borderColor: "var(--color-sidebar-active)",
                  color: "var(--color-sidebar-text)",
                }}
              >
                Ctrl+B
              </kbd>
              <span>사이드바 토글</span>
            </div>
          </div>
        </div>
      </aside>

      {/* 토글 버튼 (닫힌 상태에서만) */}
      {!isOpen && (
        <button
          onClick={onToggle}
          className="fixed left-4 top-20 z-30 p-2.5 rounded-lg transition-all duration-200 hover:scale-105 shadow-md border"
          style={{
            backgroundColor: "white",
            borderColor: "var(--color-border)",
            color: "var(--color-text-secondary)",
          }}
          onMouseEnter={(e) => {
            e.currentTarget.style.backgroundColor = "var(--color-bg-secondary)";
            e.currentTarget.style.color = "var(--color-accent)";
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.backgroundColor = "white";
            e.currentTarget.style.color = "var(--color-text-secondary)";
          }}
          aria-label="사이드바 열기"
        >
          <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 5l7 7-7 7M5 5l7 7-7 7" />
          </svg>
        </button>
      )}
    </>
  );
}
