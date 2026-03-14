import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import { createPortal } from "react-dom";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { invokeWithTimeout, IPC_TIMEOUT } from "../../utils/invokeWithTimeout";
import { formatRelativeTime } from "../../utils/formatRelativeTime";
import { cleanPath } from "../../utils/cleanPath";
import type { FolderStats, WatchedFolderInfo } from "../../types";

interface FolderTreeProps {
  folders: string[];
  onRemoveFolder?: (path: string) => void;
  onFoldersChange?: () => void; // 폴더 목록 갱신 콜백
  onReindexStart?: () => void; // 재인덱싱 시작 콜백
  isIndexing?: boolean; // 현재 인덱싱 중 여부
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
export function FolderTree({ folders, onRemoveFolder, onFoldersChange, onReindexStart, isIndexing }: FolderTreeProps) {
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
      console.error("Failed to get folder info:", e);
    }
  }, []);

  // 폴더 통계 조회
  const fetchStats = useCallback(async () => {
    if (folders.length === 0) return;

    const requestId = ++statsRequestIdRef.current;

    const entries = await Promise.all(
      folders.map(async (folder) => {
        try {
          const result = await invokeWithTimeout<FolderStats>("get_folder_stats", {
            path: folder,
          }, IPC_TIMEOUT.SETTINGS);
          return [folder, result] as const;
        } catch (e) {
          console.error(`Failed to get stats for ${folder}:`, e);
          return null;
        }
      })
    );
    // stale 응답 무시 (이후 요청이 들어온 경우)
    if (requestId === statsRequestIdRef.current) {
      const stats: Record<string, FolderStats> = {};
      for (const entry of entries) {
        if (entry) stats[entry[0]] = entry[1];
      }
      setFolderStats(stats);
    }
  }, [folders]);

  useEffect(() => {
    fetchStats();
    fetchFolderInfo();
  }, [folders, fetchFolderInfo, fetchStats]);

  // 증분 인덱싱 완료 시 통계 새로고침
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen<number>("incremental-index-updated", () => {
      fetchStats();
    }).then((fn) => { unlisten = fn; });
    return () => { unlisten?.(); };
  }, [fetchStats]);

  // 미완료 폴더 자동 재인덱싱 (앱 재시작 시)
  useEffect(() => {
    if (isIndexing) return; // 이미 인덱싱 중이면 스킵

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
          console.error(`Failed to resume indexing for ${path}:`, e);
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
      console.error("Failed to toggle favorite:", err);
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
      console.error("Failed to reindex folder:", err);
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
          <svg className="w-4 h-4 flex-shrink-0" style={{ color: "var(--color-success)" }} fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
          </svg>
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
                <svg
                  className={`w-4 h-4 transition-transform duration-200 ${isExpanded ? "rotate-90" : ""}`}
                  style={{ color: isExpanded ? "var(--color-warning)" : "var(--color-sidebar-muted)" }}
                  fill="currentColor"
                  viewBox="0 0 20 20"
                  aria-hidden="true"
                >
                  <path d="M2 6a2 2 0 012-2h5l2 2h5a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z" />
                </svg>
                {/* 즐겨찾기 표시 (별) */}
                {isFavorite && (
                  <svg className="absolute -top-1 -right-1 w-2.5 h-2.5 text-yellow-400" fill="currentColor" viewBox="0 0 24 24">
                    <path d="M11.049 2.927c.3-.921 1.603-.921 1.902 0l1.519 4.674a1 1 0 00.95.69h4.915c.969 0 1.371 1.24.588 1.81l-3.976 2.888a1 1 0 00-.363 1.118l1.518 4.674c.3.922-.755 1.688-1.538 1.118l-3.976-2.888a1 1 0 00-1.176 0l-3.976 2.888c-.783.57-1.838-.197-1.538-1.118l1.518-4.674a1 1 0 00-.363-1.118l-3.976-2.888c-.784-.57-.38-1.81.588-1.81h4.914a1 1 0 00.951-.69l1.519-4.674z" />
                  </svg>
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
                <span className="flex items-center gap-1 px-1.5 py-0.5 text-[10px] font-medium rounded bg-amber-500/20 text-amber-400 flex-shrink-0" title="인덱싱 미완료 - 자동 재개 중">
                  <svg className="w-3 h-3 animate-spin" fill="none" viewBox="0 0 24 24">
                    <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                    <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
                  </svg>
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
                <div className="break-all font-mono">{displayPath}</div>
                {folderStats[folder] && (
                  <div style={{ color: "var(--color-sidebar-text)" }}>
                    {folderStats[folder].indexed_count} 문서 / {folderStats[folder].file_count} 파일
                  </div>
                )}
                {folderStats[folder]?.last_indexed && (
                  <div>
                    마지막 인덱싱 {formatRelativeTime(folderStats[folder].last_indexed * 1000)}
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
          <svg className="w-4 h-4" fill={folderInfo[contextMenu.folderPath]?.is_favorite ? "currentColor" : "none"} stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M11.049 2.927c.3-.921 1.603-.921 1.902 0l1.519 4.674a1 1 0 00.95.69h4.915c.969 0 1.371 1.24.588 1.81l-3.976 2.888a1 1 0 00-.363 1.118l1.518 4.674c.3.922-.755 1.688-1.538 1.118l-3.976-2.888a1 1 0 00-1.176 0l-3.976 2.888c-.783.57-1.838-.197-1.538-1.118l1.518-4.674a1 1 0 00-.363-1.118l-3.976-2.888c-.784-.57-.38-1.81.588-1.81h4.914a1 1 0 00.951-.69l1.519-4.674z" />
          </svg>
          {folderInfo[contextMenu.folderPath]?.is_favorite ? "즐겨찾기 해제" : "즐겨찾기 추가"}
        </button>
        {/* 탐색기에서 열기 */}
        <button
          onClick={async () => {
            const path = contextMenu.folderPath;
            closeContextMenu();
            try { await invoke("open_folder", { path }); } catch (err) { console.error("Failed to open folder:", err); }
          }}
          role="menuitem"
          className="ctx-menu-item w-full px-3 py-2 text-left text-sm flex items-center gap-2"
        >
          <svg className="w-4 h-4 text-yellow-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3.75 9.776c.112-.017.227-.026.344-.026h15.812c.117 0 .232.009.344.026m-16.5 0a2.25 2.25 0 00-1.883 2.542l.857 6a2.25 2.25 0 002.227 1.932H19.05a2.25 2.25 0 002.227-1.932l.857-6a2.25 2.25 0 00-1.883-2.542m-16.5 0V6A2.25 2.25 0 016 3.75h3.879a1.5 1.5 0 011.06.44l2.122 2.12a1.5 1.5 0 001.06.44H18A2.25 2.25 0 0120.25 9v.776" />
          </svg>
          탐색기에서 열기
        </button>
        {/* 재인덱싱 */}
        <button
          role="menuitem"
          onClick={handleReindex}
          className="ctx-menu-item w-full px-3 py-2 text-left text-sm flex items-center gap-2"
        >
          <svg className="w-4 h-4 text-blue-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
          </svg>
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
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
            </svg>
            폴더 제거
          </button>
        )}
      </div>,
      document.body
    )}
    </>
  );
}
