import { memo, useState } from "react";
import { Bookmark, Trash2, ChevronDown, ChevronRight } from "lucide-react";
import { FileIcon } from "../ui/FileIcon";
import type { BookmarkInfo } from "../../hooks/useBookmarks";

interface BookmarkListProps {
  bookmarks: BookmarkInfo[];
  onSelect: (filePath: string, pageNumber?: number | null) => void;
  onRemove: (id: number) => void;
}

export const BookmarkList = memo(function BookmarkList({
  bookmarks,
  onSelect,
  onRemove,
}: BookmarkListProps) {
  const [expanded, setExpanded] = useState(true);

  return (
    <section className="pt-1 pb-3">
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-center gap-1.5 px-3 py-1.5 text-xs font-semibold tracking-wider uppercase text-[var(--color-text-muted)] hover:text-[var(--color-text-secondary)] transition-colors"
      >
        {expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
        <Bookmark size={12} />
        <span>북마크</span>
        {bookmarks.length > 0 && (
          <span className="ml-auto text-[10px] opacity-60">{bookmarks.length}</span>
        )}
      </button>

      {expanded && (
        <div className="mt-1">
          {bookmarks.length === 0 ? (
            <p className="px-4 py-2 text-[11px] text-[var(--color-text-muted)]">
              북마크가 없습니다
            </p>
          ) : (
            <ul className="space-y-0.5 px-1">
              {bookmarks.map((bm) => (
                <li
                  key={bm.id}
                  className="group flex items-center gap-1.5 px-2 py-1.5 rounded-md hover:bg-[var(--color-bg-tertiary)] cursor-pointer transition-colors"
                  onClick={() => onSelect(bm.file_path, bm.page_number)}
                  title={bm.note || bm.file_path}
                >
                  <FileIcon fileName={bm.file_name} size="sm" />
                  <div className="flex-1 min-w-0">
                    <p className="text-xs truncate text-[var(--color-text-primary)]">
                      {bm.file_name}
                    </p>
                    {bm.note && (
                      <p className="text-[10px] truncate text-[var(--color-text-muted)]">
                        {bm.note}
                      </p>
                    )}
                  </div>
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      onRemove(bm.id);
                    }}
                    className="p-0.5 rounded opacity-0 group-hover:opacity-100 hover:bg-[var(--color-bg-primary)] text-[var(--color-text-muted)] hover:text-[var(--color-error)] transition-all"
                    title="삭제"
                  >
                    <Trash2 size={11} />
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
    </section>
  );
});
