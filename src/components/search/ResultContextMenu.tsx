import { lazy, Suspense, useCallback, useEffect, useRef, useState, useSyncExternalStore } from "react";
import { createPortal } from "react-dom";
import { ExternalLink, FolderOpen, ClipboardCopy, Search, GitCompare } from "lucide-react";

// 코드 스플리팅: 비교 모달은 '○○와 비교' 클릭 시에만 로딩 (LineageBadge 패턴)
const VersionDiffModal = lazy(() =>
  import("./VersionDiffModal").then((m) => ({ default: m.VersionDiffModal }))
);

interface ContextMenuState {
  isOpen: boolean;
  x: number;
  y: number;
}

/* ── 임의 두 문서 비교: '비교 대상으로 선택'된 기준 파일 공유 상태 ──
 * ResultContextMenu 는 SearchResultItem / GroupedSearchResultItem / FilenameResultItem /
 * AiAnswerPanel 등 여러 곳에서 독립 인스턴스로 렌더되므로, 기준 파일은 모듈 스코프
 * store + useSyncExternalStore 로 공유한다 (중간 컴포넌트 prop drilling 회피). */
interface CompareBase {
  path: string;
  name: string;
}

let compareBaseState: CompareBase | null = null;
const compareBaseListeners = new Set<() => void>();

function setCompareBase(next: CompareBase | null) {
  compareBaseState = next;
  compareBaseListeners.forEach((listener) => listener());
}

function subscribeCompareBase(listener: () => void) {
  compareBaseListeners.add(listener);
  return () => {
    compareBaseListeners.delete(listener);
  };
}

function useCompareBase(): CompareBase | null {
  return useSyncExternalStore(subscribeCompareBase, () => compareBaseState);
}

function baseNameOf(filePath: string): string {
  return filePath.split(/[\\/]/).pop() ?? filePath;
}

function truncateName(name: string, max = 18): string {
  return name.length > max ? `${name.slice(0, max)}…` : name;
}

interface ResultContextMenuProps {
  filePath: string;
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
    const menuHeight = 240;
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
  // 임의 두 문서 비교 (D4): 전역 기준 파일 + 이 인스턴스에서 연 비교 모달의 기준
  const compareBase = useCompareBase();
  const [diffBase, setDiffBase] = useState<CompareBase | null>(null);
  const isCompareBase = compareBase?.path === filePath;

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

  // 비교 모달은 메뉴가 닫힌 뒤에도 유지되어야 하므로 메뉴 open 여부와 무관하게 렌더
  const diffModal = diffBase ? (
    <Suspense fallback={null}>
      <VersionDiffModal
        aPath={diffBase.path}
        aName={diffBase.name}
        bPath={filePath}
        bName={baseNameOf(filePath)}
        title="문서 비교"
        onClose={() => setDiffBase(null)}
      />
    </Suspense>
  ) : null;

  if (!contextMenu.isOpen) return diffModal;

  const menu = createPortal(
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
        onClick={(e) => { e.stopPropagation(); closeContextMenu(); onOpenFile(filePath, pageNumber); }}
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
          onClick={(e) => { e.stopPropagation(); closeContextMenu(); onOpenFolder(filePath); }}
          className="ctx-menu-item w-full px-3 py-2 text-left text-sm flex items-center gap-2"
        >
          <FolderOpen className="w-4 h-4 clr-warning" />
          <span className="flex-1">파일 위치 열기</span>
        </button>
      )}

      <button
        role="menuitem"
        onClick={(e) => {
          e.stopPropagation();
          closeContextMenu();
          if (onCopyPath) { onCopyPath(filePath); } else { navigator.clipboard.writeText(filePath); }
        }}
        className="ctx-menu-item w-full px-3 py-2 text-left text-sm flex items-center gap-2"
      >
        <ClipboardCopy className="w-4 h-4 clr-success" />
        <span className="flex-1">경로 복사</span>
        <kbd className="text-[10px] font-mono opacity-40">Ctrl+C</kbd>
      </button>

      <div className="my-1 border-t" style={{ borderColor: "var(--color-border)" }} />
      <button
        role="menuitem"
        disabled={!onFindSimilar}
        onClick={(e) => { e.stopPropagation(); closeContextMenu(); onFindSimilar?.(filePath); }}
        className="ctx-menu-item w-full px-3 py-2 text-left text-sm flex items-center gap-2 disabled:opacity-40 disabled:cursor-not-allowed"
        title={onFindSimilar ? undefined : "시맨틱 검색을 켜면 사용할 수 있어요"}
      >
        <Search className="w-4 h-4 clr-info" />
        <span className="flex-1">유사 문서 찾기</span>
        {!onFindSimilar && <span className="text-[10px] opacity-60">시맨틱 OFF</span>}
      </button>

      {/* 임의 두 문서 비교 (D4): 기준 선택 → 다른 파일 우클릭 시 '○○와 비교' */}
      <div className="my-1 border-t" style={{ borderColor: "var(--color-border)" }} />
      {compareBase && !isCompareBase && (
        <button
          role="menuitem"
          onClick={(e) => {
            e.stopPropagation();
            closeContextMenu();
            setDiffBase(compareBase);
            setCompareBase(null);
          }}
          className="ctx-menu-item w-full px-3 py-2 text-left text-sm flex items-center gap-2"
          title={`"${compareBase.name}" ↔ "${baseNameOf(filePath)}" 내용 비교`}
        >
          <GitCompare className="w-4 h-4 clr-info" />
          <span className="flex-1 truncate">"{truncateName(compareBase.name)}"와 비교</span>
        </button>
      )}
      <button
        role="menuitem"
        onClick={(e) => {
          e.stopPropagation();
          closeContextMenu();
          setCompareBase(isCompareBase ? null : { path: filePath, name: baseNameOf(filePath) });
        }}
        className="ctx-menu-item w-full px-3 py-2 text-left text-sm flex items-center gap-2"
        title={
          isCompareBase
            ? "비교 기준 선택을 해제합니다"
            : "이 파일을 비교 기준으로 선택한 뒤, 다른 파일을 우클릭하면 '비교' 항목이 나타납니다"
        }
      >
        <GitCompare className="w-4 h-4 clr-warning" />
        <span className="flex-1">{isCompareBase ? "비교 대상 선택 해제" : "비교 대상으로 선택"}</span>
      </button>
    </div>,
    document.body
  );

  return (
    <>
      {menu}
      {diffModal}
    </>
  );
}
