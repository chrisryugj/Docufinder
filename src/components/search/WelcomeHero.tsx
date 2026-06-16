import { memo } from "react";
import { Search, Sparkles, Clock, FileClock } from "lucide-react";
import type { RecentSearch } from "../../types/search";
import { FileIcon } from "../ui/FileIcon";
import { useRecentDocuments } from "../../hooks/useRecentDocuments";
import { usePointerSpotlight } from "../../hooks/usePointerSpotlight";

interface WelcomeHeroProps {
  indexedFiles?: number;
  indexedFolders?: number;
  recentSearches?: RecentSearch[];
  onSelectSearch?: (query: string) => void;
  semanticEnabled?: boolean;
  onAddFolder?: () => void;
  /** 최근 작업한 문서 열기 */
  onOpenFile?: (path: string) => void;
}

export const WelcomeHero = memo(function WelcomeHero({
  indexedFiles = 0,
  indexedFolders = 0,
  recentSearches = [],
  onSelectSearch,
  semanticEnabled = false,
  onAddFolder,
  onOpenFile,
}: WelcomeHeroProps) {
  const hasIndex = indexedFiles > 0;
  const recentDocs = useRecentDocuments(hasIndex);

  // 마우스를 따라오는 ambient glow (성능 안전 — ref + rAF, 리렌더 없음)
  const glowRef = usePointerSpotlight<HTMLDivElement>();

  return (
    <div
      ref={glowRef}
      className="relative w-full flex flex-col items-center justify-center select-none h-full min-h-[60vh] px-6"
    >
      {/* Glow 레이어 — 방사형 마스크로 사방 가장자리를 부드럽게 fade-out.
          overflow-hidden 사각 클립이 만들던 상/하/좌/우 직선 경계를 모두 제거한다. */}
      <div
        aria-hidden
        className="absolute inset-0 overflow-hidden pointer-events-none"
        style={{
          WebkitMaskImage:
            "radial-gradient(ellipse 75% 65% at center, black 30%, transparent 72%)",
          maskImage:
            "radial-gradient(ellipse 75% 65% at center, black 30%, transparent 72%)",
        }}
      >
        {/* Ambient glow — 마우스를 부드럽게 추종. transform(GPU)으로 reflow 없이 이동. */}
        <div
          className="absolute left-0 top-0 transition-transform duration-300 ease-out"
          style={{
            width: "460px",
            height: "460px",
            background: "radial-gradient(circle, var(--color-accent-glow) 0%, transparent 70%)",
            opacity: 0.3,
            filter: "blur(90px)",
            transform:
              "translate3d(var(--spot-x, 380px), var(--spot-y, 260px), 0) translate(-50%, -50%)",
            willChange: "transform",
          }}
        />
      </div>

      {/* Logo + Title */}
      <div className="relative z-10 flex flex-col items-center mb-4 animate-fade-in">
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

      {/* Recent Searches */}
      {recentSearches.length > 0 && (
        <div
          className="relative z-10 mt-6 w-full flex flex-col items-center animate-fade-in"
          style={{ maxWidth: "520px", animationDelay: "160ms" }}
        >
          <div className="flex items-center gap-1.5 mb-3">
            <Clock
              className="w-3 h-3"
              style={{ color: "var(--color-text-tertiary)" }}
            />
            <span
              className="ts-xs font-semibold uppercase"
              style={{
                color: "var(--color-text-tertiary)",
                letterSpacing: "0.08em",
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
                className="group px-2.5 py-1 ts-sm rounded-md transition-colors duration-150 flex items-center gap-1.5"
                style={{
                  backgroundColor: "var(--color-bg-tertiary)",
                  color: "var(--color-text-secondary)",
                }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.color = "var(--color-accent)";
                }}
                onMouseLeave={(e) => {
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

      {/* Recently opened documents — 열람 신호 노출 */}
      {hasIndex && recentDocs.length > 0 && (
        <div
          className="relative z-10 mt-6 w-full flex flex-col items-center animate-fade-in"
          style={{ maxWidth: "520px", animationDelay: "200ms" }}
        >
          <div className="flex items-center gap-1.5 mb-3">
            <FileClock
              className="w-3 h-3"
              style={{ color: "var(--color-text-tertiary)" }}
            />
            <span
              className="ts-xs font-semibold uppercase"
              style={{
                color: "var(--color-text-tertiary)",
                letterSpacing: "0.08em",
              }}
            >
              최근 작업한 문서
            </span>
          </div>
          <div className="grid grid-cols-2 gap-2 w-full">
            {recentDocs.slice(0, 6).map((doc) => (
              <button
                key={doc.path}
                onClick={() => onOpenFile?.(doc.path)}
                title={doc.path}
                className="group flex items-center gap-2 px-2.5 py-2 rounded-md transition-colors duration-150 text-left min-w-0"
                style={{
                  backgroundColor: "transparent",
                }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.backgroundColor = "var(--color-bg-secondary)";
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.backgroundColor = "transparent";
                }}
              >
                <FileIcon fileName={doc.name} size="sm" />
                <span
                  className="truncate ts-sm"
                  style={{ color: "var(--color-text-secondary)" }}
                >
                  {doc.name}
                </span>
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
              style={{ color: "var(--color-text-tertiary)" }}
            >
              {indexedFiles.toLocaleString()} 문서 · {indexedFolders} 폴더
            </span>
            {semanticEnabled && (
              <>
                <span
                  className="ts-xs"
                  style={{ color: "var(--color-text-tertiary)" }}
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
            className="flex items-center gap-2 px-5 py-2.5 rounded-xl ts-sm font-semibold transition-all duration-150 hover:scale-[1.02] active:scale-[0.98]"
            style={{
              backgroundImage: "var(--gradient-accent)",
              color: "white",
              boxShadow: "var(--shadow-premium-accent)",
            }}
          >
            폴더 추가하여 시작하기
          </button>
        )}
      </div>
    </div>
  );
});
