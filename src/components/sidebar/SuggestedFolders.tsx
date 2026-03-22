import { useState, useEffect, useCallback, useRef } from "react";
import { createPortal } from "react-dom";
import { invoke } from "@tauri-apps/api/core";
import { invokeWithTimeout, IPC_TIMEOUT } from "../../utils/invokeWithTimeout";
import { Tooltip } from "../ui/Tooltip";

interface SuggestedFolder {
  path: string;
  label: string;
  category: "known" | "drive";
  exists: boolean;
}

interface SuggestedFoldersProps {
  watchedFolders: string[];
  onAddFolder: (path: string) => void;
}

interface ContextMenuState {
  isOpen: boolean;
  x: number;
  y: number;
  folderPath: string;
}

export function SuggestedFolders({ watchedFolders, onAddFolder }: SuggestedFoldersProps) {
  const [isExpanded, setIsExpanded] = useState(false);
  const [folders, setFolders] = useState<SuggestedFolder[]>([]);
  const [contextMenu, setContextMenu] = useState<ContextMenuState>({
    isOpen: false,
    x: 0,
    y: 0,
    folderPath: "",
  });
  const contextMenuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    invokeWithTimeout<SuggestedFolder[]>("get_suggested_folders", undefined, IPC_TIMEOUT.SETTINGS)
      .then(setFolders)
      .catch(() => {});
  }, []);

  // 이미 등록된 경로인지 체크 (정규화해서 비교)
  const isRegistered = useCallback(
    (path: string) => {
      const normalize = (p: string) =>
        p.replace(/\\\\\?\\/, "").replace(/\\/g, "/").toLowerCase().replace(/\/$/, "");
      const normalizedPath = normalize(path);
      return watchedFolders.some((wp) => normalize(wp) === normalizedPath);
    },
    [watchedFolders]
  );

  // 컨텍스트 메뉴 열기
  const handleContextMenu = (e: React.MouseEvent, folderPath: string) => {
    e.preventDefault();
    e.stopPropagation();
    setContextMenu({ isOpen: true, x: e.clientX, y: e.clientY, folderPath });
  };

  const closeContextMenu = () => {
    setContextMenu((prev) => ({ ...prev, isOpen: false }));
  };

  // 탐색기에서 열기
  const handleOpenFolder = async () => {
    const path = contextMenu.folderPath;
    closeContextMenu();
    try {
      await invoke("open_folder", { path });
    } catch {
      // 폴더 열기 실패 무시
    }
  };

  // 폴더 추가
  const handleAddFolder = () => {
    const path = contextMenu.folderPath;
    closeContextMenu();
    onAddFolder(path);
  };

  // 컨텍스트 메뉴 위치 경계 보정
  useEffect(() => {
    if (contextMenu.isOpen && contextMenuRef.current) {
      const rect = contextMenuRef.current.getBoundingClientRect();
      const padding = 8;
      let { x, y } = contextMenu;
      if (x + rect.width > window.innerWidth - padding) {
        x = Math.max(padding, window.innerWidth - rect.width - padding);
      }
      if (y + rect.height > window.innerHeight - padding) {
        y = Math.max(padding, window.innerHeight - rect.height - padding);
      }
      if (x !== contextMenu.x || y !== contextMenu.y) {
        setContextMenu((prev) => ({ ...prev, x, y }));
      }
    }
  }, [contextMenu.isOpen, contextMenu.x, contextMenu.y]);

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
  }, [contextMenu.isOpen]);

  const unregistered = folders.filter((f) => !isRegistered(f.path));

  // 모두 등록됐으면 숨기기
  if (unregistered.length === 0) return null;

  return (
    <>
      {/* 접기/펴기 헤더 */}
      <div className="flex items-center gap-2 mx-3 mt-3 mb-1">
        <div className="flex-1 border-t border-dashed" style={{ borderColor: "var(--color-sidebar-border)" }} />
        <button
          onClick={() => setIsExpanded(!isExpanded)}
          className="flex items-center gap-1 text-[11px] font-medium uppercase tracking-wider shrink-0 hover-sidebar-section"
          style={{ opacity: 0.7 }}
        >
          <svg
            className={`w-3 h-3 transition-transform duration-150 ${isExpanded ? "rotate-90" : ""}`}
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
          </svg>
          추천 ({unregistered.length})
        </button>
        <div className="flex-1 border-t border-dashed" style={{ borderColor: "var(--color-sidebar-border)" }} />
      </div>

      {/* 추천 폴더 목록 */}
      {isExpanded && (
        <div className="space-y-0.5">
          {unregistered.map((folder) => (
            <FolderItem key={folder.path} folder={folder} onAdd={onAddFolder} onContextMenu={handleContextMenu} />
          ))}
        </div>
      )}

      {/* 컨텍스트 메뉴 */}
      {contextMenu.isOpen && createPortal(
        <div
          ref={contextMenuRef}
          className="fixed z-[9999] min-w-[160px] py-1 rounded-lg shadow-xl border"
          style={{
            left: contextMenu.x,
            top: contextMenu.y,
            backgroundColor: "var(--color-bg-secondary)",
            borderColor: "var(--color-border)",
          }}
        >
          <button
            onClick={handleAddFolder}
            className="ctx-menu-item w-full px-3 py-2 text-left text-sm flex items-center gap-2"
          >
            <svg className="w-4 h-4 clr-success" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4.5v15m7.5-7.5h-15" />
            </svg>
            폴더 추가
          </button>
          <button
            onClick={handleOpenFolder}
            className="ctx-menu-item w-full px-3 py-2 text-left text-sm flex items-center gap-2"
          >
            <svg className="w-4 h-4 clr-warning" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3.75 9.776c.112-.017.227-.026.344-.026h15.812c.117 0 .232.009.344.026m-16.5 0a2.25 2.25 0 00-1.883 2.542l.857 6a2.25 2.25 0 002.227 1.932H19.05a2.25 2.25 0 002.227-1.932l.857-6a2.25 2.25 0 00-1.883-2.542m-16.5 0V6A2.25 2.25 0 016 3.75h3.879a1.5 1.5 0 011.06.44l2.122 2.12a1.5 1.5 0 001.06.44H18A2.25 2.25 0 0120.25 9v.776" />
            </svg>
            탐색기에서 열기
          </button>
        </div>,
        document.body
      )}
    </>
  );
}

function FolderItem({
  folder,
  onAdd,
  onContextMenu,
}: {
  folder: SuggestedFolder;
  onAdd: (path: string) => void;
  onContextMenu: (e: React.MouseEvent, path: string) => void;
}) {
  const icon = folder.category === "drive" ? "\uD83D\uDCBE" : "\uD83D\uDCC1";

  return (
    <Tooltip content="클릭하여 이 폴더를 검색 대상에 추가합니다" position="right" delay={400} usePortal>
      <button
        onClick={() => onAdd(folder.path)}
        onContextMenu={(e) => onContextMenu(e, folder.path)}
        className="flex items-center gap-2 w-full px-2 py-1.5 text-sm rounded hover-sidebar-item transition-colors group"
        style={{ color: "var(--color-sidebar-muted)", opacity: 0.75 }}
        data-context-menu
      >
        <span className="text-[13px]">{icon}</span>
        <span className="truncate flex-1 text-left">{folder.label}</span>
        <svg
          className="w-3.5 h-3.5 opacity-0 group-hover:opacity-100 transition-opacity flex-shrink-0"
          fill="none"
          viewBox="0 0 24 24"
          strokeWidth={2}
          stroke="currentColor"
        >
          <path strokeLinecap="round" strokeLinejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
        </svg>
      </button>
    </Tooltip>
  );
}
