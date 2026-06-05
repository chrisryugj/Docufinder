import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";

export interface Command {
  id: string;
  label: string;
  /** 우측 보조 설명/현재값 */
  hint?: string;
  /** 좌측 아이콘 */
  icon?: ReactNode;
  /** 검색 매칭용 추가 키워드(라벨 외) */
  keywords?: string;
  /** 그룹 헤더 라벨 */
  group: string;
  run: () => void;
}

interface CommandPaletteProps {
  isOpen: boolean;
  onClose: () => void;
  commands: Command[];
}

/**
 * 명령 팔레트 (Cmd/Ctrl+K).
 * - 상단 정렬 경량 오버레이. 검색 입력 + 그룹별 명령 리스트.
 * - substring 매칭(한글 안전), ↑↓ 네비, Enter 실행, Esc 닫기.
 * - role="dialog" → 전역 화살표/Esc 단축키가 자동 양보(useKeyboardShortcuts).
 */
export function CommandPalette({ isOpen, onClose, commands }: CommandPaletteProps) {
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  // 열릴 때마다 초기화 + 입력 포커스
  useEffect(() => {
    if (isOpen) {
      setQuery("");
      setSelected(0);
      // 마운트 직후 포커스
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  }, [isOpen]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return commands;
    return commands.filter((c) =>
      c.label.toLowerCase().includes(q) ||
      c.group.toLowerCase().includes(q) ||
      (c.keywords?.toLowerCase().includes(q) ?? false)
    );
  }, [commands, query]);

  // 필터 변하면 선택을 첫 항목으로
  useEffect(() => { setSelected(0); }, [query]);

  // 선택 항목이 보이도록 스크롤
  useEffect(() => {
    if (!isOpen) return;
    const el = listRef.current?.querySelector<HTMLElement>(`[data-cmd-idx="${selected}"]`);
    el?.scrollIntoView({ block: "nearest" });
  }, [selected, isOpen]);

  if (!isOpen) return null;

  const runAt = (idx: number) => {
    const cmd = filtered[idx];
    if (!cmd) return;
    onClose();
    cmd.run();
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelected((s) => (filtered.length ? (s + 1) % filtered.length : 0));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelected((s) => (filtered.length ? (s - 1 + filtered.length) % filtered.length : 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      runAt(selected);
    } else if (e.key === "Escape") {
      e.preventDefault();
      onClose();
    }
  };

  // 그룹 헤더 삽입을 위해 직전 그룹 추적
  let lastGroup = "";

  return (
    <div
      className="fixed inset-0 z-[9999] flex items-start justify-center animate-fade-in"
      style={{ paddingTop: "12vh" }}
      onMouseDown={onClose}
    >
      <div className="absolute inset-0 bg-black/30" aria-hidden="true" />
      <div
        role="dialog"
        aria-modal="true"
        aria-label="명령 팔레트"
        className="relative w-full max-w-xl mx-4 rounded-xl shadow-2xl overflow-hidden"
        style={{ backgroundColor: "var(--color-bg-secondary)", border: "1px solid var(--color-border)" }}
        onMouseDown={(e) => e.stopPropagation()}
      >
        {/* 검색 입력 */}
        <div className="flex items-center gap-2 px-4 py-3 border-b" style={{ borderColor: "var(--color-border)" }}>
          <span className="text-sm" style={{ color: "var(--color-text-muted)" }}>⌘K</span>
          <input
            ref={inputRef}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="명령 검색… (폴더 추가, 설정, 테마, 검색 모드…)"
            className="flex-1 bg-transparent outline-none text-sm"
            style={{ color: "var(--color-text-primary)" }}
            aria-label="명령 검색"
          />
        </div>

        {/* 명령 리스트 */}
        <div ref={listRef} className="max-h-[50vh] overflow-y-auto py-1">
          {filtered.length === 0 ? (
            <div className="px-4 py-6 text-center text-sm" style={{ color: "var(--color-text-muted)" }}>
              일치하는 명령이 없습니다
            </div>
          ) : (
            filtered.map((cmd, idx) => {
              const showHeader = cmd.group !== lastGroup;
              lastGroup = cmd.group;
              const isSel = idx === selected;
              return (
                <div key={cmd.id}>
                  {showHeader && (
                    <div
                      className="px-4 pt-2 pb-1 text-[10px] font-semibold uppercase tracking-wider"
                      style={{ color: "var(--color-text-muted)" }}
                    >
                      {cmd.group}
                    </div>
                  )}
                  <button
                    data-cmd-idx={idx}
                    onMouseEnter={() => setSelected(idx)}
                    onClick={() => runAt(idx)}
                    className="w-full flex items-center gap-3 px-4 py-2 text-left transition-colors"
                    style={{
                      backgroundColor: isSel ? "var(--color-accent-light)" : "transparent",
                    }}
                  >
                    {cmd.icon && (
                      <span className="flex-shrink-0" style={{ color: isSel ? "var(--color-accent)" : "var(--color-text-muted)" }}>
                        {cmd.icon}
                      </span>
                    )}
                    <span className="flex-1 truncate text-sm" style={{ color: "var(--color-text-primary)" }}>
                      {cmd.label}
                    </span>
                    {cmd.hint && (
                      <span className="flex-shrink-0 text-xs truncate max-w-[40%]" style={{ color: "var(--color-text-muted)" }}>
                        {cmd.hint}
                      </span>
                    )}
                  </button>
                </div>
              );
            })
          )}
        </div>
      </div>
    </div>
  );
}
