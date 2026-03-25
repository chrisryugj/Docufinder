import { memo } from "react";
import type { RecentSearch } from "../../types/search";

interface WelcomeHeroProps {
  indexedFiles?: number;
  indexedFolders?: number;
  recentSearches?: RecentSearch[];
  onSelectSearch?: (query: string) => void;
  semanticEnabled?: boolean;
}

const FILE_TYPES = [
  { label: "HWPX", color: "var(--color-file-hwpx)", bg: "var(--color-file-hwpx-bg)", border: "color-mix(in srgb, var(--color-file-hwpx) 20%, transparent)" },
  { label: "DOCX", color: "var(--color-file-docx)", bg: "var(--color-file-docx-bg)", border: "color-mix(in srgb, var(--color-file-docx) 20%, transparent)" },
  { label: "XLSX", color: "var(--color-file-xlsx)", bg: "var(--color-file-xlsx-bg)", border: "color-mix(in srgb, var(--color-file-xlsx) 20%, transparent)" },
  { label: "PDF", color: "var(--color-file-pdf)", bg: "var(--color-file-pdf-bg)", border: "color-mix(in srgb, var(--color-file-pdf) 20%, transparent)" },
  { label: "TXT", color: "var(--color-file-txt)", bg: "var(--color-file-txt-bg)", border: "color-mix(in srgb, var(--color-file-txt) 20%, transparent)" },
];

export const WelcomeHero = memo(function WelcomeHero({
  indexedFiles = 0,
  indexedFolders = 0,
  recentSearches = [],
  onSelectSearch,
  semanticEnabled = false,
}: WelcomeHeroProps) {
  const hasIndex = indexedFiles > 0;

  return (
    <div className="flex flex-col items-center justify-center py-14 select-none">
      {/* App Icon + Title */}
      <div className="flex items-center gap-3 mb-4 stagger-item" style={{ animationDelay: "50ms" }}>
        <img src="/anything.png" alt="" className="w-10 h-10 object-contain dark:hidden" />
        <img src="/anything-l.png" alt="" className="w-10 h-10 object-contain hidden dark:block" />
        <h1
          className="ts-hero font-extrabold leading-none text-display clr-primary"
          style={{ letterSpacing: "-0.04em" }}
        >
          Anything<span className="clr-accent">.</span>
        </h1>
      </div>

      {/* Tagline */}
      <p
        className="text-lg mb-5 stagger-item"
        style={{ letterSpacing: "0.01em", animationDelay: "120ms", color: "var(--color-text-muted)" }}
      >
        AI, Everything,{" "}
        <span className="font-semibold" style={{ color: "var(--color-accent)" }}>Anything.</span>
      </p>

      {/* Supported File Types — pill badges */}
      <div className="flex items-center gap-2 mb-6 stagger-item" style={{ animationDelay: "200ms" }}>
        {FILE_TYPES.map((ft) => (
          <span
            key={ft.label}
            className="px-3 py-1 rounded-full text-xs font-bold tracking-wide uppercase text-display"
            style={{
              color: ft.color,
              backgroundColor: ft.bg,
              border: `1px solid ${ft.border}`,
            }}
          >
            {ft.label}
          </span>
        ))}
      </div>

      {/* Index Status */}
      {hasIndex ? (
        <div
          className="flex items-center gap-4 text-sm mb-8 stagger-item clr-muted"
          style={{ animationDelay: "280ms" }}
        >
          <span className="flex items-center gap-1.5">
            <span className="w-2 h-2 rounded-full" style={{ backgroundColor: "var(--color-success)" }} />
            <span className="font-semibold clr-secondary">{indexedFolders}</span>개 폴더
          </span>
          <span className="w-px h-4" style={{ backgroundColor: "var(--color-border)" }} />
          <span><span className="font-semibold clr-secondary">{indexedFiles.toLocaleString()}</span>개 문서</span>
          {semanticEnabled && (
            <>
              <span className="w-px h-4" style={{ backgroundColor: "var(--color-border)" }} />
              <span className="flex items-center gap-1">시맨틱 검색 활성</span>
            </>
          )}
        </div>
      ) : (
        <p
          className="text-sm mb-8 stagger-item clr-muted"
          style={{ animationDelay: "280ms" }}
        >
          사이드바에서 폴더를 추가하여 시작하세요
        </p>
      )}

      {/* Recent Searches */}
      {recentSearches.length > 0 && (
        <div
          className="flex flex-col items-center gap-3 stagger-item"
          style={{ animationDelay: "350ms" }}
        >
          <span className="text-xs font-semibold uppercase tracking-widest clr-muted">
            최근 검색
          </span>
          <div className="flex flex-wrap justify-center gap-2">
            {recentSearches.slice(0, 5).map((s) => (
              <button
                key={s.query}
                onClick={() => onSelectSearch?.(s.query)}
                className="px-3.5 py-2 text-sm rounded-lg btn-outline-accent-hover"
              >
                {s.query}
              </button>
            ))}
          </div>
        </div>
      )}

      {/* Keyboard Hint */}
      <div
        className="mt-10 flex items-center gap-2 text-sm stagger-item clr-muted"
        style={{ animationDelay: "420ms" }}
      >
        <kbd className="inline-flex items-center px-2 py-1 rounded text-xs text-display font-semibold bg-tertiary border border-default clr-muted">
          Ctrl+K
        </kbd>
        <span>로 바로 검색</span>
      </div>
    </div>
  );
});
