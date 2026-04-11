import { memo } from "react";
import { Search, Sparkles, Clock } from "lucide-react";
import type { RecentSearch } from "../../types/search";

interface WelcomeHeroProps {
  indexedFiles?: number;
  indexedFolders?: number;
  recentSearches?: RecentSearch[];
  onSelectSearch?: (query: string) => void;
  semanticEnabled?: boolean;
  onAddFolder?: () => void;
}

export const WelcomeHero = memo(function WelcomeHero({
  indexedFiles = 0,
  indexedFolders = 0,
  recentSearches = [],
  onSelectSearch,
  semanticEnabled = false,
  onAddFolder,
}: WelcomeHeroProps) {
  const hasIndex = indexedFiles > 0;

  const triggerSearchFocus = () => {
    document.dispatchEvent(
      new KeyboardEvent("keydown", { key: "k", ctrlKey: true, bubbles: true })
    );
  };

  return (
    <div className="w-full flex flex-col items-center justify-center select-none h-full min-h-[60vh] px-6">
      {/* Ambient glow */}
      <div
        className="absolute pointer-events-none"
        style={{
          width: "500px",
          height: "500px",
          background: "radial-gradient(circle, var(--color-accent-glow) 0%, transparent 70%)",
          opacity: 0.4,
          filter: "blur(80px)",
          top: "20%",
          left: "50%",
          transform: "translateX(-50%)",
        }}
      />

      {/* Logo + Title */}
      <div className="relative z-10 flex flex-col items-center mb-10 animate-fade-in">
        <div className="flex items-center gap-3 mb-4">
          <img
            src="/anything.png"
            alt=""
            className="w-10 h-10 object-contain drop-shadow-sm dark:hidden"
          />
          <img
            src="/anything-l.png"
            alt=""
            className="w-10 h-10 object-contain drop-shadow-sm hidden dark:block"
          />
        </div>

        <h1
          className="text-display font-extrabold leading-none mb-3"
          style={{
            fontSize: "clamp(2.5rem, 5vw, 3.5rem)",
            letterSpacing: "var(--tracking-hero)",
            color: "var(--color-text-primary)",
          }}
        >
          Anything<span style={{ color: "var(--color-accent)" }}>.</span>
        </h1>

        <p
          className="text-center font-medium"
          style={{
            fontSize: "var(--text-md)",
            color: "var(--color-text-muted)",
            letterSpacing: "-0.01em",
            maxWidth: "320px",
          }}
        >
          내 PC 깊숙이 흩어진 문서들.
          <br />
          이제 AI가 읽고, 답을 찾아냅니다.
        </p>
      </div>

      {/* Search Prompt */}
      <div
        className="relative z-10 w-full animate-fade-in"
        style={{ maxWidth: "520px", animationDelay: "80ms" }}
      >
        <div
          onClick={triggerSearchFocus}
          className="flex items-center gap-3 w-full rounded-2xl px-5 py-4 cursor-text transition-all duration-200 group/search"
          style={{
            backgroundColor: "var(--color-bg-secondary)",
            border: "1px solid var(--color-border)",
            boxShadow: "var(--shadow-card)",
          }}
          onMouseEnter={(e) => {
            e.currentTarget.style.borderColor = "var(--color-accent)";
            e.currentTarget.style.boxShadow = "var(--shadow-card-hover)";
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.borderColor = "var(--color-border)";
            e.currentTarget.style.boxShadow = "var(--shadow-card)";
          }}
        >
          <Search
            className="w-5 h-5 flex-shrink-0 transition-colors"
            style={{ color: "var(--color-text-muted)" }}
          />
          <span
            className="flex-1 ts-base font-medium"
            style={{ color: "var(--color-text-muted)" }}
          >
            무엇이든 검색하세요...
          </span>
          <kbd
            className="inline-flex items-center px-2 py-1 rounded text-xs font-bold font-mono transition-colors group-hover/search:text-[var(--color-accent)]"
            style={{
              backgroundColor: "var(--color-bg-tertiary)",
              border: "1px solid var(--color-border)",
              color: "var(--color-text-muted)",
            }}
          >
            Ctrl+K
          </kbd>
        </div>
      </div>

      {/* Recent Searches */}
      {recentSearches.length > 0 && (
        <div
          className="relative z-10 mt-6 w-full flex flex-col items-center animate-fade-in"
          style={{ maxWidth: "520px", animationDelay: "160ms" }}
        >
          <div className="flex items-center gap-1.5 mb-3">
            <Clock
              className="w-3 h-3"
              style={{ color: "var(--color-text-muted)", opacity: 0.6 }}
            />
            <span
              className="ts-xs font-semibold uppercase"
              style={{
                color: "var(--color-text-muted)",
                letterSpacing: "0.08em",
                opacity: 0.6,
              }}
            >
              최근 검색
            </span>
          </div>
          <div className="flex flex-wrap justify-center gap-2">
            {recentSearches.slice(0, 6).map((s) => (
              <button
                key={s.query}
                onClick={() => onSelectSearch?.(s.query)}
                className="group px-3 py-1.5 ts-sm rounded-lg border transition-all duration-150 hover:scale-[1.02] active:scale-[0.98] flex items-center gap-1.5"
                style={{
                  backgroundColor: "var(--color-bg-secondary)",
                  borderColor: "var(--color-border)",
                  color: "var(--color-text-secondary)",
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
                <Search className="w-3 h-3 opacity-40 group-hover:opacity-80 transition-opacity" />
                <span className="truncate max-w-[140px]">{s.query}</span>
              </button>
            ))}
          </div>
        </div>
      )}

      {/* Status line */}
      <div
        className="relative z-10 mt-8 flex items-center gap-3 animate-fade-in"
        style={{ animationDelay: "240ms" }}
      >
        {hasIndex ? (
          <>
            <span
              className="ts-xs tabular-nums"
              style={{ color: "var(--color-text-muted)", opacity: 0.5 }}
            >
              {indexedFiles.toLocaleString()} 문서 · {indexedFolders} 폴더
            </span>
            {semanticEnabled && (
              <>
                <span
                  className="ts-xs"
                  style={{ color: "var(--color-text-muted)", opacity: 0.3 }}
                >
                  ·
                </span>
                <span className="flex items-center gap-1">
                  <Sparkles
                    className="w-3 h-3"
                    style={{ color: "var(--color-accent-ai)" }}
                  />
                  <span
                    className="ts-xs font-semibold"
                    style={{ color: "var(--color-accent-ai)" }}
                  >
                    AI Ready
                  </span>
                </span>
              </>
            )}
          </>
        ) : (
          <button
            onClick={onAddFolder}
            className="flex items-center gap-2 px-4 py-2 rounded-xl ts-sm font-medium transition-all duration-150 hover:scale-[1.02] active:scale-[0.98]"
            style={{
              backgroundColor: "var(--color-accent)",
              color: "white",
              boxShadow: "0 2px 8px rgba(1, 175, 122, 0.3)",
            }}
          >
            폴더 추가하여 시작하기
          </button>
        )}
      </div>
    </div>
  );
});
