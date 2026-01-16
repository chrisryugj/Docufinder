import { useCallback } from "react";
import type { SearchResult } from "../../types/search";
import { HighlightedText } from "./HighlightedText";
import { FileIcon } from "../ui/FileIcon";
import { Badge, getFileTypeBadgeVariant } from "../ui/Badge";

interface SearchResultItemProps {
  result: SearchResult;
  index: number;
  isExpanded: boolean;
  isSelected?: boolean;
  onToggleExpand: () => void;
  onOpenFile: (filePath: string, page?: number | null) => void;
  onCopyPath?: (path: string) => void;
  onOpenFolder?: (path: string) => void;
}

export function SearchResultItem({
  result,
  index,
  isExpanded,
  isSelected = false,
  onToggleExpand,
  onOpenFile,
  onCopyPath,
  onOpenFolder,
}: SearchResultItemProps) {
  const fileExt = result.file_name.split(".").pop()?.toLowerCase() || "";

  // 경로에서 폴더 추출
  const folderPath = result.file_path.replace(/[/\\][^/\\]+$/, "");

  // 경로 복사
  const handleCopyPath = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      if (onCopyPath) {
        onCopyPath(result.file_path);
      } else {
        navigator.clipboard.writeText(result.file_path);
      }
    },
    [result.file_path, onCopyPath]
  );

  // 폴더 열기
  const handleOpenFolder = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      onOpenFolder?.(folderPath);
    },
    [folderPath, onOpenFolder]
  );

  return (
    <div
      className={`
        search-result-item result-card
        bg-gray-800 rounded-lg p-4 border
        ${isSelected
          ? "border-blue-500 ring-2 ring-blue-500/20 bg-blue-900/10"
          : "border-gray-700"
        }
      `}
      role="option"
      aria-selected={isSelected}
      style={{ "--item-index": index } as React.CSSProperties}
      tabIndex={isSelected ? 0 : -1}
    >
      {/* 헤더 */}
      <div className="flex items-start justify-between mb-2">
        <div
          className="flex items-center gap-2 cursor-pointer hover:text-blue-400 flex-1 min-w-0"
          onClick={() => onOpenFile(result.file_path, result.page_number)}
          title={result.page_number ? `${result.page_number}페이지로 열기` : "파일 열기"}
        >
          <FileIcon fileName={result.file_name} size="md" />
          <span className="font-medium text-white truncate">{result.file_name}</span>
        </div>

        {/* 액션 버튼 + 뱃지 */}
        <div className="flex items-center gap-1.5 ml-2 flex-shrink-0">
          {/* 액션 버튼들 */}
          <div className="flex items-center gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity">
            {/* 경로 복사 */}
            <button
              onClick={handleCopyPath}
              className="p-1 text-gray-500 hover:text-gray-300 rounded transition-colors"
              title="경로 복사"
              aria-label="파일 경로 복사"
            >
              <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M8 5H6a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2v-1M8 5a2 2 0 002 2h2a2 2 0 002-2M8 5a2 2 0 012-2h2a2 2 0 012 2m0 0h2a2 2 0 012 2v3m2 4H10m0 0l3-3m-3 3l3 3" />
              </svg>
            </button>

            {/* 폴더 열기 */}
            {onOpenFolder && (
              <button
                onClick={handleOpenFolder}
                className="p-1 text-gray-500 hover:text-gray-300 rounded transition-colors"
                title="폴더 열기"
                aria-label="상위 폴더 열기"
              >
                <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 19a2 2 0 01-2-2V7a2 2 0 012-2h4l2 2h4a2 2 0 012 2v1M5 19h14a2 2 0 002-2v-5a2 2 0 00-2-2H9a2 2 0 00-2 2v5a2 2 0 01-2 2z" />
                </svg>
              </button>
            )}
          </div>

          {/* 뱃지 */}
          {result.location_hint ? (
            <Badge variant="success">{result.location_hint}</Badge>
          ) : result.page_number ? (
            <Badge variant="primary">{result.page_number}p</Badge>
          ) : null}
          <Badge variant={getFileTypeBadgeVariant(result.file_name)}>
            {fileExt.toUpperCase()}
          </Badge>
        </div>
      </div>

      {/* 내용 */}
      <div className="cursor-pointer" onClick={onToggleExpand}>
        <p className="text-gray-300 text-sm leading-relaxed">
          <HighlightedText
            text={isExpanded ? result.full_content : result.content_preview}
            ranges={result.highlight_ranges}
          />
        </p>
        {!isExpanded && result.full_content.length > result.content_preview.length && (
          <span className="text-blue-400 text-xs mt-1 hover:underline inline-block">
            더보기 ▼
          </span>
        )}
        {isExpanded && (
          <span className="text-blue-400 text-xs mt-1 hover:underline inline-block">
            접기 ▲
          </span>
        )}
      </div>

      {/* 경로 (브레드크럼 스타일) */}
      <p className="text-gray-500 text-xs mt-2 truncate" title={result.file_path}>
        {formatBreadcrumb(folderPath)}
      </p>
    </div>
  );
}

/** 경로를 브레드크럼 형식으로 변환 */
function formatBreadcrumb(path: string): string {
  const parts = path.replace(/\\/g, "/").split("/").filter(Boolean);
  if (parts.length <= 3) {
    return parts.join(" › ");
  }
  // 처음 1개 + ... + 마지막 2개
  return `${parts[0]} › ... › ${parts.slice(-2).join(" › ")}`;
}
