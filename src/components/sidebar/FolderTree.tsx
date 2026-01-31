import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { formatRelativeTime } from "../../utils/formatRelativeTime";
import type { FolderStats, WatchedFolderInfo } from "../../types";

interface FolderTreeProps {
  folders: string[];
  onRemoveFolder?: (path: string) => void;
  onFoldersChange?: () => void; // 폴더 목록 갱신 콜백
  onReindexStart?: () => void; // 재인덱싱 시작 콜백
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
export function FolderTree({ folders, onRemoveFolder, onFoldersChange, onReindexStart }: FolderTreeProps) {
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

  // 폴더 정보 조회 (즐겨찾기 포함)
  const fetchFolderInfo = useCallback(async () => {
    try {
      const infos = await invoke<WatchedFolderInfo[]>("get_folders_with_info");
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
  useEffect(() => {
    if (folders.length === 0) return;

    let isMounted = true;

    const fetchStats = async () => {
      const stats: Record<string, FolderStats> = {};
      for (const folder of folders) {
        if (!isMounted) return;
        try {
          const result = await invoke<FolderStats>("get_folder_stats", {
            path: folder,
          });
          stats[folder] = result;
        } catch (e) {
          console.error(`Failed to get stats for ${folder}:`, e);
        }
      }
      if (isMounted) {
        setFolderStats(stats);
      }
    };

    fetchStats();
    fetchFolderInfo();

    return () => {
      isMounted = false;
    };
  }, [folders, fetchFolderInfo]);

  // 즐겨찾기 토글 (컨텍스트 메뉴용)
  const handleToggleFavorite = async () => {
    const path = contextMenu.folderPath;
    closeContextMenu();
    try {
      await invoke("toggle_favorite", { path });
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

  // 폴더 정렬: 즐겨찾기 먼저
  const sortedFolders = [...folders].sort((a, b) => {
    const aFav = folderInfo[a]?.is_favorite ? 1 : 0;
    const bFav = folderInfo[b]?.is_favorite ? 1 : 0;
    return bFav - aFav;
  });

  // Windows 정규화 prefix 제거 (\\?\)
  const cleanPath = (path: string) => {
    return path.replace(/^\\\\\?\\/, "");
  };

  // 폴더 경로에서 이름만 추출
  const getFolderName = (path: string) => {
    const cleaned = cleanPath(path);
    const parts = cleaned.replace(/\\/g, "/").split("/");
    return parts[parts.length - 1] || cleaned;
  };

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

  return (
    <ul className="space-y-1" role="tree" aria-label="인덱싱된 폴더">
      {sortedFolders.map((folder) => {
        const isExpanded = expandedFolders.has(folder);
        const displayPath = cleanPath(folder);
        const isFavorite = folderInfo[folder]?.is_favorite ?? false;
        return (
          <li key={folder} role="treeitem" aria-expanded={isExpanded}>
            <div
              className="group flex items-center gap-1.5 px-2 py-1.5 mx-1 rounded-lg cursor-pointer transition-all duration-200 hover:bg-white/10 text-slate-400 hover:text-white"
              onClick={() => toggleExpand(folder)}
              onContextMenu={(e) => handleContextMenu(e, folder)}
            >
              {/* 즐겨찾기 + 폴더 아이콘 (하나로 통합) */}
              <div className="relative flex-shrink-0">
                <svg
                  className={`w-4 h-4 transition-transform duration-200 ${isExpanded ? "rotate-90 text-yellow-500" : "text-slate-500 group-hover:text-yellow-400"}`}
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

              {/* 파일 수 배지 */}
              {folderStats[folder] && (
                <span className="px-1.5 py-0.5 text-[10px] font-medium rounded bg-white/10 text-slate-400 flex-shrink-0">
                  {folderStats[folder].file_count}
                </span>
              )}
            </div>

            {/* 상세 정보 (확장 시) */}
            {isExpanded && (
              <div className="ml-9 mr-2 px-3 py-2 my-1 text-[11px] rounded bg-black/20 text-slate-500 space-y-1">
                <div className="break-all font-mono">{displayPath}</div>
                {folderStats[folder] && (
                  <div className="flex items-center gap-2 text-slate-400">
                    <span>파일 {folderStats[folder].file_count}개</span>
                    {folderStats[folder].last_indexed && (
                      <>
                        <span className="text-slate-600">•</span>
                        <span>
                          마지막 인덱싱:{" "}
                          {formatRelativeTime(
                            folderStats[folder].last_indexed * 1000
                          )}
                        </span>
                      </>
                    )}
                  </div>
                )}
              </div>
            )}
          </li>
        );
      })}

      {/* 컨텍스트 메뉴 */}
      {contextMenu.isOpen && (
        <div
          ref={contextMenuRef}
          className="fixed z-50 min-w-[140px] py-1 rounded-lg shadow-xl border"
          style={{
            left: contextMenu.x,
            top: contextMenu.y,
            backgroundColor: "var(--color-bg-secondary)",
            borderColor: "var(--color-border)",
          }}
        >
          {/* 즐겨찾기 토글 */}
          <button
            onClick={handleToggleFavorite}
            className="w-full px-3 py-2 text-left text-sm flex items-center gap-2 transition-colors"
            style={{ color: folderInfo[contextMenu.folderPath]?.is_favorite ? "#facc15" : "var(--color-text-primary)" }}
            onMouseEnter={(e) => {
              e.currentTarget.style.backgroundColor = "rgba(250, 204, 21, 0.15)";
              e.currentTarget.style.color = "#facc15";
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.backgroundColor = "transparent";
              e.currentTarget.style.color = folderInfo[contextMenu.folderPath]?.is_favorite ? "#facc15" : "var(--color-text-primary)";
            }}
          >
            <svg className="w-4 h-4" fill={folderInfo[contextMenu.folderPath]?.is_favorite ? "currentColor" : "none"} stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M11.049 2.927c.3-.921 1.603-.921 1.902 0l1.519 4.674a1 1 0 00.95.69h4.915c.969 0 1.371 1.24.588 1.81l-3.976 2.888a1 1 0 00-.363 1.118l1.518 4.674c.3.922-.755 1.688-1.538 1.118l-3.976-2.888a1 1 0 00-1.176 0l-3.976 2.888c-.783.57-1.838-.197-1.538-1.118l1.518-4.674a1 1 0 00-.363-1.118l-3.976-2.888c-.784-.57-.38-1.81.588-1.81h4.914a1 1 0 00.951-.69l1.519-4.674z" />
            </svg>
            {folderInfo[contextMenu.folderPath]?.is_favorite ? "즐겨찾기 해제" : "즐겨찾기 추가"}
          </button>
          {/* 재인덱싱 */}
          <button
            onClick={handleReindex}
            className="w-full px-3 py-2 text-left text-sm flex items-center gap-2 transition-colors"
            style={{ color: "var(--color-text-primary)" }}
            onMouseEnter={(e) => {
              e.currentTarget.style.backgroundColor = "var(--color-accent-light)";
              e.currentTarget.style.color = "var(--color-accent)";
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.backgroundColor = "transparent";
              e.currentTarget.style.color = "var(--color-text-primary)";
            }}
          >
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
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
              className="w-full px-3 py-2 text-left text-sm flex items-center gap-2 transition-colors"
              style={{ color: "#f87171" }}
              onMouseEnter={(e) => {
                e.currentTarget.style.backgroundColor = "rgba(248, 113, 113, 0.2)";
                e.currentTarget.style.color = "#fca5a5";
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.backgroundColor = "transparent";
                e.currentTarget.style.color = "#f87171";
              }}
            >
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
              </svg>
              폴더 제거
            </button>
          )}
        </div>
      )}
    </ul>
  );
}
