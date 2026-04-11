import { memo, useState, useRef, useEffect, useCallback, useMemo } from "react";
import { createPortal } from "react-dom";
import { cleanPath } from "../../utils/cleanPath";

interface ScopeChipProps {
  watchedFolders: string[];
  searchScope: string | null;
  onSearchScopeChange: (scope: string | null) => void;
}

/** 경로에서 마지막 폴더명만 추출 */
function folderDisplayName(path: string): string {
  const cleaned = cleanPath(path).replace(/[/\\]$/, "");
  const parts = cleaned.split(/[/\\]/);
  return parts[parts.length - 1] || cleaned;
}

/** 전체 경로 표시용 (백슬래시 통일) */
function folderFullPath(path: string): string {
  return cleanPath(path).replace(/\//g, "\\").replace(/\\$/, "");
}

export const ScopeChip = memo(function ScopeChip({
  watchedFolders,
  searchScope,
  onSearchScopeChange,
}: ScopeChipProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [filter, setFilter] = useState("");
  const chipRef = useRef<HTMLButtonElement>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);
  const filterInputRef = useRef<HTMLInputElement>(null);

  // 폴더 1개 이하면 표시 불필요
  if (watchedFolders.length <= 1) return null;

  const displayLabel = searchScope ? folderDisplayName(searchScope) : "전체";

  const filteredFolders = useMemo(() => {
    if (!filter) return watchedFolders;
    const lower = filter.toLowerCase();
    return watchedFolders.filter((f) => folderFullPath(f).toLowerCase().includes(lower));
  }, [watchedFolders, filter]);

  // 열릴 때 검색 input 포커스
  useEffect(() => {
    if (isOpen) {
      setFilter("");
      setTimeout(() => filterInputRef.current?.focus(), 0);
    }
  }, [isOpen]);

  // 외부 클릭으로 닫기
  useEffect(() => {
    if (!isOpen) return;
    const handleClick = (e: MouseEvent) => {
      if (chipRef.current?.contains(e.target as Node)) return;
      if (dropdownRef.current?.contains(e.target as Node)) return;
      setIsOpen(false);
    };
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setIsOpen(false);
    };
    document.addEventListener("mousedown", handleClick);
    document.addEventListener("keydown", handleKey);
    return () => {
      document.removeEventListener("mousedown", handleClick);
      document.removeEventListener("keydown", handleKey);
    };
  }, [isOpen]);

  const handleSelect = useCallback((folder: string | null) => {
    onSearchScopeChange(folder);
    setIsOpen(false);
  }, [onSearchScopeChange]);

  // 드롭다운 위치 계산
  const [pos, setPos] = useState({ top: 0, left: 0, width: 320 });
  useEffect(() => {
    if (!isOpen || !chipRef.current) return;
    const rect = chipRef.current.getBoundingClientRect();
    const dropWidth = Math.min(360, window.innerWidth - 32);
    setPos({
      top: rect.bottom + 4,
      left: Math.max(8, Math.min(rect.left, window.innerWidth - dropWidth - 8)),
      width: dropWidth,
    });
  }, [isOpen]);

  return (
    <>
      <button
        ref={chipRef}
        onClick={() => setIsOpen(!isOpen)}
        className="flex items-center gap-1 px-2 py-0.5 rounded text-[11px] font-medium shrink-0 mr-1 transition-colors"
        style={{
          backgroundColor: searchScope ? "var(--color-accent-subtle)" : "var(--color-bg-tertiary)",
          color: searchScope ? "var(--color-accent)" : "var(--color-text-muted)",
          border: `1px solid ${searchScope ? "var(--color-accent-subtle)" : "var(--color-border)"}`,
        }}
        title={searchScope ? folderFullPath(searchScope) : "전체 폴더에서 검색"}
      >
        {/* 폴더 아이콘 */}
        <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <path d="M22 19a2 2 0 01-2 2H4a2 2 0 01-2-2V5a2 2 0 012-2h5l2 3h9a2 2 0 012 2z" />
        </svg>
        <span className="max-w-[80px] truncate">{displayLabel}</span>
        {/* 캐럿 */}
        <svg width="8" height="8" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round">
          <polyline points="6 9 12 15 18 9" />
        </svg>
      </button>

      {isOpen && createPortal(
        <div
          ref={dropdownRef}
          className="fixed z-[9999] rounded-lg overflow-hidden shadow-xl"
          style={{
            top: pos.top,
            left: pos.left,
            width: pos.width,
            backgroundColor: "var(--color-bg-secondary)",
            border: "1px solid var(--color-border)",
          }}
        >
          {/* 검색 입력 */}
          <div className="p-2" style={{ borderBottom: "1px solid var(--color-border)" }}>
            <div className="flex items-center gap-2 px-2 py-1.5 rounded-md" style={{ backgroundColor: "var(--color-bg-tertiary)" }}>
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ color: "var(--color-text-muted)", flexShrink: 0 }}>
                <circle cx="11" cy="11" r="8" />
                <path d="m21 21-4.35-4.35" />
              </svg>
              <input
                ref={filterInputRef}
                type="text"
                value={filter}
                onChange={(e) => setFilter(e.target.value)}
                placeholder="폴더 검색..."
                className="flex-1 bg-transparent border-none outline-none text-xs"
                style={{ color: "var(--color-text-primary)" }}
              />
            </div>
          </div>

          {/* 폴더 목록 */}
          <div className="max-h-[240px] overflow-y-auto py-1">
            {/* 전체 옵션 */}
            <button
              onClick={() => handleSelect(null)}
              className="w-full flex items-center gap-2 px-3 py-2 text-left text-xs transition-colors hover:bg-[var(--color-bg-tertiary)]"
              style={{ color: searchScope === null ? "var(--color-accent)" : "var(--color-text-secondary)" }}
            >
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ flexShrink: 0 }}>
                <path d="M22 19a2 2 0 01-2 2H4a2 2 0 01-2-2V5a2 2 0 012-2h5l2 3h9a2 2 0 012 2z" />
              </svg>
              <span className="font-medium">전체 폴더</span>
              {searchScope === null && (
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" className="ml-auto" style={{ color: "var(--color-accent)" }}>
                  <polyline points="20 6 9 17 4 12" />
                </svg>
              )}
            </button>

            {/* 개별 폴더 */}
            {filteredFolders.map((folder) => {
              const isSelected = searchScope === folder;
              const full = folderFullPath(folder);
              const name = folderDisplayName(folder);
              return (
                <button
                  key={folder}
                  onClick={() => handleSelect(folder)}
                  className="w-full flex items-center gap-2 px-3 py-2 text-left text-xs transition-colors hover:bg-[var(--color-bg-tertiary)]"
                  style={{ color: isSelected ? "var(--color-accent)" : "var(--color-text-secondary)" }}
                  title={full}
                >
                  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ flexShrink: 0 }}>
                    <path d="M22 19a2 2 0 01-2 2H4a2 2 0 01-2-2V5a2 2 0 012-2h5l2 3h9a2 2 0 012 2z" />
                  </svg>
                  <div className="flex-1 min-w-0">
                    <div className="font-medium truncate">{name}</div>
                    <div className="truncate opacity-50 text-[10px]">{full}</div>
                  </div>
                  {isSelected && (
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" className="ml-auto shrink-0" style={{ color: "var(--color-accent)" }}>
                      <polyline points="20 6 9 17 4 12" />
                    </svg>
                  )}
                </button>
              );
            })}

            {filteredFolders.length === 0 && (
              <div className="px-3 py-4 text-center text-xs" style={{ color: "var(--color-text-muted)" }}>
                일치하는 폴더가 없습니다
              </div>
            )}
          </div>
        </div>,
        document.body
      )}
    </>
  );
});
