import { memo, useState, useRef, useEffect, useCallback } from "react";
import { Home, Plus, HelpCircle, Settings, BarChart3, Files, MoreHorizontal, Download } from "lucide-react";
import type { UpdatePhase } from "../../hooks/useUpdater";

interface HeaderProps {
  onAddFolder: () => void;
  onOpenSettings: () => void;
  onOpenHelp: () => void;
  onOpenStats: () => void;
  onOpenDuplicates: () => void;
  onGoHome: () => void;
  isIndexing: boolean;
  isSidebarOpen: boolean;
  hasQuery?: boolean;
  updatePhase?: UpdatePhase;
  onOpenUpdate?: () => void;
}

export const Header = memo(function Header({ onAddFolder, onOpenSettings, onOpenHelp, onOpenStats, onOpenDuplicates, onGoHome, isIndexing, isSidebarOpen, hasQuery, updatePhase, onOpenUpdate }: HeaderProps) {
  const updateVisible =
    updatePhase === "available" ||
    updatePhase === "downloading" ||
    updatePhase === "installing" ||
    updatePhase === "ready-to-restart";
  const [menuOpen, setMenuOpen] = useState(false);
  const menuBtnRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);

  const closeMenu = useCallback(() => setMenuOpen(false), []);

  // 외부 클릭 닫기
  useEffect(() => {
    if (!menuOpen) return;
    const handleClick = (e: MouseEvent) => {
      if (menuRef.current?.contains(e.target as Node)) return;
      if (menuBtnRef.current?.contains(e.target as Node)) return;
      closeMenu();
    };
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") closeMenu();
    };
    document.addEventListener("mousedown", handleClick);
    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("mousedown", handleClick);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [menuOpen, closeMenu]);

  const menuItems = [
    { icon: BarChart3, label: "문서 통계", onClick: onOpenStats },
    { icon: Files, label: "중복 문서 탐지", onClick: onOpenDuplicates },
    { icon: HelpCircle, label: "도움말", onClick: onOpenHelp },
  ];

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

      {/* Right: Action buttons — 업데이트 + 폴더추가 + 오버플로우 + 설정 */}
      <div className="flex items-center gap-0.5">
        {updateVisible && onOpenUpdate && (
          <button
            onClick={onOpenUpdate}
            className="relative p-1.5 rounded-md transition-colors btn-icon-hover"
            aria-label="업데이트"
            title={
              updatePhase === "available" ? "업데이트 사용 가능 — 클릭하여 설치" :
              updatePhase === "downloading" ? "업데이트 다운로드 중" :
              updatePhase === "installing" ? "업데이트 설치 중" :
              "재시작 필요"
            }
          >
            <Download className="w-4 h-4" style={{ color: "var(--color-accent)" }} />
            <span
              className="absolute top-1 right-1 w-1.5 h-1.5 rounded-full animate-pulse"
              style={{ backgroundColor: "var(--color-accent)" }}
            />
          </button>
        )}
        <button
          onClick={onAddFolder}
          disabled={isIndexing}
          className="p-1.5 rounded-md transition-colors disabled:opacity-40 disabled:cursor-not-allowed btn-icon-hover"
          aria-label="폴더 추가"
          title={isIndexing ? "인덱싱이 완료된 후 폴더를 추가할 수 있습니다" : "폴더 추가"}
        >
          {isIndexing ? (
            <span
              className="w-4 h-4 block rounded-full animate-spin"
              style={{ border: "1.5px solid var(--color-text-muted)", borderTopColor: "var(--color-accent)" }}
            />
          ) : (
            <Plus className="w-4 h-4" style={{ color: "var(--color-accent)" }} />
          )}
        </button>

        {/* 오버플로우 메뉴 */}
        <div className="relative" data-tour="help-button">
          <button
            ref={menuBtnRef}
            onClick={() => setMenuOpen((v) => !v)}
            className="p-1.5 rounded-md transition-colors btn-icon-hover"
            aria-label="더보기"
            aria-expanded={menuOpen}
            title="더보기"
          >
            <MoreHorizontal className="w-4 h-4" style={{ color: "var(--color-text-muted)" }} />
          </button>

          {menuOpen && (
            <div
              ref={menuRef}
              className="absolute right-0 top-full mt-1 py-1 rounded-lg z-50"
              style={{
                backgroundColor: "var(--color-bg-secondary)",
                border: "1px solid var(--color-border)",
                boxShadow: "var(--shadow-lg)",
                minWidth: "160px",
              }}
            >
              {menuItems.map((item) => (
                <button
                  key={item.label}
                  onClick={() => { item.onClick(); closeMenu(); }}
                  className="w-full flex items-center gap-2.5 px-3 py-2 text-sm transition-colors hover:bg-[var(--color-bg-tertiary)]"
                  style={{ color: "var(--color-text-secondary)" }}
                >
                  <item.icon className="w-4 h-4 flex-shrink-0" style={{ color: "var(--color-text-muted)" }} />
                  {item.label}
                </button>
              ))}
            </div>
          )}
        </div>

        <button
          onClick={onOpenSettings}
          className="p-1.5 rounded-md transition-colors btn-icon-hover"
          aria-label="설정"
          data-tour="settings-button"
        >
          <Settings className="w-4 h-4" style={{ color: "var(--color-text-muted)" }} />
        </button>
      </div>
    </header>
  );
});
