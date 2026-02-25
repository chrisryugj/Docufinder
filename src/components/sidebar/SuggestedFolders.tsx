import { useState, useEffect, useCallback } from "react";
import { invokeWithTimeout, IPC_TIMEOUT } from "../../utils/invokeWithTimeout";

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

export function SuggestedFolders({ watchedFolders, onAddFolder }: SuggestedFoldersProps) {
  const [folders, setFolders] = useState<SuggestedFolder[]>([]);
  const [isExpanded, setIsExpanded] = useState(true);

  useEffect(() => {
    invokeWithTimeout<SuggestedFolder[]>("get_suggested_folders", undefined, IPC_TIMEOUT.SETTINGS)
      .then(setFolders)
      .catch((err) => console.error("Failed to get suggested folders:", err));
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

  const knownFolders = folders.filter((f) => f.category === "known");
  const drives = folders.filter((f) => f.category === "drive");

  // 모두 등록됐으면 숨기기
  const hasUnregistered = folders.some((f) => !isRegistered(f.path));
  if (!hasUnregistered || folders.length === 0) return null;

  return (
    <div className="px-2 mb-2">
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className="flex items-center gap-1 w-full px-2 py-1 text-[11px] font-medium rounded hover:bg-white/5 transition-colors"
        style={{ color: "var(--color-text-muted)" }}
      >
        <svg
          className={`w-3 h-3 transition-transform ${isExpanded ? "rotate-90" : ""}`}
          fill="none"
          viewBox="0 0 24 24"
          strokeWidth={2}
          stroke="currentColor"
        >
          <path strokeLinecap="round" strokeLinejoin="round" d="M8.25 4.5l7.5 7.5-7.5 7.5" />
        </svg>
        빠른 추가
      </button>

      {isExpanded && (
        <div className="mt-1 space-y-0.5">
          {knownFolders
            .filter((f) => !isRegistered(f.path))
            .map((folder) => (
              <FolderItem key={folder.path} folder={folder} onAdd={onAddFolder} />
            ))}
          {drives
            .filter((f) => !isRegistered(f.path))
            .map((folder) => (
              <FolderItem key={folder.path} folder={folder} onAdd={onAddFolder} />
            ))}
        </div>
      )}
    </div>
  );
}

function FolderItem({
  folder,
  onAdd,
}: {
  folder: SuggestedFolder;
  onAdd: (path: string) => void;
}) {
  const icon = folder.category === "drive" ? "\uD83D\uDCBE" : "\uD83D\uDCC1";

  return (
    <button
      onClick={() => onAdd(folder.path)}
      className="flex items-center gap-2 w-full px-2 py-1.5 text-xs rounded hover:bg-white/5 transition-colors group"
      style={{ color: "var(--color-text-secondary)" }}
      title={folder.path}
    >
      <span className="text-[13px]">{icon}</span>
      <span className="truncate flex-1 text-left">{folder.label}</span>
      <svg
        className="w-3.5 h-3.5 opacity-0 group-hover:opacity-100 transition-opacity flex-shrink-0"
        fill="none"
        viewBox="0 0 24 24"
        strokeWidth={2}
        stroke="currentColor"
        style={{ color: "var(--color-text-muted)" }}
      >
        <path strokeLinecap="round" strokeLinejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
      </svg>
    </button>
  );
}
