import { useState } from "react";

interface FolderTreeProps {
  folders: string[];
  onRemoveFolder?: (path: string) => void;
}

/**
 * 인덱싱된 폴더 목록 표시
 */
export function FolderTree({ folders, onRemoveFolder }: FolderTreeProps) {
  const [expandedFolders, setExpandedFolders] = useState<Set<string>>(
    new Set()
  );

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
      {folders.map((folder) => {
        const isExpanded = expandedFolders.has(folder);
        const displayPath = cleanPath(folder);
        return (
          <li key={folder} role="treeitem" aria-expanded={isExpanded}>
            <div
              className="group flex items-center gap-3 px-3 py-2 mx-2 rounded-lg cursor-pointer transition-all duration-200 hover:bg-white/10 text-slate-400 hover:text-white"
              onClick={() => toggleExpand(folder)}
            >
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

            {/* 전체 경로 (확장 시) */}
            {isExpanded && (
              <div
                className="ml-9 mr-2 px-3 py-2 my-1 text-[11px] rounded bg-black/20 text-slate-500 break-all font-mono"
              >
                {displayPath}
              </div>
            )}
          </li>
        );
      })}
    </ul>
  );
}
