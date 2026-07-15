import { useState, useEffect, useCallback } from "react";
import { Check } from "lucide-react";
import type {
  ResizableCol,
  ToggleableCol,
  FilenameVisible,
  FilenameSort,
  FilenameSortField,
} from "../../hooks/useFilenameColumns";

interface FilenameColumnHeaderProps {
  gridTemplate: string;
  visible: FilenameVisible;
  sort: FilenameSort | null;
  onSort: (field: FilenameSortField) => void;
  startResize: (col: ResizableCol, e: React.MouseEvent) => void;
  /** 컬럼 경계 더블클릭 — 내용 길이에 맞춰 자동 너비 */
  onAutoFit: (col: ResizableCol) => void;
  toggleColumn: (col: ToggleableCol) => void;
  resetWidths: () => void;
}

interface ColDef {
  key: FilenameSortField;
  label: string;
  resize?: ResizableCol;
  toggle?: ToggleableCol;
}

// 렌더 순서 — buildFilenameGridTemplate과 동일 순서를 유지해야 정렬이 맞는다.
const COLS: ColDef[] = [
  { key: "name", label: "이름", resize: "name" },
  { key: "path", label: "경로", resize: "path", toggle: "path" },
  { key: "size", label: "크기", resize: "size", toggle: "size" },
  { key: "time", label: "수정한 날짜", resize: "time", toggle: "time" },
  { key: "type", label: "유형" },
];

/**
 * 파일명 검색 결과의 Everything식 컬럼 헤더.
 * - 컬럼명 클릭 → 정렬(오름/내림 토글 + 화살표)
 * - 컬럼 우측 경계 드래그 → 너비 조절 / 더블클릭 → 자동 맞춤
 * - 헤더 우클릭 → 컬럼 표시·숨김 + 너비 초기화 메뉴
 */
export function FilenameColumnHeader({
  gridTemplate,
  visible,
  sort,
  onSort,
  startResize,
  onAutoFit,
  toggleColumn,
  resetWidths,
}: FilenameColumnHeaderProps) {
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);

  useEffect(() => {
    if (!menu) return;
    const close = () => setMenu(null);
    document.addEventListener("mousedown", close);
    document.addEventListener("scroll", close, true);
    window.addEventListener("resize", close);
    return () => {
      document.removeEventListener("mousedown", close);
      document.removeEventListener("scroll", close, true);
      window.removeEventListener("resize", close);
    };
  }, [menu]);

  const openMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    setMenu({ x: e.clientX, y: e.clientY });
  }, []);

  const cols = COLS.filter((c) => !c.toggle || visible[c.toggle]);

  return (
    <>
      <div
        className="grid items-center gap-2 px-2.5 py-1.5 text-[11px] font-medium select-none sticky top-0 z-20"
        style={{
          gridTemplateColumns: gridTemplate,
          color: "var(--color-text-muted)",
          backgroundColor: "var(--color-bg-primary)",
          borderBottom: "1px solid var(--color-border)",
        }}
        role="row"
        aria-label="파일명 결과 컬럼 (우클릭: 컬럼 설정)"
        onContextMenu={openMenu}
      >
        {cols.map((c) => (
          <ColHead
            key={c.key}
            label={c.label}
            field={c.key}
            sort={sort}
            onSort={onSort}
            onResize={c.resize ? (e) => startResize(c.resize!, e) : undefined}
            onAutoFit={c.resize ? () => onAutoFit(c.resize!) : undefined}
          />
        ))}
      </div>

      {menu && (
        <div
          className="fixed z-[9999] py-1 rounded-lg shadow-lg text-xs min-w-[160px]"
          style={{
            top: menu.y,
            left: menu.x,
            backgroundColor: "var(--color-bg-secondary)",
            border: "1px solid var(--color-border)",
          }}
          onMouseDown={(e) => e.stopPropagation()}
          role="menu"
        >
          <MenuToggle label="경로" checked={visible.path} onClick={() => toggleColumn("path")} />
          <MenuToggle label="크기" checked={visible.size} onClick={() => toggleColumn("size")} />
          <MenuToggle label="수정한 날짜" checked={visible.time} onClick={() => toggleColumn("time")} />
          <div className="my-1 h-px" style={{ backgroundColor: "var(--color-border)" }} />
          <button
            type="button"
            className="w-full text-left px-3 py-1.5 hover:bg-[var(--color-bg-tertiary)]"
            style={{ color: "var(--color-text-secondary)" }}
            onClick={() => {
              resetWidths();
              setMenu(null);
            }}
          >
            컬럼 너비 초기화
          </button>
        </div>
      )}
    </>
  );
}

function ColHead({
  label,
  field,
  sort,
  onSort,
  onResize,
  onAutoFit,
}: {
  label: string;
  field: FilenameSortField;
  sort: FilenameSort | null;
  onSort: (field: FilenameSortField) => void;
  onResize?: (e: React.MouseEvent) => void;
  onAutoFit?: () => void;
}) {
  const active = sort?.field === field;
  return (
    <div className="relative flex items-center min-w-0 pr-2">
      <button
        type="button"
        onClick={() => onSort(field)}
        className="flex items-center gap-1 min-w-0 hover:text-[var(--color-text-primary)] transition-colors"
        title={`${label}(으)로 정렬`}
      >
        <span className="truncate">{label}</span>
        <span
          className="text-[8px] flex-shrink-0 leading-none"
          style={{ opacity: active ? 1 : 0.25, color: active ? "var(--color-accent)" : "inherit" }}
          aria-hidden="true"
        >
          {active ? (sort!.dir === "asc" ? "▲" : "▼") : "▲"}
        </span>
      </button>
      {onResize && (
        <span
          onMouseDown={onResize}
          onClick={(e) => e.stopPropagation()}
          onDoubleClick={(e) => {
            e.stopPropagation();
            onAutoFit?.();
          }}
          className="absolute top-0 right-0 h-full w-2.5 cursor-col-resize flex items-center justify-end group/resize"
          role="separator"
          aria-orientation="vertical"
          aria-label={`${label} 컬럼 너비 조절 (더블클릭: 자동 맞춤)`}
          title="드래그: 너비 조절 · 더블클릭: 자동 맞춤"
        >
          <span className="w-px h-3.5 bg-[var(--color-border-hover)] group-hover/resize:bg-[var(--color-accent)] transition-colors" />
        </span>
      )}
    </div>
  );
}

function MenuToggle({ label, checked, onClick }: { label: string; checked: boolean; onClick: () => void }) {
  return (
    <button
      type="button"
      className="w-full flex items-center gap-2 px-3 py-1.5 hover:bg-[var(--color-bg-tertiary)]"
      style={{ color: "var(--color-text-secondary)" }}
      onClick={onClick}
      role="menuitemcheckbox"
      aria-checked={checked}
    >
      <span className="w-3.5 flex-shrink-0" style={{ color: "var(--color-accent)" }}>
        {checked && <Check className="w-3.5 h-3.5" />}
      </span>
      <span>{label}</span>
    </button>
  );
}
