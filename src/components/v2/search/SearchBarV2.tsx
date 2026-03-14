import React, { forwardRef, useRef } from 'react';
import { Search, Command, X, Loader2 } from 'lucide-react';
import type { IndexStatus } from '../../../types/index';

interface SearchBarV2Props {
  query: string;
  onQueryChange: (query: string) => void;
  onCompositionStart?: () => void;
  onCompositionEnd?: (finalValue: string) => void;
  isLoading: boolean;
  status: IndexStatus | null;
  resultCount: number;
  searchTime: number;
  searchMode: 'keyword' | 'semantic' | 'hybrid';
  onSearchModeChange: (mode: 'keyword' | 'semantic' | 'hybrid') => void;
}

export const SearchBarV2 = forwardRef<HTMLInputElement, SearchBarV2Props>(({
  query,
  onQueryChange,
  onCompositionStart,
  onCompositionEnd,
  isLoading,
  resultCount,
  searchTime,
  searchMode,
  onSearchModeChange
}, ref) => {
  
  const containerRef = useRef<HTMLDivElement>(null);

  return (
    <div className="w-full max-w-4xl mx-auto space-y-3" ref={containerRef}>
      {/* 
        Windows 11 / macOS Spotlight 느낌의 프리미엄 검색바 
        v2-card 클래스를 이용해 자연스러운 깊이감 투입 
      */}
      <div className="relative group z-10">
        {/* Focus Glow Background */}
        <div className="absolute -inset-1 bg-gradient-to-r from-blue-500/0 via-blue-500/10 to-indigo-500/0 rounded-[1.5rem] blur-lg group-focus-within:bg-blue-500/20 transition-all duration-700 opacity-0 group-focus-within:opacity-100 pointer-events-none"></div>
        
        <div className="v2-card v2-card-interactive flex items-center bg-white dark:bg-[#1E293B] relative overflow-hidden transition-all duration-300 focus-within:ring-2 focus-within:ring-blue-500/30">
          
          {/* 돋보기 / 로딩 아이콘 */}
          <div className="pl-6 flex-shrink-0 text-slate-400 dark:text-slate-500 transition-colors group-focus-within:text-blue-500">
            {isLoading ? (
              <Loader2 size={24} className="animate-spin" />
            ) : (
              <Search size={24} strokeWidth={2.5} />
            )}
          </div>

          <input
            ref={ref}
            type="text"
            value={query}
            onChange={(e) => onQueryChange(e.target.value)}
            onCompositionStart={onCompositionStart}
            onCompositionEnd={(e) => onCompositionEnd?.((e.target as HTMLInputElement).value)}
            placeholder="문서의 제목, 내용, 키워드를 검색하세요..."
            className="flex-1 bg-transparent border-none py-5 px-5 text-xl outline-none placeholder:text-slate-400/80 dark:placeholder:text-slate-500 text-slate-800 dark:text-slate-100 font-medium tracking-tight v2-font-heading"
            autoComplete="off"
            spellCheck="false"
          />

          {/* 지우기 버튼 */}
          {query.length > 0 && (
            <button
              onClick={() => {
                onQueryChange('');
                (ref as React.RefObject<HTMLInputElement>)?.current?.focus();
              }}
              className="mr-2 p-2 text-slate-400 hover:text-slate-600 dark:hover:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-700/50 rounded-full transition-colors focus:outline-none focus:ring-2 focus:ring-slate-300"
              aria-label="Clear search"
            >
              <X size={18} />
            </button>
          )}

          {/* 단축키 힌트 */}
          <div className="pr-6 hidden sm:flex items-center pointer-events-none text-slate-300 dark:text-slate-600">
            <div className="flex items-center gap-1 text-xs font-semibold px-2 py-1 rounded-md border border-slate-200 dark:border-slate-700 bg-slate-50 dark:bg-slate-800/50 shadow-sm">
              <Command size={12} />
              <span>K</span>
            </div>
          </div>
        </div>
      </div>

      {/* 검색 메타데이터 및 모드 토글 (하단에 깔끔하게 배치) */}
      <div className="flex items-center justify-between px-2 pt-1 animate-fade-in text-sm">
        <div className="text-slate-500 dark:text-slate-400 flex items-center gap-2 font-medium">
          {query && !isLoading && (
            <>
              <span className="text-slate-800 dark:text-slate-200 font-semibold">{resultCount.toLocaleString()}</span>개의 문서 찾음 
              <span className="text-slate-300 dark:text-slate-600 text-xs">({searchTime.toFixed(3)}초)</span>
            </>
          )}
          {isLoading && <span>검색 중...</span>}
        </div>

        {/* 모드 선택 (Semantic 모드가 활성화되었을 경우에만 유효하나 UI 테스트용으로 노출) */}
        <div className="flex bg-slate-200/50 dark:bg-slate-800/50 p-0.5 rounded-lg border border-slate-200 dark:border-slate-700/50 shadow-sm">
          {(['keyword', 'hybrid', 'semantic'] as const).map((mode) => (
            <button
              key={mode}
              onClick={() => onSearchModeChange(mode)}
              className={`px-3 py-1 text-xs font-semibold rounded-md transition-all ${
                searchMode === mode 
                  ? 'bg-white dark:bg-slate-700 text-blue-600 dark:text-blue-400 shadow-sm ring-1 ring-slate-900/5 dark:ring-white/10' 
                  : 'text-slate-500 dark:text-slate-400 hover:text-slate-700 dark:hover:text-slate-300'
              }`}
            >
              {mode === 'keyword' ? '키워드' : mode === 'hybrid' ? '스마트' : '의미 기반'}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
});

SearchBarV2.displayName = 'SearchBarV2';
