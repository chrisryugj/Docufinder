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
          backgroundColor: "var(--color-bg-secondary)",
          borderRight: "1px solid var(--color-border)",
          boxShadow: isOpen ? "var(--shadow-xl)" : "none",
        }}
        aria-label="사이드바"
        aria-hidden={!isOpen}
      >
        <div className="flex flex-col h-full">
          {/* 헤더 */}
          <div
            className="flex items-center justify-between px-4 py-3 border-b"
            style={{ borderColor: "var(--color-border)" }}
          >
            <h2 className="text-sm font-semibold" style={{ color: "var(--color-text-secondary)" }}>
              탐색
            </h2>
            <button
              onClick={onToggle}
              className="p-1.5 rounded-lg transition-all duration-200"
              style={{ color: "var(--color-text-muted)" }}
              onMouseEnter={(e) => {
                e.currentTarget.style.backgroundColor = "var(--color-bg-tertiary)";
                e.currentTarget.style.color = "var(--color-text-primary)";
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.backgroundColor = "transparent";
                e.currentTarget.style.color = "var(--color-text-muted)";
              }}
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
            <section className="py-4">
              <div className="flex items-center justify-between px-4 mb-3">
                <h3
                  className="text-xs font-semibold uppercase tracking-wider"
                  style={{ color: "var(--color-text-muted)" }}
                >
                  인덱스 폴더
                </h3>
                <button
                  onClick={onAddFolder}
                  className="p-1 rounded transition-all duration-200"
                  style={{ color: "var(--color-text-muted)" }}
                  onMouseEnter={(e) => {
                    e.currentTarget.style.color = "var(--color-accent)";
                  }}
                  onMouseLeave={(e) => {
                    e.currentTarget.style.color = "var(--color-text-muted)";
                  }}
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
            <div className="mx-4" style={{ borderTop: "1px solid var(--color-border)" }} />

            {/* 최근 검색 섹션 */}
            <section className="py-4">
              <h3
                className="px-4 mb-3 text-xs font-semibold uppercase tracking-wider"
                style={{ color: "var(--color-text-muted)" }}
              >
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
          <div
            className="px-4 py-3 border-t"
            style={{ borderColor: "var(--color-border)" }}
          >
            <div className="flex items-center gap-2 text-xs" style={{ color: "var(--color-text-muted)" }}>
              <kbd
                className="px-1.5 py-0.5 rounded text-xs font-mono"
                style={{
                  backgroundColor: "var(--color-bg-tertiary)",
                  color: "var(--color-text-muted)",
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
          className="fixed left-4 top-20 z-30 p-2.5 rounded-lg transition-all duration-200 hover:scale-105"
          style={{
            backgroundColor: "var(--color-bg-secondary)",
            boxShadow: "var(--shadow-lg)",
            color: "var(--color-text-muted)",
          }}
          onMouseEnter={(e) => {
            e.currentTarget.style.backgroundColor = "var(--color-bg-tertiary)";
            e.currentTarget.style.color = "var(--color-text-primary)";
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.backgroundColor = "var(--color-bg-secondary)";
            e.currentTarget.style.color = "var(--color-text-muted)";
          }}
          aria-label="사이드바 열기"
        >
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 5l7 7-7 7M5 5l7 7-7 7" />
          </svg>
        </button>
      )}
    </>
  );
}
