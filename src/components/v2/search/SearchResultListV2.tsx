import React from 'react';
import { FileText, FileSpreadsheet, FileIcon, ChevronRight } from 'lucide-react';

export interface SearchResult {
  file_path: string;
  file_name_highlighted?: string;
  content_preview?: string;
  page_number?: number | null;
  score?: number;
}

// 기존 Highlighting 로직을 재사용하거나 간단히 래핑합니다. (여기서는 HTML 삽입 방식으로 V2 테마 mark 태그가 적용되게 함)
function HighlightedText({ content }: { content: string }) {
  if (!content) return null;
  // Rust 백엔드에서 내려주는 <em> 태그를 V2에서 아름답게 보이게 할 <mark> 태그로 치환
  const safeContent = content.replace(/<em>/g, '<mark>').replace(/<\/em>/g, '</mark>');
  return (
    <span dangerouslySetInnerHTML={{ __html: safeContent }} />
  );
}

// 확장자별 프리미엄 아이콘 및 색상 뱃지
function getFileBadgeStyle(ext: string) {
  const norm = ext.toLowerCase();
  switch (norm) {
    case 'hwpx':
    case 'hwp':
      return {
        bg: 'bg-violet-50 dark:bg-violet-500/10',
        text: 'text-violet-700 dark:text-violet-400',
        border: 'border-violet-200/50 dark:border-violet-500/20',
        label: norm.toUpperCase(),
        Icon: FileText
      };
    case 'pdf':
      return {
        bg: 'bg-red-50 dark:bg-red-500/10',
        text: 'text-red-700 dark:text-red-400',
        border: 'border-red-200/50 dark:border-red-500/20',
        label: 'PDF',
        Icon: FileText
      };
    case 'xlsx':
    case 'xls':
    case 'csv':
      return {
        bg: 'bg-emerald-50 dark:bg-emerald-500/10',
        text: 'text-emerald-700 dark:text-emerald-400',
        border: 'border-emerald-200/50 dark:border-emerald-500/20',
        label: norm.toUpperCase(),
        Icon: FileSpreadsheet
      };
    case 'docx':
    case 'doc':
      return {
        bg: 'bg-blue-50 dark:bg-blue-500/10',
        text: 'text-blue-700 dark:text-blue-400',
        border: 'border-blue-200/50 dark:border-blue-500/20',
        label: norm.toUpperCase(),
        Icon: FileText
      };
    default:
      return {
        bg: 'bg-slate-100 dark:bg-slate-500/10',
        text: 'text-slate-600 dark:text-slate-400',
        border: 'border-slate-200/50 dark:border-slate-500/20',
        label: norm.toUpperCase() || 'FILE',
        Icon: FileIcon
      };
  }
}

interface SearchResultListV2Props {
  results: SearchResult[];
  onOpenFile: (path: string, page?: number) => void;
  onOpenFolder: (path: string) => void;
  selectedIndex: number;
}

export const SearchResultListV2: React.FC<SearchResultListV2Props> = ({
  results,
  onOpenFile,
  onOpenFolder,
  selectedIndex
}) => {
  if (!results.length) return null;

  return (
    <div className="flex flex-col gap-5 pb-24 mt-8 animate-fade-in relative z-0">
      {results.map((result, idx) => {
        const isSelected = selectedIndex === idx;
        const filename = result.file_path.split(/[\\/]/).pop() || '';
        const ext = filename.split('.').pop() || '';
        const badge = getFileBadgeStyle(ext);
        const BadgeIcon = badge.Icon;

        return (
          <div 
            key={`${result.file_path}-${result.page_number || idx}`}
            className={`
              v2-card v2-card-interactive group cursor-pointer overflow-hidden relative
              ${isSelected ? 'ring-2 ring-blue-500/50 shadow-md transform scale-[1.002]' : ''}
            `}
            onClick={() => onOpenFile(result.file_path, result.page_number === null ? undefined : result.page_number)}
            onDoubleClick={(e) => {
              e.stopPropagation();
              onOpenFolder(result.file_path);
            }}
          >
            {/* 좌측 강조 선 (선택 시 혹은 호버 시 살짝 등장) */}
            <div className={`absolute left-0 top-0 bottom-0 w-1 bg-gradient-to-b from-blue-400 to-indigo-500 transform origin-left transition-transform duration-300 ${isSelected ? 'scale-x-100' : 'scale-x-0 group-hover:scale-x-100 opacity-50'}`}></div>

            <div className="p-6">
              <div className="flex items-start justify-between gap-4 mb-3">
                
                {/* 메인 타이틀 영역 */}
                <div className="flex items-center gap-3 flex-1 min-w-0">
                  <div className={`flex items-center justify-center w-10 h-10 rounded-xl border flex-shrink-0 shadow-sm ${badge.bg} ${badge.text} ${badge.border}`}>
                    <BadgeIcon size={20} className="opacity-90" strokeWidth={2.5}/>
                  </div>
                  
                  <div className="flex-1 min-w-0">
                    <h3 className="text-lg font-semibold tracking-tight text-slate-900 dark:text-slate-100 truncate group-hover:text-blue-600 dark:group-hover:text-blue-400 transition-colors duration-200">
                      <HighlightedText content={result.file_name_highlighted || filename} />
                    </h3>
                    
                    {/* 경로 및 페이지 정보 (메타 데이터) */}
                    <div className="flex items-center gap-2 mt-0.5 text-xs text-slate-500 dark:text-slate-400">
                      <span className="truncate max-w-[60%] opacity-80 hover:opacity-100 hover:text-blue-500 transition-opacity" onClick={(e) => { e.stopPropagation(); onOpenFolder(result.file_path); }}>
                        {result.file_path}
                      </span>
                      {result.page_number && (
                        <>
                          <span className="w-1 h-1 rounded-full bg-slate-300 dark:bg-slate-600"></span>
                          <span className="font-medium bg-slate-100 dark:bg-slate-800 px-1.5 py-0.5 rounded text-slate-600 dark:text-slate-300">
                            Page {result.page_number}
                          </span>
                        </>
                      )}
                      {result.score !== undefined && result.score > 0 && (
                        <>
                          <span className="w-1 h-1 rounded-full bg-slate-300 dark:bg-slate-600 hidden sm:inline-block"></span>
                          <span className="hidden sm:inline-block">매칭률: {(result.score * 100).toFixed(1)}%</span>
                        </>
                      )}
                    </div>
                  </div>
                </div>

                {/* 우측 아이콘 (Hover 시 우측으로 살짝 이동) */}
                <div className="text-slate-300 dark:text-slate-600 group-hover:text-blue-500 transform group-hover:translate-x-1 transition-all duration-300 flex-shrink-0 mt-2">
                  <ChevronRight size={20} strokeWidth={2}/>
                </div>
              </div>

              {/* 본문 미리보기 영역 */}
              {result.content_preview && (
                <div className="mt-4 pl-13">
                  <p className="text-[15px] leading-relaxed text-slate-600 dark:text-slate-300 line-clamp-3">
                    <HighlightedText content={result.content_preview} />
                  </p>
                </div>
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
};
