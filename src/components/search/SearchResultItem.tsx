import { useCallback } from "react";
import type { SearchResult } from "../../types/search";
import { HighlightedText } from "./HighlightedText";
import { FileIcon } from "../ui/FileIcon";
import { Badge, getFileTypeBadgeVariant } from "../ui/Badge";
import { ConfidenceBadge } from "../ui/ConfidenceBadge";

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
      className="search-result-item result-card"
      style={{
        "--item-index": index,
        padding: "1.25rem 1.5rem",
        ...(isSelected && {
          borderColor: "var(--color-accent)",
          backgroundColor: "var(--color-accent-light)",
          boxShadow: "0 0 0 3px var(--color-accent-muted)",
        }),
      } as React.CSSProperties}
      role="option"
      aria-selected={isSelected}
      tabIndex={isSelected ? 0 : -1}
    >
      {/* 헤더 */}
      <div className="flex items-start justify-between mb-2">
        <div
          className="flex items-center gap-2.5 cursor-pointer flex-1 min-w-0 group/filename transition-colors duration-200"
          onClick={() => onOpenFile(result.file_path, result.page_number)}
          title={result.page_number ? `${result.page_number}페이지로 열기` : "파일 열기"}
          style={{ color: "var(--color-text-primary)" }}
          onMouseEnter={(e) => {
            e.currentTarget.style.color = "var(--color-accent)";
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.color = "var(--color-text-primary)";
          }}
        >
          <FileIcon fileName={result.file_name} size="md" />
          <span
            className="truncate"
            style={{ fontSize: "1.125rem", fontWeight: 600 }}
          >
            {result.file_name}
          </span>
          <svg
            className="w-3.5 h-3.5 flex-shrink-0 opacity-0 group-hover/filename:opacity-100 transition-opacity"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14" />
          </svg>
        </div>

        {/* 액션 버튼 + 뱃지 */}
        <div className="flex items-center gap-1.5 ml-2 flex-shrink-0">
          {/* 액션 버튼들 - 항상 노출 (opacity 0.5 → hover 1) */}
          <div className="flex items-center gap-0.5 opacity-50 group-hover:opacity-100 transition-opacity">
            {/* 경로 복사 */}
            <button
              onClick={handleCopyPath}
              className="p-1 rounded transition-colors"
              style={{ color: "var(--color-text-muted)" }}
              onMouseEnter={(e) => {
                e.currentTarget.style.color = "var(--color-text-secondary)";
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.color = "var(--color-text-muted)";
              }}
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
                className="p-1 rounded transition-colors"
                style={{ color: "var(--color-text-muted)" }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.color = "var(--color-text-secondary)";
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.color = "var(--color-text-muted)";
                }}
                title="폴더 열기"
                aria-label="상위 폴더 열기"
              >
                <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 19a2 2 0 01-2-2V7a2 2 0 012-2h4l2 2h4a2 2 0 012 2v1M5 19h14a2 2 0 002-2v-5a2 2 0 00-2-2H9a2 2 0 00-2 2v5a2 2 0 01-2 2z" />
                </svg>
              </button>
            )}
          </div>

          {/* 신뢰도 */}
          <ConfidenceBadge confidence={result.confidence} />

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
      <div
        className="cursor-pointer rounded-md p-2 -mx-2 transition-colors flex gap-2"
        onClick={onToggleExpand}
        style={{ backgroundColor: "transparent" }}
        onMouseEnter={(e) => {
          e.currentTarget.style.backgroundColor = "var(--color-bg-tertiary)";
        }}
        onMouseLeave={(e) => {
          e.currentTarget.style.backgroundColor = "transparent";
        }}
      >
        {/* 토글 아이콘 */}
        <svg
          className={`w-3 h-3 flex-shrink-0 mt-1 transition-transform ${isExpanded ? "rotate-90" : ""}`}
          style={{ color: "var(--color-text-muted)" }}
          fill="currentColor"
          viewBox="0 0 20 20"
        >
          <path fillRule="evenodd" d="M7.293 14.707a1 1 0 010-1.414L10.586 10 7.293 6.707a1 1 0 011.414-1.414l4 4a1 1 0 010 1.414l-4 4a1 1 0 01-1.414 0z" clipRule="evenodd" />
        </svg>
        <div className="flex-1 min-w-0">
          <p
            className="text-sm"
            style={{
              color: "var(--color-text-secondary)",
              lineHeight: "var(--leading-relaxed)",
              letterSpacing: "0.3px",
            }}
          >
            <HighlightedText
              text={isExpanded ? result.full_content : result.content_preview}
              ranges={result.highlight_ranges}
            />
          </p>
          {!isExpanded && result.full_content.length > result.content_preview.length && (
            <span className="text-xs mt-1 inline-block" style={{ color: "var(--color-accent)" }}>
              더보기
            </span>
          )}
        </div>
      </div>

      {/* 경로 (브레드크럼 스타일) */}
      <p
        className="text-xs mt-2 truncate font-mono"
        style={{ color: "var(--color-text-muted)" }}
        title={result.file_path}
      >
        {formatBreadcrumb(folderPath)}
      </p>
    </div>
  );
}

/** 경로를 브레드크럼 형식으로 변환 */
function formatBreadcrumb(path: string): string {
  // Windows long path prefix 제거
  let cleanPath = path.replace(/^\\\\\?\\/, "").replace(/^\/\/\?\//, "");
  const parts = cleanPath.replace(/\\/g, "/").split("/").filter(Boolean);
  if (parts.length <= 3) {
    return parts.join(" › ");
  }
  // 처음 1개 + ... + 마지막 2개
  return `${parts[0]} › ... › ${parts.slice(-2).join(" › ")}`;
}
