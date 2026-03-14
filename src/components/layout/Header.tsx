import { memo } from "react";
import { Home, Plus, HelpCircle, Settings } from "lucide-react";

interface HeaderProps {
  onAddFolder: () => void;
  onOpenSettings: () => void;
  onOpenHelp: () => void;
  onGoHome: () => void;
  isIndexing: boolean;
  isSidebarOpen: boolean;
  hasQuery?: boolean;
}

export const Header = memo(function Header({ onAddFolder, onOpenSettings, onOpenHelp, onGoHome, isIndexing, isSidebarOpen, hasQuery }: HeaderProps) {
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
          className="text-[15px] font-bold tracking-tight leading-none"
          style={{ color: "var(--color-text-primary)", letterSpacing: "-0.02em" }}
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
          className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-md text-[13px] font-medium transition-colors disabled:opacity-40 disabled:cursor-not-allowed btn-icon-hover"
          aria-label="폴더 추가"
        >
          {isIndexing ? (
            <span className="flex items-center gap-1.5">
              <span
                className="w-3 h-3 rounded-full animate-spin"
                style={{ border: "1.5px solid var(--color-text-muted)", borderTopColor: "var(--color-accent)" }}
              />
              <span style={{ color: "var(--color-text-muted)" }}>인덱싱 중</span>
            </span>
          ) : (
            <>
              <Plus className="w-3.5 h-3.5" />
              <span style={{ color: "var(--color-text-secondary)" }}>폴더 추가</span>
            </>
          )}
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
