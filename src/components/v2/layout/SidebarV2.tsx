import React from 'react';
import { Home, Clock, Star, HardDrive } from 'lucide-react';

interface SidebarV2Props {
  // 나중에 구체적인 핸들러 추가
}

export const SidebarV2: React.FC<SidebarV2Props> = () => {
  return (
    <aside className="hidden md:flex flex-col w-[280px] border-r border-slate-200/40 dark:border-slate-800/40 bg-white/30 dark:bg-[#020617]/40 backdrop-blur-3xl z-30 shadow-[4px_0_24px_-12px_rgba(0,0,0,0.05)] pt-4 pb-6 px-4">
      {/* 로고 영역 */}
      <div className="mb-8 px-2 flex items-center gap-3">
        <div className="w-10 h-10 rounded-2xl bg-gradient-to-br from-blue-500 to-indigo-600 shadow-lg shadow-blue-500/25 flex items-center justify-center border border-white/20">
          <span className="text-white text-lg font-bold leading-none select-none tracking-tighter">DF</span>
        </div>
        <div className="flex flex-col">
          <span className="text-xl font-bold tracking-tight text-slate-800 dark:text-slate-100 leading-tight">Anything</span>
          <span className="text-[10px] font-semibold text-blue-600/80 dark:text-blue-400/80 tracking-wide uppercase">Premium Search</span>
        </div>
      </div>

      {/* 내비게이션 메뉴 */}
      <nav className="flex-1 space-y-8">
        {/* 그룹 1 */}
        <div className="space-y-1.5">
          <button className="w-full flex items-center gap-3 px-3 py-2.5 rounded-xl bg-blue-50/50 dark:bg-blue-500/10 text-blue-700 dark:text-blue-400 font-semibold shadow-sm transition-all">
            <Home size={18} strokeWidth={2.5} />
            홈
          </button>
          <button className="w-full flex items-center gap-3 px-3 py-2.5 rounded-xl text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-200 hover:bg-white/50 dark:hover:bg-slate-800/50 font-medium transition-all group">
            <Clock size={18} className="group-hover:scale-110 transition-transform" />
            최근 검색어
          </button>
          <button className="w-full flex items-center gap-3 px-3 py-2.5 rounded-xl text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-200 hover:bg-white/50 dark:hover:bg-slate-800/50 font-medium transition-all group">
            <Star size={18} className="group-hover:scale-110 transition-transform" />
            즐겨찾기
          </button>
        </div>

        {/* 그룹 2 */}
        <div>
          <h3 className="px-3 text-xs font-semibold text-slate-400 dark:text-slate-500 uppercase tracking-widest mb-3">검색 위치</h3>
          <div className="space-y-1">
            <button className="w-full flex items-center gap-3 px-3 py-2.5 rounded-xl text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-200 hover:bg-white/50 dark:hover:bg-slate-800/50 font-medium transition-all group">
              <HardDrive size={18} className="text-slate-400 dark:text-slate-500 group-hover:text-blue-500 transition-colors" />
              로컬 디스크 C:
            </button>
            <button className="w-full flex items-center gap-3 px-3 py-2.5 rounded-xl text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-200 hover:bg-white/50 dark:hover:bg-slate-800/50 font-medium transition-all group border border-dashed border-slate-300 dark:border-slate-700 bg-slate-50/50 dark:bg-slate-800/20">
              <span className="text-xl leading-none text-slate-400">+</span>
              폴더 추가
            </button>
          </div>
        </div>
      </nav>

      {/* 하단 시스템 스택 표시 */}
      <div className="mt-auto px-1">
        <div className="flex flex-wrap gap-2">
          <span className="flex items-center gap-1.5 px-2.5 py-1 rounded-lg bg-white/40 dark:bg-slate-800/40 border border-slate-200/50 dark:border-slate-700/50 text-[11px] font-medium text-slate-500 shadow-sm backdrop-blur-md">
            <span className="w-1.5 h-1.5 rounded-full bg-green-500 shadow-[0_0_8px_rgba(34,197,94,0.6)]"></span>
            FTS Engine
          </span>
          <span className="flex items-center gap-1.5 px-2.5 py-1 rounded-lg bg-white/40 dark:bg-slate-800/40 border border-slate-200/50 dark:border-slate-700/50 text-[11px] font-medium text-slate-500 shadow-sm backdrop-blur-md">
            <span className="w-1.5 h-1.5 rounded-full bg-blue-500 shadow-[0_0_8px_rgba(59,130,246,0.6)]"></span>
            Vector API
          </span>
        </div>
      </div>
    </aside>
  );
};
