import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { formatRelativeTime } from "../../utils/formatRelativeTime";
import type { FolderStats, WatchedFolderInfo } from "../../types";

interface FolderTreeProps {
  folders: string[];
  onRemoveFolder?: (path: string) => void;
  onFoldersChange?: () => void; // 폴더 목록 갱신 콜백
}

/**
 * 인덱싱된 폴더 목록 표시
 */
export function FolderTree({ folders, onRemoveFolder, onFoldersChange }: FolderTreeProps) {
  const [expandedFolders, setExpandedFolders] = useState<Set<string>>(
    new Set()
  );
  const [folderStats, setFolderStats] = useState<Record<string, FolderStats>>(
    {}
  );
  const [folderInfo, setFolderInfo] = useState<Record<string, WatchedFolderInfo>>(
    {}
  );

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
    const fetchStats = async () => {
      const stats: Record<string, FolderStats> = {};
      for (const folder of folders) {
        try {
          const result = await invoke<FolderStats>("get_folder_stats", {
            path: folder,
          });
          stats[folder] = result;
        } catch (e) {
          console.error(`Failed to get stats for ${folder}:`, e);
        }
      }
      setFolderStats(stats);
    };

    if (folders.length > 0) {
      fetchStats();
      fetchFolderInfo();
    }
  }, [folders, fetchFolderInfo]);

  // 즐겨찾기 토글
  const handleToggleFavorite = async (path: string, e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      await invoke("toggle_favorite", { path });
      await fetchFolderInfo();
      onFoldersChange?.();
    } catch (err) {
      console.error("Failed to toggle favorite:", err);
    }
  };

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
              className="group flex items-center gap-2 px-3 py-2 mx-2 rounded-lg cursor-pointer transition-all duration-200 hover:bg-white/10 text-slate-400 hover:text-white"
              onClick={() => toggleExpand(folder)}
            >
              {/* 즐겨찾기 버튼 */}
              <button
                onClick={(e) => handleToggleFavorite(folder, e)}
                className={`p-0.5 rounded transition-all ${
                  isFavorite
                    ? "text-yellow-400"
                    : "text-slate-600 opacity-0 group-hover:opacity-100 hover:text-yellow-400"
                }`}
                aria-label={isFavorite ? "즐겨찾기 해제" : "즐겨찾기 추가"}
              >
                <svg className="w-3.5 h-3.5" fill={isFavorite ? "currentColor" : "none"} stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M11.049 2.927c.3-.921 1.603-.921 1.902 0l1.519 4.674a1 1 0 00.95.69h4.915c.969 0 1.371 1.24.588 1.81l-3.976 2.888a1 1 0 00-.363 1.118l1.518 4.674c.3.922-.755 1.688-1.538 1.118l-3.976-2.888a1 1 0 00-1.176 0l-3.976 2.888c-.783.57-1.838-.197-1.538-1.118l1.518-4.674a1 1 0 00-.363-1.118l-3.976-2.888c-.784-.57-.38-1.81.588-1.81h4.914a1 1 0 00.951-.69l1.519-4.674z" />
                </svg>
              </button>

              {/* 폴더 아이콘 */}
              <svg
                className={`w-4 h-4 flex-shrink-0 transition-transform duration-200 ${isExpanded ? "rotate-90 text-yellow-500" : "text-slate-500 group-hover:text-yellow-400"}`}
                fill="currentColor"
                viewBox="0 0 20 20"
                aria-hidden="true"
              >
                <path d="M2 6a2 2 0 012-2h5l2 2h5a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z" />
              </svg>

              {/* 폴더 이름 */}
              <span
                className="flex-1 text-sm truncate font-medium"
                title={displayPath}
              >
                {getFolderName(folder)}
              </span>

              {/* 파일 수 배지 */}
              {folderStats[folder] && (
                <span className="px-1.5 py-0.5 text-[10px] font-medium rounded bg-white/10 text-slate-400">
                  {folderStats[folder].file_count}
                </span>
              )}

              {/* 삭제 버튼 - Hover시 드러남 */}
              {onRemoveFolder && (
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    onRemoveFolder(folder);
                  }}
                  className="opacity-0 group-hover:opacity-100 p-1 rounded hover:bg-red-500/20 text-slate-500 hover:text-red-400 transition-all duration-200"
                  aria-label={`${getFolderName(folder)} 폴더 제거`}
                >
                  <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                  </svg>
                </button>
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
    </ul>
  );
}
