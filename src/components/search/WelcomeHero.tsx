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
  { label: "HWPX", color: "var(--color-file-hwpx)" },
  { label: "DOCX", color: "var(--color-file-docx)" },
  { label: "XLSX", color: "var(--color-file-xlsx)" },
  { label: "PDF", color: "var(--color-file-pdf)" },
  { label: "TXT", color: "var(--color-file-txt)" },
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
    <div className="flex flex-col items-center justify-center py-16 select-none">
      {/* App Icon + Title */}
      <div className="flex items-center gap-3 mb-4 stagger-item" style={{ animationDelay: "50ms" }}>
        <img src="/anything.png" alt="" className="w-10 h-10 object-contain dark:hidden" />
        <img src="/anything-l.png" alt="" className="w-10 h-10 object-contain hidden dark:block" />
        <h1
          className="text-[2.75rem] font-extrabold tracking-tight leading-none"
          style={{
            color: "var(--color-text-primary)",
            letterSpacing: "-0.03em",
          }}
        >
          Anything<span style={{ color: "var(--color-accent)" }}>.</span>
        </h1>
      </div>

      {/* Tagline */}
      <p
        className="text-lg mb-8 stagger-item"
        style={{
          color: "var(--color-text-muted)",
          letterSpacing: "0.01em",
          animationDelay: "120ms",
        }}
      >
        흩어진 문서, 한 번에 찾으세요.
      </p>

      {/* Supported File Types — pill badges */}
      <div className="flex items-center gap-2 mb-8 stagger-item" style={{ animationDelay: "200ms" }}>
        {FILE_TYPES.map((ft) => (
          <span
            key={ft.label}
            className="px-3 py-1 rounded-full text-xs font-bold tracking-wide uppercase"
            style={{
              color: ft.color,
              backgroundColor: `color-mix(in srgb, ${ft.color} 10%, transparent)`,
              border: `1px solid color-mix(in srgb, ${ft.color} 20%, transparent)`,
            }}
          >
            {ft.label}
          </span>
        ))}
      </div>

      {/* Decorative divider */}
      <div className="flex items-center gap-3 mb-8 stagger-item" style={{ animationDelay: "280ms" }}>
        <div className="w-8 h-px" style={{ backgroundColor: "var(--color-border)" }} />
        <div className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: "var(--color-accent)", opacity: 0.5 }} />
        <div className="w-8 h-px" style={{ backgroundColor: "var(--color-border)" }} />
      </div>

      {/* Index Status */}
      {hasIndex ? (
        <div
          className="flex items-center gap-4 text-sm mb-8 stagger-item"
          style={{
            color: "var(--color-text-muted)",
            animationDelay: "350ms",
          }}
        >
          <span className="flex items-center gap-1.5">
            <span
              className="w-2 h-2 rounded-full"
              style={{ backgroundColor: "var(--color-success)" }}
            />
            <span className="font-semibold" style={{ color: "var(--color-text-secondary)" }}>{indexedFolders}</span>개 폴더
          </span>
          <span
            className="w-px h-4"
            style={{ backgroundColor: "var(--color-border)" }}
          />
          <span><span className="font-semibold" style={{ color: "var(--color-text-secondary)" }}>{indexedFiles.toLocaleString()}</span>개 문서</span>
          {semanticEnabled && (
            <>
              <span
                className="w-px h-4"
                style={{ backgroundColor: "var(--color-border)" }}
              />
              <span className="flex items-center gap-1">
                시맨틱 검색 활성
              </span>
            </>
          )}
        </div>
      ) : (
        <p
          className="text-sm mb-8 stagger-item"
          style={{
            color: "var(--color-text-muted)",
            animationDelay: "350ms",
          }}
        >
          사이드바에서 폴더를 추가하여 시작하세요
        </p>
      )}

      {/* Recent Searches */}
      {recentSearches.length > 0 && (
        <div
          className="flex flex-col items-center gap-3 stagger-item"
          style={{ animationDelay: "450ms" }}
        >
          <span
            className="text-xs font-semibold uppercase tracking-widest"
            style={{ color: "var(--color-text-muted)" }}
          >
            최근 검색
          </span>
          <div className="flex flex-wrap justify-center gap-2">
            {recentSearches.slice(0, 5).map((s) => (
              <button
                key={s.query}
                onClick={() => onSelectSearch?.(s.query)}
                className="px-3.5 py-2 text-sm rounded-lg transition-colors"
                style={{
                  backgroundColor: "var(--color-bg-tertiary)",
                  color: "var(--color-text-secondary)",
                  border: "1px solid var(--color-border)",
                }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.borderColor = "var(--color-accent)";
                  e.currentTarget.style.color = "var(--color-accent)";
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.borderColor = "var(--color-border)";
                  e.currentTarget.style.color = "var(--color-text-secondary)";
                }}
              >
                {s.query}
              </button>
            ))}
          </div>
        </div>
      )}

      {/* Keyboard Hint */}
      <div
        className="mt-10 flex items-center gap-2 text-sm stagger-item"
        style={{
          color: "var(--color-text-muted)",
          animationDelay: "550ms",
        }}
      >
        <kbd
          className="inline-flex items-center px-2 py-1 rounded text-xs font-mono"
          style={{
            backgroundColor: "var(--color-bg-tertiary)",
            border: "1px solid var(--color-border)",
            color: "var(--color-text-muted)",
          }}
        >
          Ctrl+K
        </kbd>
        <span>로 바로 검색</span>
      </div>
    </div>
  );
});
