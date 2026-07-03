import { memo, useState } from "react";
import { Trash2, ChevronRight } from "lucide-react";
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
    <section className="pt-2.5 pb-3 mt-1 border-t" style={{ borderColor: "var(--color-sidebar-border)" }}>
      {/* 다른 섹션(인덱싱된 폴더·스마트 폴더·최근 검색) 헤더와 동일한 시각 언어 */}
      <div className="flex items-center justify-between px-1 pb-1 mb-1">
        <button
          onClick={() => setExpanded(!expanded)}
          className="flex items-center gap-1.5 text-xs font-semibold hover-sidebar-section"
          aria-expanded={expanded}
        >
          <ChevronRight
            className={`w-3.5 h-3.5 transition-transform duration-150 ${expanded ? "rotate-90" : ""}`}
          />
          북마크
          <span className="font-normal opacity-60">({bookmarks.length})</span>
        </button>
      </div>

      {expanded && (
        <div className="mt-1">
          {bookmarks.length === 0 ? (
            <p className="px-3 py-2 text-[11px]" style={{ color: "var(--color-sidebar-section)" }}>
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
                    className="p-0.5 rounded opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 hover:bg-[var(--color-bg-primary)] text-[var(--color-text-muted)] hover:text-[var(--color-error)] transition-all"
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
