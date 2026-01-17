import { Button } from "../ui/Button";

interface HeaderProps {
  onAddFolder: () => void;
  onOpenSettings: () => void;
  isIndexing: boolean;
}

export function Header({ onAddFolder, onOpenSettings, isIndexing }: HeaderProps) {
  return (
    <header
      className="px-6 py-4 flex justify-between items-center border-b"
      style={{ borderColor: 'var(--color-border)' }}
    >
      <div>
        <h1 className="text-2xl font-bold font-display" style={{ color: 'var(--color-text-primary)' }}>
          DocuFinder
        </h1>
        <p className="text-sm" style={{ color: 'var(--color-text-muted)' }}>
          로컬 문서 검색 시스템
        </p>
      </div>
      <div className="flex items-center gap-3">
        <Button
          onClick={onAddFolder}
          disabled={isIndexing}
          isLoading={isIndexing}
          aria-label="폴더 추가"
        >
          {isIndexing ? "인덱싱 중..." : "폴더 추가"}
        </Button>
        <button
          onClick={onOpenSettings}
          className="p-2 rounded-lg transition-all duration-200 hover:scale-105"
          style={{
            color: 'var(--color-text-muted)',
            backgroundColor: 'transparent',
          }}
          onMouseEnter={(e) => {
            e.currentTarget.style.backgroundColor = 'var(--color-bg-tertiary)';
            e.currentTarget.style.color = 'var(--color-text-primary)';
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.backgroundColor = 'transparent';
            e.currentTarget.style.color = 'var(--color-text-muted)';
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
