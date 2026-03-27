import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import { createPortal } from "react-dom";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Folder, Star, Loader2, ShieldCheck, FolderOpen, RefreshCw, Trash2 } from "lucide-react";
import { invokeWithTimeout, IPC_TIMEOUT } from "../../utils/invokeWithTimeout";
import { formatRelativeTime } from "../../utils/formatRelativeTime";
import { cleanPath } from "../../utils/cleanPath";
import { logToBackend } from "../../utils/errorLogger";
import type { FolderStats, WatchedFolderInfo } from "../../types";

interface FolderTreeProps {
  folders: string[];
  onRemoveFolder?: (path: string) => void;
  onFoldersChange?: () => void; // 폴더 목록 갱신 콜백
  onReindexStart?: () => void; // 재인덱싱 시작 콜백
  isIndexing?: boolean; // 현재 인덱싱 중 여부
  isAutoIndexing?: React.RefObject<boolean>; // autoIndexAllDrives 실행 중 여부
}

interface ContextMenuState {
  isOpen: boolean;
  x: number;
  y: number;
  folderPath: string;
}

/**
 * 인덱싱된 폴더 목록 표시
 */
export function FolderTree({ folders, onRemoveFolder, onFoldersChange, onReindexStart, isIndexing, isAutoIndexing }: FolderTreeProps) {
  const [expandedFolders, setExpandedFolders] = useState<Set<string>>(
    new Set()
  );
  const [folderStats, setFolderStats] = useState<Record<string, FolderStats>>(
    {}
  );
  const [folderInfo, setFolderInfo] = useState<Record<string, WatchedFolderInfo>>(
    {}
  );
  const [contextMenu, setContextMenu] = useState<ContextMenuState>({
    isOpen: false,
    x: 0,
    y: 0,
    folderPath: "",
  });
  const contextMenuRef = useRef<HTMLDivElement>(null);

  // 자동 재인덱싱 트리거 추적 (중복 방지)
  const resumedRef = useRef<Set<string>>(new Set());

  // 통계 요청 카운터 (stale 응답 방지)
  const statsRequestIdRef = useRef(0);

  // 폴더 정보 조회 (즐겨찾기 포함)
  const fetchFolderInfo = useCallback(async () => {
    try {
      const infos = await invokeWithTimeout<WatchedFolderInfo[]>("get_folders_with_info", undefined, IPC_TIMEOUT.SETTINGS);
      const infoMap: Record<string, WatchedFolderInfo> = {};
      for (const info of infos) {
        infoMap[info.path] = info;
      }
      setFolderInfo(infoMap);
    } catch (e) {
      logToBackend("error", "Failed to get folder info", String(e), "FolderTree");
    }
  }, []);

  // 폴더 통계 배치 조회 (N+1 IPC 방지: 단일 호출로 전체 폴더 통계)
  const fetchStats = useCallback(async () => {
    if (folders.length === 0) return;

    const requestId = ++statsRequestIdRef.current;

    try {
      const allStats = await invokeWithTimeout<Record<string, FolderStats>>(
        "get_all_folder_stats", undefined, IPC_TIMEOUT.SETTINGS
      );
      // stale 응답 무시 (이후 요청이 들어온 경우)
      if (requestId === statsRequestIdRef.current) {
        setFolderStats(allStats);
      }
    } catch (e) {
      logToBackend("error", "Failed to get folder stats", String(e), "FolderTree");
    }
  }, [folders]);

  useEffect(() => {
    fetchStats();
    fetchFolderInfo();
  }, [folders, fetchFolderInfo, fetchStats]);

  // 증분 인덱싱 완료 시 통계 새로고침 — ref 패턴으로 listener를 한 번만 등록
  const fetchStatsRef = useRef(fetchStats);
  useEffect(() => { fetchStatsRef.current = fetchStats; });
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen<number>("incremental-index-updated", () => {
      fetchStatsRef.current();
    }).then((fn) => { unlisten = fn; });
    return () => { unlisten?.(); };
  }, []);

  // 미완료 폴더 자동 재인덱싱 (앱 재시작 시)
  useEffect(() => {
    if (isIndexing) return; // 이미 인덱싱 중이면 스킵
    if (isAutoIndexing?.current) return; // autoIndexAllDrives 실행 중이면 스킵

    const incompleteFolders = Object.entries(folderInfo)
      .filter(([path, info]) => (info.indexing_status === "indexing" || info.indexing_status === "cancelled") && !resumedRef.current.has(path))
      .map(([path]) => path);

    if (incompleteFolders.length === 0) return;

    const resumeIndexing = async () => {
      for (const path of incompleteFolders) {
        resumedRef.current.add(path);
        console.info(`Resuming incomplete indexing: ${path}`);
        try {
          onReindexStart?.();
          await invoke("resume_indexing", { path });
          onFoldersChange?.();
        } catch (e) {
          logToBackend("error", `Failed to resume indexing for ${path}`, String(e), "FolderTree");
        }
      }
      // 완료 후 정보 새로고침
      fetchFolderInfo();
    };

    resumeIndexing();
  }, [folderInfo, isIndexing, onReindexStart, onFoldersChange, fetchFolderInfo]);

  // 즐겨찾기 토글 (컨텍스트 메뉴용)
  const handleToggleFavorite = async () => {
    const path = contextMenu.folderPath;
    closeContextMenu();
    try {
      await invokeWithTimeout("toggle_favorite", { path }, IPC_TIMEOUT.SETTINGS);
      await fetchFolderInfo();
      onFoldersChange?.();
    } catch (err) {
      logToBackend("error", "Failed to toggle favorite", String(err), "FolderTree");
    }
  };

  // 컨텍스트 메뉴 열기
  const handleContextMenu = (e: React.MouseEvent, folderPath: string) => {
    e.preventDefault();
    e.stopPropagation();
    setContextMenu({
      isOpen: true,
      x: e.clientX,
      y: e.clientY,
      folderPath,
    });
  };

  // 컨텍스트 메뉴 닫기
  const closeContextMenu = () => {
    setContextMenu((prev) => ({ ...prev, isOpen: false }));
  };

  // 재인덱싱 실행
  const handleReindex = async () => {
    const path = contextMenu.folderPath;
    closeContextMenu();
    onReindexStart?.();
    try {
      await invoke("reindex_folder", { path });
      onFoldersChange?.();
    } catch (err) {
      logToBackend("error", "Failed to reindex folder", String(err), "FolderTree");
    }
  };

  // 컨텍스트 메뉴 위치 경계 보정
  useEffect(() => {
    if (contextMenu.isOpen && contextMenuRef.current) {
      const menu = contextMenuRef.current;
      const rect = menu.getBoundingClientRect();
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

  // 폴더 정렬: 즐겨찾기 먼저 (folders/folderInfo 변경 시에만 재계산)
  const sortedFolders = useMemo(() => [...folders].sort((a, b) => {
    const aFav = folderInfo[a]?.is_favorite ? 1 : 0;
    const bFav = folderInfo[b]?.is_favorite ? 1 : 0;
    return bFav - aFav;
  }), [folders, folderInfo]);

  // 폴더 경로에서 이름만 추출
  const getFolderName = useCallback((path: string) => {
    const cleaned = cleanPath(path);
    const parts = cleaned.replace(/\\/g, "/").split("/");
    return parts[parts.length - 1] || cleaned;
  }, []);

  const toggleExpand = (path: string) => {
    setExpandedFolders((prev) => {
      const next = new Set(prev);
      if (next.has(path)) {
        next.delete(path);
      } else {
        next.add(path);
      }
      return next;
    });
  };

  // 모든 폴더가 드라이브 루트인지 감지 (전체 PC 인덱싱 모드)
  const isDriveRoot = (p: string) => /^([A-Za-z]:\\?|\\\\?\?\\[A-Za-z]:\\?)$/.test(p.replace(/[\\/]+$/, "").replace(/^\\\\\?\\/, ""));
  const isFullPcMode = folders.length > 0 && folders.every(isDriveRoot);
  const totalIndexed = isFullPcMode
    ? Object.values(folderStats).reduce((sum, s) => sum + s.indexed_count, 0)
    : 0;
  const driveLetters = isFullPcMode
    ? folders.map((f) => f.replace(/^\\\\\?\\/, "").charAt(0).toUpperCase()).sort().join(", ")
    : "";

  if (folders.length === 0) {
    return (
      <div
        className="text-sm py-2 px-3"
        style={{ color: "var(--color-sidebar-muted)" }}
      >
        등록된 폴더가 없습니다
      </div>
    );
  }

  // 전체 PC 인덱싱 모드: 요약 표시
  if (isFullPcMode) {
    return (
      <div className="px-3 py-2 space-y-1.5">
        <div className="flex items-center gap-2">
          <ShieldCheck className="w-4 h-4 flex-shrink-0" style={{ color: "var(--color-success)" }} />
          <span className="text-sm font-medium" style={{ color: "var(--color-sidebar-text)" }}>
            전체 PC 인덱싱
          </span>
        </div>
        <div className="text-xs space-y-0.5 pl-6" style={{ color: "var(--color-sidebar-muted)" }}>
          <div>드라이브: {driveLetters}</div>
          {totalIndexed > 0 && <div>{totalIndexed.toLocaleString()}개 문서</div>}
        </div>
      </div>
    );
  }

  return (
    <>
    <ul className="space-y-1" role="tree" aria-label="인덱싱된 폴더">
      {sortedFolders.map((folder) => {
        const isExpanded = expandedFolders.has(folder);
        const displayPath = cleanPath(folder);
        const isFavorite = folderInfo[folder]?.is_favorite ?? false;
        return (
          <li key={folder} role="treeitem" aria-expanded={isExpanded}>
            <div
              className="group flex items-center gap-1.5 px-2 py-1.5 mx-1 rounded-lg cursor-pointer transition-all duration-200 hover-sidebar-item"
              onClick={() => toggleExpand(folder)}
              onContextMenu={(e) => handleContextMenu(e, folder)}
              data-context-menu
            >
              {/* 즐겨찾기 + 폴더 아이콘 (하나로 통합) */}
              <div className="relative flex-shrink-0">
                <Folder
                  className={`w-4 h-4 transition-transform duration-200 ${isExpanded ? "rotate-90" : ""}`}
                  style={{ color: isExpanded ? "var(--color-warning)" : "var(--color-sidebar-muted)" }}
                  fill="currentColor"
                  aria-hidden="true"
                />
                {/* 즐겨찾기 표시 (별) */}
                {isFavorite && (
                  <Star className="absolute -top-1 -right-1 w-2.5 h-2.5 clr-favorite" fill="currentColor" />
                )}
              </div>

              {/* 폴더 이름 */}
              <span
                className="flex-1 text-sm truncate font-medium"
                title={displayPath}
              >
                {getFolderName(folder)}
              </span>

              {/* 인덱싱 미완료 표시 */}
              {folderInfo[folder]?.indexing_status === "indexing" && (
                <span className="flex items-center gap-1 px-1.5 py-0.5 text-[10px] font-medium rounded flex-shrink-0" style={{ backgroundColor: "var(--color-warning-bg)", color: "var(--color-warning)" }} title="인덱싱 미완료 - 자동 재개 중">
                  <Loader2 className="w-3 h-3 animate-spin" />
                  재개중
                </span>
              )}

              {/* 파일 수 배지 */}
              {folderStats[folder] && folderInfo[folder]?.indexing_status !== "indexing" && (
                <span
                  className="px-1.5 py-0.5 text-xs font-medium rounded flex-shrink-0"
                  style={{ backgroundColor: "var(--color-sidebar-hover)", color: "var(--color-sidebar-muted)" }}
                >
                  {folderStats[folder].indexed_count}
                </span>
              )}
            </div>

            {/* 상세 정보 (확장 시) */}
            {isExpanded && (
              <div
                className="ml-9 mr-2 px-3 py-2 my-1 text-xs rounded space-y-0.5"
                style={{ backgroundColor: "var(--color-sidebar-hover)", color: "var(--color-sidebar-muted)" }}
              >
                <div className="break-all">{displayPath}</div>
                {folderStats[folder] && (
                  <div>
                    {folderStats[folder].indexed_count} 문서 / {folderStats[folder].file_count} 파일
                  </div>
                )}
                {folderStats[folder]?.last_indexed && (
                  <div>
                    인덱싱 {formatRelativeTime(folderStats[folder].last_indexed * 1000, true)}
                  </div>
                )}
              </div>
            )}
          </li>
        );
      })}

    </ul>
    {/* 컨텍스트 메뉴 - Portal로 body에 렌더링 (사이드바 overflow 회피) */}
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
        role="menu"
        aria-label="폴더 메뉴"
      >
        {/* 즐겨찾기 토글 */}
        <button
          role="menuitem"
          onClick={handleToggleFavorite}
          className={`w-full px-3 py-2 text-left text-sm flex items-center gap-2 ctx-menu-item-favorite ${folderInfo[contextMenu.folderPath]?.is_favorite ? "ctx-menu-item-favorite--active" : ""}`}
        >
          <Star className="w-4 h-4" fill={folderInfo[contextMenu.folderPath]?.is_favorite ? "currentColor" : "none"} />
          {folderInfo[contextMenu.folderPath]?.is_favorite ? "즐겨찾기 해제" : "즐겨찾기 추가"}
        </button>
        {/* 탐색기에서 열기 */}
        <button
          onClick={async () => {
            const path = contextMenu.folderPath;
            closeContextMenu();
            try { await invoke("open_folder", { path }); } catch (err) { logToBackend("error", "Failed to open folder", String(err), "FolderTree"); }
          }}
          role="menuitem"
          className="ctx-menu-item w-full px-3 py-2 text-left text-sm flex items-center gap-2"
        >
          <FolderOpen className="w-4 h-4 clr-warning" />
          탐색기에서 열기
        </button>
        {/* 재인덱싱 */}
        <button
          role="menuitem"
          onClick={handleReindex}
          className="ctx-menu-item w-full px-3 py-2 text-left text-sm flex items-center gap-2"
        >
          <RefreshCw className="w-4 h-4 clr-info" />
          재인덱싱
        </button>
        {onRemoveFolder && (
          <button
            onClick={() => {
              const path = contextMenu.folderPath;
              closeContextMenu();
              onRemoveFolder(path);
            }}
            role="menuitem"
            className="ctx-menu-item-danger w-full px-3 py-2 text-left text-sm flex items-center gap-2"
          >
            <Trash2 className="w-4 h-4" />
            폴더 제거
          </button>
        )}
      </div>,
      document.body
    )}
    </>
  );
}
