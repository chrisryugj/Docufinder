import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { MapPin, Folder } from "lucide-react";
import { formatRelativeTime } from "../../utils/formatRelativeTime";
import { cleanPath } from "../../utils/cleanPath";
import type { SearchResult } from "../../types/search";

interface Props {
  copies: SearchResult[];
  currentFilePath: string;
  onOpenFile: (filePath: string) => void;
}

/**
 * 같은 파일명의 다른 경로 복사본 뱃지.
 * 클릭 시 portal popover로 경로 목록 표시 (LineageBadge와 유사, lineage 개념 아님).
 */
export function FilenameCopiesBadge({ copies, currentFilePath, onOpenFile }: Props) {
  const [isOpen, setIsOpen] = useState(false);
  const [pos, setPos] = useState<{ top: number; left: number }>({ top: 0, left: 0 });
  const triggerRef = useRef<HTMLButtonElement>(null);
  const popoverRef = useRef<HTMLDivElement>(null);

  const toggleOpen = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    if (!isOpen && triggerRef.current) {
      const rect = triggerRef.current.getBoundingClientRect();
      const POPOVER_MAX_WIDTH = 520;
      const POPOVER_MAX_HEIGHT = 360;
      const MARGIN = 12;
      const maxLeft = window.innerWidth - POPOVER_MAX_WIDTH - MARGIN;
      const left = Math.max(MARGIN, Math.min(rect.left, maxLeft));
      const top =
        rect.bottom + POPOVER_MAX_HEIGHT + MARGIN > window.innerHeight
          ? Math.max(MARGIN, rect.top - POPOVER_MAX_HEIGHT - 4)
          : rect.bottom + 4;
      setPos({ top, left });
    }
    setIsOpen((v) => !v);
  }, [isOpen]);

  useEffect(() => {
    if (!isOpen) return;
    const handler = (e: MouseEvent) => {
      const t = e.target as Node;
      if (
        triggerRef.current && !triggerRef.current.contains(t) &&
        popoverRef.current && !popoverRef.current.contains(t)
      ) setIsOpen(false);
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [isOpen]);

  if (copies.length < 2) return null;

  return (
    <>
      <button
        ref={triggerRef}
        onClick={toggleOpen}
        className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[11px] font-medium transition-colors hover:opacity-80"
        style={{
          backgroundColor: "var(--color-accent-subtle)",
          color: "var(--color-accent)",
          border: "1px solid var(--color-accent-subtle)",
        }}
        title={`같은 이름의 다른 경로 복사본 ${copies.length - 1}개 더 있음`}
        aria-label={`복사본 ${copies.length}곳 보기`}
      >
        <MapPin className="w-3 h-3" strokeWidth={2} />
        <span>{copies.length}곳</span>
      </button>

      {isOpen &&
        createPortal(
          <div
            ref={popoverRef}
            className="fixed z-50 rounded-lg shadow-2xl overflow-hidden"
            style={{
              top: pos.top,
              left: pos.left,
              minWidth: 360,
              maxWidth: 520,
              maxHeight: 360,
              backgroundColor: "var(--color-bg-elevated, var(--color-bg-primary))",
              backgroundImage:
                "linear-gradient(var(--color-bg-elevated, var(--color-bg-primary)), var(--color-bg-elevated, var(--color-bg-primary)))",
              border: "1px solid var(--color-border)",
              boxShadow: "0 10px 25px rgba(0,0,0,0.15), 0 2px 6px rgba(0,0,0,0.08)",
            }}
          >
            <div
              className="px-3 py-2 text-xs font-semibold border-b"
              style={{
                backgroundColor: "var(--color-bg-subtle)",
                color: "var(--color-text-secondary)",
                borderColor: "var(--color-border-subtle)",
              }}
            >
              같은 이름 파일 위치 ({copies.length}곳)
            </div>
            <div className="overflow-y-auto" style={{ maxHeight: 320 }}>
              {copies.map((c) => {
                const isCurrent = c.file_path === currentFilePath;
                return (
                  <button
                    key={c.file_path}
                    onClick={(e) => {
                      e.stopPropagation();
                      onOpenFile(c.file_path);
                      setIsOpen(false);
                    }}
                    className="w-full text-left px-3 py-2 flex items-start gap-2 hover:brightness-110"
                    style={{
                      backgroundColor: isCurrent ? "var(--color-bg-hover)" : "transparent",
                      borderBottom: "1px solid var(--color-border-subtle)",
                    }}
                  >
                    <Folder
                      className="w-3.5 h-3.5 mt-0.5 flex-shrink-0"
                      style={{ color: "var(--color-text-muted)" }}
                    />
                    <div className="flex-1 min-w-0">
                      <div
                        className="text-[11px] truncate"
                        style={{ color: "var(--color-text-primary)" }}
                        title={c.file_path}
                      >
                        {cleanPath(c.file_path)}
                      </div>
                      {c.modified_at ? (
                        <div
                          className="text-[10px] mt-0.5"
                          style={{ color: "var(--color-text-muted)" }}
                          title={new Date(c.modified_at * 1000).toLocaleString("ko-KR")}
                        >
                          {formatRelativeTime(c.modified_at * 1000)}
                        </div>
                      ) : null}
                    </div>
                  </button>
                );
              })}
            </div>
          </div>,
          document.body,
        )}
    </>
  );
}
