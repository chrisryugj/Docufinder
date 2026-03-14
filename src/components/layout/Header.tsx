import { memo } from "react";

interface HeaderProps {
  onAddFolder: () => void;
  onOpenSettings: () => void;
  onOpenHelp: () => void;
  isIndexing: boolean;
  isSidebarOpen: boolean;
}

export const Header = memo(function Header({ onAddFolder, onOpenSettings, onOpenHelp, isIndexing, isSidebarOpen }: HeaderProps) {
  return (
    <header
      className={`py-2 flex justify-between items-center bg-transparent transition-all duration-300 ${isSidebarOpen ? "px-6" : "pl-16 pr-6"
        }`}
    >
      <div className="flex items-center gap-3">
        <div className="flex items-center gap-2">
          {/* App Icon */}
          <img
            src="/icon.png"
            alt="Anything"
            className="w-7 h-7 flex-shrink-0 object-contain"
          />

          {/* Title Container */}
          <div className="flex items-baseline gap-2">
            <h1 className="text-base font-semibold font-display leading-tight" style={{ color: 'var(--color-text-primary)' }}>
              Anything
            </h1>
          </div>
        </div>
      </div>

      <div className="flex items-center gap-1">
        <button
          onClick={onAddFolder}
          disabled={isIndexing}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-sm font-medium transition-all duration-150 outline-none focus:outline-none disabled:opacity-50 disabled:cursor-not-allowed btn-icon-hover"
          aria-label="폴더 추가"
        >
          {isIndexing ? (
            <span className="flex items-center gap-1.5">
              <span className="w-3.5 h-3.5 border-2 border-current border-t-transparent rounded-full animate-spin" />
              인덱싱 중...
            </span>
          ) : (
            <>
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
              </svg>
              폴더 추가
            </>
          )}
        </button>
        <button
          onClick={onOpenHelp}
          className="p-2 rounded-lg transition-all duration-150 outline-none focus:outline-none btn-icon-hover"
          aria-label="도움말"
        >
          <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M8.228 9c.549-1.165 2.03-2 3.772-2 2.21 0 4 1.343 4 3 0 1.4-1.278 2.575-3.006 2.907-.542.104-.994.54-.994 1.093m0 3h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
        </button>
        <button
          onClick={onOpenSettings}
          className="p-2 rounded-lg transition-all duration-150 outline-none focus:outline-none btn-icon-hover"
          aria-label="설정"
        >
          <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
          </svg>
        </button>
      </div>
    </header>
  );
});
