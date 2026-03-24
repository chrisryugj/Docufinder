import { useCallback, memo } from "react";
import { ExternalLink, ChevronDown, ClipboardCopy, FolderOpen, Search } from "lucide-react";
import type { SearchResult } from "../../types/search";
import { HighlightedText } from "./HighlightedText";
import { buildPreviewContext } from "./searchTextUtils";
import { formatPathSegments, buildExpandedContext } from "../../utils/searchTextUtils";
import { HighlightedFilename } from "./HighlightedFilename";
import { FileIcon } from "../ui/FileIcon";
import { Badge, getFileTypeBadgeVariant } from "../ui/Badge";
import { Tooltip } from "../ui/Tooltip";
import { formatRelativeTime } from "../../utils/formatRelativeTime";
import { useContextMenu, ResultContextMenu } from "./ResultContextMenu";

interface SearchResultItemProps {
  result: SearchResult;
  index: number;
  isExpanded: boolean;
  isSelected?: boolean;
  isCompact?: boolean;
  onToggleExpand: () => void;
  onOpenFile: (filePath: string, page?: number | null) => void;
  onCopyPath?: (path: string) => void;
  onOpenFolder?: (path: string) => void;
  refineKeywords?: string[];
  query?: string;
  onFindSimilar?: (filePath: string) => void;
  category?: string;
}

/** Get file-type stripe CSS class */
function getStripeClass(fileName: string): string {
  const ext = fileName.split(".").pop()?.toLowerCase() || "";
  const map: Record<string, string> = {
    hwpx: "result-stripe-hwpx",
    hwp: "result-stripe-hwp",
    docx: "result-stripe-docx",
    doc: "result-stripe-docx",
    xlsx: "result-stripe-xlsx",
    xls: "result-stripe-xlsx",
    pdf: "result-stripe-pdf",
    pptx: "result-stripe-pptx",
    txt: "result-stripe-txt",
  };
  return map[ext] || "result-stripe-txt";
}

export const SearchResultItem = memo(function SearchResultItem({
  result,
  index,
  isExpanded,
  isSelected = false,
  isCompact = false,
  onToggleExpand,
  onOpenFile,
  onCopyPath,
  onOpenFolder,
  refineKeywords,
  query = "",
  onFindSimilar,
  category,
}: SearchResultItemProps) {
  const fileExt = result.file_name.split(".").pop()?.toLowerCase() || "";
  const folderPath = result.file_path.replace(/[/\\][^/\\]+$/, "");

  // Modified date
  const modifiedAtMs = result.modified_at ? result.modified_at * 1000 : null;
  const relativeTime = modifiedAtMs ? formatRelativeTime(modifiedAtMs) : null;
  const absoluteDate = modifiedAtMs
    ? new Date(modifiedAtMs).toLocaleString("ko-KR", {
        year: "numeric", month: "2-digit", day: "2-digit",
        hour: "2-digit", minute: "2-digit",
      })
    : null;

  // Context menu
  const { contextMenu, handleContextMenu, closeContextMenu } = useContextMenu();

  // Text processing
  const cleanSnippet = result.snippet?.replace(/\[\[HL\]\]/g, '').replace(/\[\[\/HL\]\]/g, '');
  const effectiveFullText = cleanSnippet || result.content_preview;
  const expandedView = isExpanded
    ? buildExpandedContext(effectiveFullText, result.highlight_ranges, result.snippet)
    : null;
  const previewView = !isExpanded
    ? buildPreviewContext({
        previewText: result.content_preview,
        fullText: effectiveFullText,
        highlightRanges: result.highlight_ranges,
        snippet: result.snippet,
        query,
      })
    : null;
  const displayText = isExpanded
    ? expandedView?.text ?? effectiveFullText
    : previewView?.text ?? result.content_preview;
  const displayRanges = isExpanded
    ? expandedView?.ranges ?? result.highlight_ranges
    : previewView?.ranges ?? [];

  const handleCopyPath = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      if (onCopyPath) {
        onCopyPath(result.file_path);
      } else {
        navigator.clipboard.writeText(result.file_path).catch(() => {});
      }
    },
    [result.file_path, onCopyPath]
  );

  const handleOpenFolder = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      onOpenFolder?.(folderPath);
    },
    [folderPath, onOpenFolder]
  );

  return (
    <div
      id={`search-result-${index}`}
      className={`search-result-item result-card ${getStripeClass(result.file_name)}`}
      style={{
        "--item-index": index,
        padding: isCompact ? "0.5rem 0.625rem" : "0.75rem 0.875rem",
        ...(isSelected && {
          backgroundColor: "var(--color-accent-light)",
          boxShadow: "inset 3px 0 0 var(--color-accent)",
        }),
      } as React.CSSProperties}
      role="option"
      aria-selected={isSelected}
      aria-label={`${result.file_name} 검색 결과`}
      tabIndex={isSelected ? 0 : -1}
      onContextMenu={handleContextMenu}
      data-context-menu
    >
      {/* Row 1: Filename + confidence + time */}
      <div className="flex items-center justify-between mb-1.5">
        <div
          className="flex items-center cursor-pointer flex-1 min-w-0 group/filename hover-accent-text gap-2"
          onClick={() => onOpenFile(result.file_path, result.page_number)}
          title={result.page_number ? `${result.page_number}페이지로 열기` : "파일 열기"}
        >
          <FileIcon fileName={result.file_name} size="sm" />
          <span
            className="truncate ts-base"
            style={{ fontWeight: 700, letterSpacing: "-0.01em" }}
          >
            <HighlightedFilename filename={result.file_name} query={query} />
          </span>
          <ExternalLink className="w-3.5 h-3.5 flex-shrink-0 opacity-0 group-hover/filename:opacity-60 transition-opacity" />
        </div>

        {/* Right side: confidence % + time + file type */}
        <div className="flex items-center gap-2 ml-2 flex-shrink-0">
          {/* Confidence — number only */}
          <span
            className="text-[11px] font-semibold tabular-nums leading-none"
            style={{
              color: result.confidence >= 70
                ? "var(--color-success)"
                : result.confidence >= 40
                  ? "var(--color-warning)"
                  : "var(--color-text-muted)",
            }}
          >
            {Math.round(result.confidence)}%
          </span>

          {/* Relative time */}
          {relativeTime && (
            <Tooltip content={absoluteDate} position="bottom" delay={200}>
              <span
                className="text-[11px] tabular-nums leading-none"
                style={{ color: "var(--color-text-muted)" }}
              >
                {relativeTime}
              </span>
            </Tooltip>
          )}

          {/* File type badge */}
          <Badge variant={getFileTypeBadgeVariant(result.file_name)}>
            {fileExt.toUpperCase()}
          </Badge>
          {category && category !== "기타" && (
            <Badge variant="secondary">{category}</Badge>
          )}
        </div>
      </div>

      {/* Row 2: Content preview (pl-6 = FileIcon 16px + gap 8px 정렬) */}
      <div
        className="cursor-pointer rounded flex gap-1.5 hover-bg-tertiary -mx-1.5 px-1.5 py-1 pl-6"
        onClick={onToggleExpand}
      >
        <ChevronDown
          className={`w-3 h-3 flex-shrink-0 mt-1 transition-transform ${isExpanded ? "rotate-180" : ""}`}
          style={{ color: "var(--color-text-muted)" }}
        />
        <div className="flex-1 min-w-0">
          <p
            style={{
              color: "var(--color-text-secondary)",
              fontSize: "var(--text-sm)",
              lineHeight: "1.7",
              letterSpacing: "0.3px",
              ...(!isExpanded && {
                display: "-webkit-box",
                WebkitLineClamp: 2,
                WebkitBoxOrient: "vertical" as const,
                overflow: "hidden",
              }),
            }}
          >
            <HighlightedText
              text={displayText}
              ranges={displayRanges}
              refineKeywords={refineKeywords}
              searchQuery={query}
              formatMode={isExpanded ? "full" : "preview"}
            />
          </p>
        </div>
      </div>

      {/* Row 3: Path + action buttons (pl-6 = Row 1 FileIcon 정렬) */}
      {!isCompact && (
        <div className="flex items-center justify-between mt-1.5 pl-6">
          {/* Breadcrumb path */}
          <div
            className="flex flex-wrap items-center gap-0.5 flex-1 min-w-0"
            title={result.file_path.replace(/^\\\\\?\\/, "")}
          >
            {formatPathSegments(folderPath).map((seg, i, arr) => (
              <div key={i} className="flex items-center leading-none">
                {seg.fullPath ? (
                  <button
                    onClick={(e) => { e.stopPropagation(); onOpenFolder?.(seg.fullPath); }}
                    className="text-xs px-0.5 py-0.5 rounded transition-colors hover:underline clr-muted hover-accent-text"
                    title={`${seg.fullPath} 열기`}
                  >
                    {seg.label}
                  </button>
                ) : (
                  <span className="text-xs px-0.5 py-0.5" style={{ color: "var(--color-text-muted)", opacity: 0.5 }}>
                    {seg.label}
                  </span>
                )}
                {i < arr.length - 1 && (
                  <span className="text-[11px] mx-px" style={{ color: "var(--color-text-muted)", opacity: 0.3 }}>/</span>
                )}
              </div>
            ))}
          </div>

          {/* Action buttons — always visible, colored */}
          <div className="flex items-center gap-0.5 ml-2 flex-shrink-0">
            {result.page_number && (
              <span className="text-[10px] px-1.5 py-0.5 rounded" style={{ backgroundColor: "var(--color-bg-tertiary)", color: "var(--color-text-muted)" }}>
                {result.page_number}p
              </span>
            )}
            <button
              onClick={handleCopyPath}
              className="p-1 rounded btn-icon-hover"
              title="경로 복사"
              aria-label="파일 경로 복사"
            >
              <ClipboardCopy className="w-3.5 h-3.5" style={{ color: "var(--color-accent)" }} />
            </button>
            {onOpenFolder && (
              <button
                onClick={handleOpenFolder}
                className="p-1 rounded btn-icon-hover"
                title="폴더 열기"
                aria-label="상위 폴더 열기"
              >
                <FolderOpen className="w-3.5 h-3.5" style={{ color: "var(--color-warning)" }} />
              </button>
            )}
            {onFindSimilar && (
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  onFindSimilar(result.file_path);
                }}
                className="p-1 rounded btn-icon-hover"
                title="유사 문서 찾기"
                aria-label="유사 문서 찾기"
              >
                <Search className="w-3.5 h-3.5" style={{ color: "var(--color-info)" }} />
              </button>
            )}
          </div>
        </div>
      )}

      <ResultContextMenu
        filePath={result.file_path}
        folderPath={folderPath}
        pageNumber={result.page_number}
        onOpenFile={onOpenFile}
        onCopyPath={onCopyPath}
        onOpenFolder={onOpenFolder}
        contextMenu={contextMenu}
        closeContextMenu={closeContextMenu}
      />
    </div>
  );
});
