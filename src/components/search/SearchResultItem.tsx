import { useCallback, memo } from "react";
import type { SearchResult } from "../../types/search";
import { HighlightedText } from "./HighlightedText";
import { buildPreviewContext } from "./searchTextUtils";
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
        navigator.clipboard.writeText(result.file_path);
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
        padding: isCompact ? "0.375rem 0.625rem" : "0.625rem 0.875rem",
        ...(isSelected && {
          borderColor: "var(--color-accent)",
          backgroundColor: "var(--color-accent-light)",
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
            className="truncate"
            style={{ fontSize: "14px", fontWeight: 700, letterSpacing: "-0.01em" }}
          >
            <HighlightedFilename filename={result.file_name} query={query} />
          </span>
          <svg
            className="w-3.5 h-3.5 flex-shrink-0 opacity-0 group-hover/filename:opacity-60 transition-opacity"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14" />
          </svg>
        </div>

        {/* Right side: confidence % + time + file type */}
        <div className="flex items-center gap-2 ml-2 flex-shrink-0">
          {/* Confidence — number only */}
          <span
            className="text-xs font-semibold tabular-nums"
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
                className="text-[11px] tabular-nums"
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
        </div>
      </div>

      {/* Row 2: Content preview */}
      <div
        className="cursor-pointer rounded flex gap-1.5 hover-bg-tertiary -mx-1.5 px-1.5 py-1"
        onClick={onToggleExpand}
      >
        <svg
          className={`w-3 h-3 flex-shrink-0 mt-1 transition-transform ${isExpanded ? "rotate-180" : ""}`}
          style={{ color: "var(--color-text-muted)" }}
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
        </svg>
        <div className="flex-1 min-w-0">
          <p
            style={{
              color: "var(--color-text-secondary)",
              fontSize: "13px",
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

      {/* Row 3: Path + action buttons */}
      {!isCompact && (
        <div className="flex items-center justify-between mt-1.5">
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
                    className="text-xs px-0.5 py-0.5 rounded transition-colors hover:underline"
                    style={{ color: "var(--color-text-muted)" }}
                    onMouseEnter={(e) => {
                      e.currentTarget.style.color = "var(--color-accent)";
                    }}
                    onMouseLeave={(e) => {
                      e.currentTarget.style.color = "var(--color-text-muted)";
                    }}
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
              <svg className="w-3.5 h-3.5" style={{ color: "var(--color-accent)" }} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M8 5H6a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2v-1M8 5a2 2 0 002 2h2a2 2 0 002-2M8 5a2 2 0 012-2h2a2 2 0 012 2m0 0h2a2 2 0 012 2v3m2 4H10m0 0l3-3m-3 3l3 3" />
              </svg>
            </button>
            {onOpenFolder && (
              <button
                onClick={handleOpenFolder}
                className="p-1 rounded btn-icon-hover"
                title="폴더 열기"
                aria-label="상위 폴더 열기"
              >
                <svg className="w-3.5 h-3.5" style={{ color: "var(--color-warning)" }} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 19a2 2 0 01-2-2V7a2 2 0 012-2h4l2 2h4a2 2 0 012 2v1M5 19h14a2 2 0 002-2v-5a2 2 0 00-2-2H9a2 2 0 00-2 2v5a2 2 0 01-2 2z" />
                </svg>
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

function formatPathSegments(path: string): { label: string; fullPath: string }[] {
  const cleanPath = path.replace(/^\\\\\?\\/, "").replace(/^\/\/\?\//, "");
  const parts = cleanPath.split(/[/\\]/).filter(Boolean);

  const segments = parts.map((part, i) => ({
    label: part,
    fullPath: parts.slice(0, i + 1).join("\\"),
  }));

  if (segments.length > 6) {
    return [
      ...segments.slice(0, 2),
      { label: "\u2026", fullPath: "" },
      ...segments.slice(-2),
    ];
  }
  return segments;
}

const EXPANDED_CONTEXT_BEFORE_CHARS = 300;
const EXPANDED_CONTEXT_AFTER_CHARS = 300;

function buildExpandedContext(
  fullText: string,
  ranges: [number, number][],
  snippet?: string
): { text: string; ranges: [number, number][] } {
  const anchor = snippet
    ? findSnippetAnchor(fullText, snippet, ranges)
    : findFirstRangeAnchor(ranges);
  const effectiveAnchor = anchor ?? findFirstRangeAnchor(ranges);
  if (!effectiveAnchor) {
    const limitedText = fullText.slice(0, 600);
    return {
      text: limitedText + (fullText.length > 600 ? "..." : ""),
      ranges,
    };
  }

  const startOffset = Math.max(0, effectiveAnchor.start - EXPANDED_CONTEXT_BEFORE_CHARS);
  const endOffset = Math.min(fullText.length, effectiveAnchor.end + EXPANDED_CONTEXT_AFTER_CHARS);

  const trimmedText = fullText.slice(startOffset, endOffset);
  const trimmedRanges = ranges
    .filter(([start, end]) => end > startOffset && start < endOffset)
    .map(([start, end]) => {
      const clippedStart = Math.max(0, start - startOffset);
      const clippedEnd = Math.min(trimmedText.length, end - startOffset);
      return [clippedStart, clippedEnd] as [number, number];
    });

  const prefix = startOffset > 0 ? "..." : "";
  const suffix = endOffset < fullText.length ? "..." : "";
  const finalText = prefix + trimmedText + suffix;

  const offsetAdjust = prefix.length;
  const adjustedRanges = trimmedRanges.map(
    ([start, end]) => [start + offsetAdjust, end + offsetAdjust] as [number, number]
  );

  return { text: finalText, ranges: adjustedRanges };
}

function findSnippetAnchor(
  fullText: string,
  snippet: string,
  ranges: [number, number][]
): { start: number; end: number } | null {
  const segments = snippet.split("...");
  let fallback: { start: number; end: number } | null = null;

  for (const segment of segments) {
    if (!segment.includes("[[HL]]")) continue;
    const parsed = parseSnippetSegment(segment);
    if (!parsed.text.trim()) continue;

    let searchStart = 0;
    while (true) {
      const index = fullText.indexOf(parsed.text, searchStart);
      if (index === -1) break;

      const candidate = parsed.ranges.length
        ? { start: index + parsed.ranges[0][0], end: index + parsed.ranges[0][1] }
        : { start: index, end: index + parsed.text.length };

      if (!fallback) fallback = candidate;

      if (ranges.some(([rangeStart, rangeEnd]) => candidate.start >= rangeStart && candidate.end <= rangeEnd)) {
        return candidate;
      }
      searchStart = index + parsed.text.length;
    }
  }
  return fallback;
}

function parseSnippetSegment(segment: string): { text: string; ranges: [number, number][] } {
  const ranges: [number, number][] = [];
  let text = "";
  let i = 0;

  while (i < segment.length) {
    if (segment.slice(i, i + 6) === "[[HL]]") {
      const start = text.length;
      i += 6;
      const endMarker = segment.indexOf("[[/HL]]", i);
      if (endMarker !== -1) {
        text += segment.slice(i, endMarker);
        ranges.push([start, text.length]);
        i = endMarker + 7;
      } else {
        text += segment.slice(i);
        ranges.push([start, text.length]);
        break;
      }
    } else {
      text += segment[i];
      i += 1;
    }
  }
  return { text, ranges };
}

function findFirstRangeAnchor(ranges: [number, number][]): { start: number; end: number } | null {
  if (ranges.length === 0) return null;
  let [start, end] = ranges[0];
  for (const [rangeStart, rangeEnd] of ranges) {
    if (rangeStart < start) { start = rangeStart; end = rangeEnd; }
  }
  return { start, end };
}
