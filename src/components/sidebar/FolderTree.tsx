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

  // 폴더 경로에서 이름만 추출
  const getFolderName = (path: string) => {
    const parts = path.replace(/\\/g, "/").split("/");
    return parts[parts.length - 1] || path;
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
      <div className="text-sm text-gray-500 py-2 px-3">
        등록된 폴더가 없습니다
      </div>
    );
  }

  return (
    <ul className="space-y-1" role="tree" aria-label="인덱싱된 폴더">
      {folders.map((folder) => {
        const isExpanded = expandedFolders.has(folder);
        return (
          <li key={folder} role="treeitem" aria-expanded={isExpanded}>
            <div
              className="group flex items-center gap-2 px-3 py-1.5 rounded-md hover:bg-gray-800/50 cursor-pointer transition-colors"
              onClick={() => toggleExpand(folder)}
            >
              {/* 폴더 아이콘 */}
              <svg
                className="w-4 h-4 text-yellow-500 flex-shrink-0"
                fill="currentColor"
                viewBox="0 0 20 20"
                aria-hidden="true"
              >
                <path d="M2 6a2 2 0 012-2h5l2 2h5a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z" />
              </svg>

              {/* 폴더 이름 */}
              <span
                className="flex-1 text-sm text-gray-300 truncate"
                title={folder}
              >
                {getFolderName(folder)}
              </span>

              {/* 삭제 버튼 */}
              {onRemoveFolder && (
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    onRemoveFolder(folder);
                  }}
                  className="opacity-0 group-hover:opacity-100 p-0.5 text-gray-500 hover:text-red-400 transition-opacity"
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
              <div className="ml-6 px-3 py-1 text-xs text-gray-500 break-all">
                {folder}
              </div>
            )}
          </li>
        );
      })}
    </ul>
  );
}
