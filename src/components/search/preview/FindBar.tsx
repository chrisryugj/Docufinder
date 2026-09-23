import { ChevronDown, ChevronUp, Search, X } from "lucide-react";
import type { PreviewFind } from "./usePreviewFind";

/** 찾기 바 (Ctrl+F): 문서 내 즉석 찾기, 패널 상단 고정 */
export function FindBar({ find }: { find: PreviewFind }) {
  const {
    findInputRef, findInput, setFindInput, findComposingRef, setFindTerm,
    handleFindInputKeyDown, findTerm, findCount, findActiveIdx, handleFindNav, closeFind,
  } = find;
  return (
    <div
      className="flex items-center gap-1.5 px-3 py-1.5 border-b shrink-0"
      style={{ borderColor: "var(--color-border)", backgroundColor: "var(--color-bg-secondary)" }}
    >
      <Search size={12} className="shrink-0 text-[var(--color-text-muted)]" />
      <input
        ref={findInputRef}
        type="text"
        value={findInput}
        onChange={(e) => setFindInput(e.target.value)}
        onCompositionStart={() => { findComposingRef.current = true; }}
        onCompositionEnd={(e) => {
          findComposingRef.current = false;
          setFindInput(e.currentTarget.value);
          setFindTerm(e.currentTarget.value);
        }}
        onKeyDown={handleFindInputKeyDown}
        placeholder="문서 내 찾기"
        className="flex-1 min-w-0 bg-transparent border-none focus:outline-none text-xs"
        style={{ color: "var(--color-text-primary)" }}
        aria-label="문서 내 찾기"
      />
      {findTerm.trim() && (
        <span
          className="text-2xs tabular-nums shrink-0"
          aria-live="polite"
          style={{ color: findCount === 0 ? "var(--color-error)" : "var(--color-text-muted)" }}
        >
          {findCount === 0 ? "0/0" : `${findActiveIdx + 1}/${findCount}`}
        </span>
      )}
      <button
        onClick={() => handleFindNav(-1)}
        disabled={findCount === 0}
        className="p-1 rounded-lg hover:bg-[var(--color-bg-tertiary)] text-[var(--color-text-secondary)] disabled:opacity-40 transition-colors"
        title="이전 (Shift+Enter)"
        aria-label="이전 찾은 곳"
      >
        <ChevronUp size={12} />
      </button>
      <button
        onClick={() => handleFindNav(1)}
        disabled={findCount === 0}
        className="p-1 rounded-lg hover:bg-[var(--color-bg-tertiary)] text-[var(--color-text-secondary)] disabled:opacity-40 transition-colors"
        title="다음 (Enter)"
        aria-label="다음 찾은 곳"
      >
        <ChevronDown size={12} />
      </button>
      <button
        onClick={closeFind}
        className="p-1 rounded-lg hover:bg-[var(--color-bg-tertiary)] text-[var(--color-text-muted)] transition-colors"
        title="찾기 닫기 (Esc)"
        aria-label="찾기 닫기"
      >
        <X size={12} />
      </button>
    </div>
  );
}
