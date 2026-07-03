import { memo } from "react";
import { FolderSearch } from "lucide-react";
import type { SmartFolder } from "../../hooks/useSmartFolders";

interface SmartFoldersProps {
  folders: SmartFolder[];
  onApply: (folder: SmartFolder) => void;
  onRemove: (id: string) => void;
}

/** 저장된 검색(스마트 폴더) 목록 — 클릭 시 해당 검색 재실행 */
export const SmartFolders = memo(function SmartFolders({
  folders,
  onApply,
  onRemove,
}: SmartFoldersProps) {
  if (folders.length === 0) {
    return (
      <div
        className="flex items-start gap-2 text-[11px] py-2 px-3 leading-relaxed"
        style={{ color: "var(--color-sidebar-section)" }}
      >
        <FolderSearch className="flex-shrink-0 w-3.5 h-3.5 mt-0.5" aria-hidden="true" />
        <span>자주 쓰는 검색 조건을 저장하면 여기서 한 번에 다시 실행할 수 있어요</span>
      </div>
    );
  }

  return (
    <ul className="space-y-0.5" role="list" aria-label="스마트 폴더">
      {folders.map((folder) => (
        <li key={folder.id}>
          <div
            className="group flex items-center gap-2 px-2 py-1.5 mx-1 rounded-lg cursor-pointer hover-sidebar-item"
            onClick={() => onApply(folder)}
          >
            <FolderSearch
              className="flex-shrink-0 w-3.5 h-3.5"
              style={{ color: "var(--color-sidebar-muted)" }}
              aria-hidden="true"
            />
            <span
              className="flex-1 text-left text-sm truncate"
              style={{ color: "var(--color-sidebar-text)" }}
              title={folder.name}
            >
              {folder.name}
            </span>
            <button
              onClick={(e) => {
                e.stopPropagation();
                onRemove(folder.id);
              }}
              className="hidden group-hover:flex flex-shrink-0 p-0.5 rounded hover-sidebar-danger"
              aria-label={`"${folder.name}" 스마트 폴더 삭제`}
            >
              <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </div>
        </li>
      ))}
    </ul>
  );
});
