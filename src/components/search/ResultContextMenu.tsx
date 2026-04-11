import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { ExternalLink, FolderOpen, ClipboardCopy, Search } from "lucide-react";

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
  onFindSimilar?: (filePath: string) => void;
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

    const menuWidth = 220;
    const menuHeight = 170;
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
  onFindSimilar,
  contextMenu,
  closeContextMenu,
}: ResultContextMenuProps & {
  contextMenu: ContextMenuState;
  closeContextMenu: () => void;
}) {
  const contextMenuRef = useRef<HTMLDivElement>(null);

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

  // 메뉴 열릴 때 첫 번째 아이템 포커스 + 키보드 내비게이션 + 닫힌 후 포커스 복귀
  useEffect(() => {
    if (!contextMenu.isOpen || !contextMenuRef.current) return;

    const menu = contextMenuRef.current;
    const items = () => menu.querySelectorAll<HTMLElement>('[role="menuitem"]');
    const triggerElement = document.activeElement as HTMLElement | null;

    // 첫 번째 아이템 포커스
    requestAnimationFrame(() => items()[0]?.focus());

    const restoreAndClose = () => {
      closeContextMenu();
      // 트리거 요소로 포커스 복귀
      requestAnimationFrame(() => triggerElement?.focus());
    };

    const handleKeyDown = (e: KeyboardEvent) => {
      const menuItems = items();
      const current = document.activeElement as HTMLElement;
      const idx = Array.from(menuItems).indexOf(current);

      switch (e.key) {
        case "ArrowDown": {
          e.preventDefault();
          const next = idx < menuItems.length - 1 ? idx + 1 : 0;
          menuItems[next]?.focus();
          break;
        }
        case "ArrowUp": {
          e.preventDefault();
          const prev = idx > 0 ? idx - 1 : menuItems.length - 1;
          menuItems[prev]?.focus();
          break;
        }
        case "Home":
          e.preventDefault();
          menuItems[0]?.focus();
          break;
        case "End":
          e.preventDefault();
          menuItems[menuItems.length - 1]?.focus();
          break;
        case "Escape":
          e.preventDefault();
          restoreAndClose();
          break;
        case "Tab":
          e.preventDefault();
          restoreAndClose();
          break;
      }
    };

    menu.addEventListener("keydown", handleKeyDown);
    return () => menu.removeEventListener("keydown", handleKeyDown);
  }, [contextMenu.isOpen, closeContextMenu]);

  if (!contextMenu.isOpen) return null;

  return createPortal(
    <div
      ref={contextMenuRef}
      role="menu"
      aria-label="파일 작업 메뉴"
      className="fixed min-w-[180px] py-1 rounded-lg shadow-xl border ctx-menu-animate"
      style={{
        left: contextMenu.x,
        top: contextMenu.y,
        zIndex: 9999,
        backgroundColor: "var(--color-bg-secondary)",
        borderColor: "var(--color-border)",
      }}
    >
      {/* 파일 열기 (Primary action) */}
      <button
        role="menuitem"
        onClick={() => { closeContextMenu(); onOpenFile(filePath, pageNumber); }}
        className="ctx-menu-item w-full px-3 py-2 text-left text-sm flex items-center gap-2"
      >
        <ExternalLink className="w-4 h-4 clr-info" />
        <span className="flex-1">파일 열기</span>
        <kbd className="text-[10px] font-mono opacity-40">Enter</kbd>
      </button>

      {/* 구분선 */}
      <div className="my-1 border-t" style={{ borderColor: "var(--color-border)" }} />

      {onOpenFolder && (
        <button
          role="menuitem"
          onClick={() => { closeContextMenu(); onOpenFolder(folderPath); }}
          className="ctx-menu-item w-full px-3 py-2 text-left text-sm flex items-center gap-2"
        >
          <FolderOpen className="w-4 h-4 clr-warning" />
          <span className="flex-1">폴더 열기</span>
        </button>
      )}

      <button
        role="menuitem"
        onClick={() => {
          closeContextMenu();
          if (onCopyPath) { onCopyPath(filePath); } else { navigator.clipboard.writeText(filePath); }
        }}
        className="ctx-menu-item w-full px-3 py-2 text-left text-sm flex items-center gap-2"
      >
        <ClipboardCopy className="w-4 h-4 clr-success" />
        <span className="flex-1">경로 복사</span>
        <kbd className="text-[10px] font-mono opacity-40">Ctrl+C</kbd>
      </button>

      {onFindSimilar && (
        <>
          <div className="my-1 border-t" style={{ borderColor: "var(--color-border)" }} />
          <button
            role="menuitem"
            onClick={() => { closeContextMenu(); onFindSimilar(filePath); }}
            className="ctx-menu-item w-full px-3 py-2 text-left text-sm flex items-center gap-2"
          >
            <Search className="w-4 h-4 clr-info" />
            <span className="flex-1">유사 문서 찾기</span>
          </button>
        </>
      )}
    </div>,
    document.body
  );
}
