import { Button } from "../ui/Button";

interface HeaderProps {
  onAddFolder: () => void;
  onOpenSettings: () => void;
  onOpenHelp: () => void;
  isIndexing: boolean;
  isSidebarOpen: boolean;
}

export function Header({ onAddFolder, onOpenSettings, onOpenHelp, isIndexing, isSidebarOpen }: HeaderProps) {
  return (
    <header
      className={`py-4 flex justify-between items-center bg-transparent transition-all duration-300 ${isSidebarOpen ? "px-6" : "pl-20 pr-6"
        }`}
    >
      <div className="flex items-center gap-4">

        <div className="flex items-center gap-3">
          {/* App Icon */}
          <div className="flex-shrink-0 text-blue-600">
            <svg
              className="w-8 h-8"
              viewBox="0 0 24 24"
              fill="none"
              xmlns="http://www.w3.org/2000/svg"
            >
              <rect x="4" y="4" width="16" height="16" rx="4" className="fill-blue-100" />
              <path d="M15 15L19 19" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" />
              <circle cx="10.5" cy="10.5" r="4.5" stroke="currentColor" strokeWidth="2.5" />
            </svg>
          </div>

          {/* Title Container */}
          <div>
            <h1 className="text-xl font-bold font-display leading-tight" style={{ color: 'var(--color-text-primary)' }}>
              DocuFinder
            </h1>
            <p className="text-xs font-medium tracking-wide" style={{ color: 'var(--color-text-muted)' }}>
              로컬 문서 검색 시스템
            </p>
          </div>
        </div>
      </div>

      <div className="flex items-center gap-3">
        <Button
          onClick={onAddFolder}
          disabled={isIndexing}
          isLoading={isIndexing}
          aria-label="폴더 추가"
          className="font-medium shadow-none hover:shadow-sm transition-colors"
        >
          {isIndexing ? "인덱싱 중..." : "폴더 추가"}
        </Button>
        <button
          onClick={onOpenHelp}
          className="p-2 rounded hover:bg-[var(--color-bg-tertiary)] transition-colors"
          style={{
            color: 'var(--color-text-secondary)',
          }}
          aria-label="도움말"
        >
          <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M8.228 9c.549-1.165 2.03-2 3.772-2 2.21 0 4 1.343 4 3 0 1.4-1.278 2.575-3.006 2.907-.542.104-.994.54-.994 1.093m0 3h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
        </button>
        <button
          onClick={onOpenSettings}
          className="p-2 rounded hover:bg-[var(--color-bg-tertiary)] transition-colors"
          style={{
            color: 'var(--color-text-secondary)',
          }}
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
}
