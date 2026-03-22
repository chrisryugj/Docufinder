import { memo } from "react";
import { Home, Plus, HelpCircle, Settings, BarChart3, Files, CalendarClock } from "lucide-react";

interface HeaderProps {
  onAddFolder: () => void;
  onOpenSettings: () => void;
  onOpenHelp: () => void;
  onOpenStats: () => void;
  onOpenDuplicates: () => void;
  onOpenExpiry: () => void;
  onGoHome: () => void;
  isIndexing: boolean;
  isSidebarOpen: boolean;
  hasQuery?: boolean;
}

export const Header = memo(function Header({ onAddFolder, onOpenSettings, onOpenHelp, onOpenStats, onOpenDuplicates, onOpenExpiry, onGoHome, isIndexing, isSidebarOpen, hasQuery }: HeaderProps) {
  return (
    <header
      className={`flex items-center justify-between transition-all duration-200 ${isSidebarOpen ? "px-5" : "pl-14 pr-5"}`}
      style={{ height: "44px" }}
    >
      {/* Left: App Title — click to go home */}
      <button
        onClick={onGoHome}
        className="flex items-center gap-2 rounded-md px-1 -mx-1 transition-opacity hover:opacity-70"
        title="홈으로"
      >
        <img src="/anything.png" alt="Anything" className="w-7 h-7 flex-shrink-0 object-contain dark:hidden" />
        <img src="/anything-l.png" alt="Anything" className="w-7 h-7 flex-shrink-0 object-contain hidden dark:block" />
        <h1
          className="ts-md font-bold leading-none text-display"
          style={{ color: "var(--color-text-primary)", letterSpacing: "-0.03em" }}
        >
          Anything<span style={{ color: "var(--color-accent)", fontWeight: 800 }}>.</span>
        </h1>
        {hasQuery && (
          <Home className="w-3.5 h-3.5 flex-shrink-0" style={{ color: "var(--color-text-muted)" }} />
        )}
      </button>

      {/* Right: Action buttons — minimal, icon-only */}
      <div className="flex items-center gap-0.5">
        <button
          onClick={onAddFolder}
          disabled={isIndexing}
          className="flex items-center gap-1.5 pl-3 pr-1.5 py-1 rounded-full ts-sm font-medium transition-colors disabled:opacity-40 disabled:cursor-not-allowed hover-btn-secondary group"
          style={{
            backgroundColor: "var(--color-bg-tertiary)",
            border: "1px solid var(--color-border)",
          }}
          aria-label="폴더 추가"
        >
          {isIndexing ? (
            <span className="flex items-center gap-1.5 pr-1">
              <span
                className="w-3 h-3 rounded-full animate-spin"
                style={{ border: "1.5px solid var(--color-text-muted)", borderTopColor: "var(--color-accent)" }}
              />
              <span className="clr-muted">인덱싱 중</span>
            </span>
          ) : (
            <>
              <span className="clr-secondary">폴더 추가</span>
              <span
                className="w-6 h-6 rounded-full flex items-center justify-center transition-transform group-hover:scale-105"
                style={{ backgroundColor: "var(--color-accent)", color: "white" }}
              >
                <Plus className="w-3 h-3" strokeWidth={2.5} />
              </span>
            </>
          )}
        </button>

        <button
          onClick={onOpenStats}
          className="p-1.5 rounded-md transition-colors btn-icon-hover"
          aria-label="문서 통계"
          title="문서 통계"
        >
          <BarChart3 className="w-4 h-4" style={{ color: "var(--color-text-muted)" }} />
        </button>

        <button
          onClick={onOpenDuplicates}
          className="p-1.5 rounded-md transition-colors btn-icon-hover"
          aria-label="중복 문서 탐지"
          title="중복 문서 탐지"
        >
          <Files className="w-4 h-4" style={{ color: "var(--color-text-muted)" }} />
        </button>

        <button
          onClick={onOpenExpiry}
          className="p-1.5 rounded-md transition-colors btn-icon-hover"
          aria-label="문서 만료 알림"
          title="문서 만료 알림"
        >
          <CalendarClock className="w-4 h-4" style={{ color: "var(--color-text-muted)" }} />
        </button>

        <button
          onClick={onOpenHelp}
          className="p-1.5 rounded-md transition-colors btn-icon-hover"
          aria-label="도움말"
        >
          <HelpCircle className="w-4 h-4" style={{ color: "var(--color-text-muted)" }} />
        </button>

        <button
          onClick={onOpenSettings}
          className="p-1.5 rounded-md transition-colors btn-icon-hover"
          aria-label="설정"
        >
          <Settings className="w-4 h-4" style={{ color: "var(--color-text-muted)" }} />
        </button>
      </div>
    </header>
  );
});
