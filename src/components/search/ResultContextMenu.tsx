import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

interface ContextMenuState {
  isOpen: boolean;
  x: number;
  y: number;
}

interface ResultContextMenuProps {
  filePath: string;
  folderPath: string;
  pageNumber?: number | null;
  onOpenFile: (filePath: string, page?: number | null) => void;
  onCopyPath?: (path: string) => void;
  onOpenFolder?: (path: string) => void;
}

/** 컨텍스트 메뉴 표시를 위한 훅 */
export function useContextMenu() {
  const [contextMenu, setContextMenu] = useState<ContextMenuState>({
    isOpen: false,
    x: 0,
    y: 0,
  });

  const handleContextMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();

    const menuWidth = 200;
    const menuHeight = 160;
    const padding = 8;

    let x = e.clientX;
    let y = e.clientY;

    if (x + menuWidth > window.innerWidth - padding) {
      x = window.innerWidth - menuWidth - padding;
    }
    if (y + menuHeight > window.innerHeight - padding) {
      y = window.innerHeight - menuHeight - padding;
    }

    setContextMenu({ isOpen: true, x, y });
  }, []);

  const closeContextMenu = useCallback(() => {
    setContextMenu((prev) => ({ ...prev, isOpen: false }));
  }, []);

  return { contextMenu, handleContextMenu, closeContextMenu };
}

/** 검색 결과 컨텍스트 메뉴 */
export function ResultContextMenu({
  filePath,
  folderPath,
  pageNumber,
  onOpenFile,
  onCopyPath,
  onOpenFolder,
  contextMenu,
  closeContextMenu,
}: ResultContextMenuProps & {
  contextMenu: ContextMenuState;
  closeContextMenu: () => void;
}) {
  const contextMenuRef = useRef<HTMLDivElement>(null);

  // 위치 경계 보정
  useEffect(() => {
    if (contextMenu.isOpen && contextMenuRef.current) {
      const menu = contextMenuRef.current;
      const rect = menu.getBoundingClientRect();
      const padding = 8;
      // 보정은 초기 위치 계산에서 처리됨
      if (rect.right > window.innerWidth - padding || rect.bottom > window.innerHeight - padding) {
        // 경계 초과 시 재배치는 useContextMenu에서 처리
      }
    }
  }, [contextMenu.isOpen]);

  // 외부 클릭 시 메뉴 닫기
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (contextMenuRef.current && !contextMenuRef.current.contains(e.target as Node)) {
        closeContextMenu();
      }
    };
    if (contextMenu.isOpen) {
      document.addEventListener("mousedown", handleClickOutside);
    }
    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
    };
  }, [contextMenu.isOpen, closeContextMenu]);

  if (!contextMenu.isOpen) return null;

  const menuItemStyle = { color: "var(--color-text-primary)" };
  const handleHover = (e: React.MouseEvent<HTMLButtonElement>) => {
    e.currentTarget.style.backgroundColor = "var(--color-accent-light)";
    e.currentTarget.style.color = "var(--color-accent)";
  };
  const handleLeave = (e: React.MouseEvent<HTMLButtonElement>) => {
    e.currentTarget.style.backgroundColor = "transparent";
    e.currentTarget.style.color = "var(--color-text-primary)";
  };

  return createPortal(
    <div
      ref={contextMenuRef}
      role="menu"
      aria-label="파일 작업 메뉴"
      className="fixed min-w-[140px] py-1 rounded-lg shadow-xl border"
      style={{
        left: contextMenu.x,
        top: contextMenu.y,
        zIndex: 9999,
        backgroundColor: "var(--color-bg-secondary)",
        borderColor: "var(--color-border)",
      }}
    >
      <button
        role="menuitem"
        onClick={() => { closeContextMenu(); onOpenFile(filePath, pageNumber); }}
        className="w-full px-3 py-2 text-left text-sm flex items-center gap-2 transition-colors"
        style={menuItemStyle}
        onMouseEnter={handleHover}
        onMouseLeave={handleLeave}
      >
        <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14" />
        </svg>
        파일 열기
      </button>

      {onOpenFolder && (
        <button
          role="menuitem"
          onClick={() => { closeContextMenu(); onOpenFolder(folderPath); }}
          className="w-full px-3 py-2 text-left text-sm flex items-center gap-2 transition-colors"
          style={menuItemStyle}
          onMouseEnter={handleHover}
          onMouseLeave={handleLeave}
        >
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 19a2 2 0 01-2-2V7a2 2 0 012-2h4l2 2h4a2 2 0 012 2v1M5 19h14a2 2 0 002-2v-5a2 2 0 00-2-2H9a2 2 0 00-2 2v5a2 2 0 01-2 2z" />
          </svg>
          폴더 열기
        </button>
      )}

      <button
        role="menuitem"
        onClick={() => {
          closeContextMenu();
          if (onCopyPath) { onCopyPath(filePath); } else { navigator.clipboard.writeText(filePath); }
        }}
        className="w-full px-3 py-2 text-left text-sm flex items-center gap-2 transition-colors"
        style={menuItemStyle}
        onMouseEnter={handleHover}
        onMouseLeave={handleLeave}
      >
        <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M8 5H6a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2v-1M8 5a2 2 0 002 2h2a2 2 0 002-2M8 5a2 2 0 012-2h2a2 2 0 012 2m0 0h2a2 2 0 012 2v3m2 4H10m0 0l3-3m-3 3l3 3" />
        </svg>
        경로 복사
      </button>
    </div>,
    document.body
  );
}
