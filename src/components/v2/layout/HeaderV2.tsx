import React from 'react';
import { Settings, FolderPlus, HelpCircle } from "lucide-react";

interface HeaderV2Props {
  onAddFolder: () => void;
  onOpenSettings: () => void;
  onOpenHelp: () => void;
  isIndexing: boolean;
}

export const HeaderV2: React.FC<HeaderV2Props> = ({
  onAddFolder,
  onOpenSettings,
  onOpenHelp,
  isIndexing
}) => {
  return (
    <header className="flex items-center justify-between px-8 py-4 backdrop-blur-md bg-white/70 dark:bg-[#0B1120]/70 border-b border-slate-200/50 dark:border-slate-800/50 z-20 sticky top-0 transition-colors">
      
      {/* 윈도우 드래그 영역 (Tauri 특화) 등 필요한 경우 사용, 여기서는 공간 차지용 */}
      <div className="flex items-center gap-4 flex-1" data-tauri-drag-region>
        {/* 사이드바가 없을 때 여기에 타이틀 표시 고려 */}
        <h1 className="text-xl font-bold tracking-tight text-slate-800 dark:text-slate-100 flex items-center gap-2" data-tauri-drag-region>
          <span className="bg-gradient-to-br from-blue-500 to-indigo-600 bg-clip-text text-transparent">Anything</span>
          <span className="text-xs font-semibold px-2 py-0.5 rounded-full bg-blue-100 text-blue-700 dark:bg-blue-500/20 dark:text-blue-300">V2</span>
        </h1>
      </div>

      <div className="flex items-center gap-3">
        {/* 모던하고 절제된 아이콘 버튼들 */}
        <button 
          onClick={onAddFolder}
          disabled={isIndexing}
          className="flex items-center gap-2 px-3 py-1.5 text-sm font-medium text-slate-600 dark:text-slate-300 hover:text-slate-900 dark:hover:text-white bg-slate-100 dark:bg-slate-800/50 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-lg transition-all disabled:opacity-50 disabled:cursor-not-allowed"
          title="검색 폴더 추가"
        >
          <FolderPlus size={16} />
          <span>폴더 추가</span>
        </button>

        <div className="w-px h-5 bg-slate-300 dark:bg-slate-700 mx-1 flex-shrink-0"></div>

        <button 
          onClick={onOpenSettings}
          className="p-1.5 text-slate-400 hover:text-slate-700 dark:hover:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-lg transition-colors"
          title="설정"
        >
          <Settings size={18} />
        </button>

        <button 
          onClick={onOpenHelp}
          className="p-1.5 text-slate-400 hover:text-slate-700 dark:hover:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-lg transition-colors"
          title="도움말"
        >
          <HelpCircle size={18} />
        </button>
      </div>
    </header>
  );
};
