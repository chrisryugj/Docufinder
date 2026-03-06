import { useCallback, memo } from "react";
import type { SearchResult } from "../../types/search";
import { HighlightedText } from "./HighlightedText";
import { buildPreviewContext } from "./searchTextUtils";
import { HighlightedFilename } from "./HighlightedFilename";
import { getMatchTypeBadge } from "./matchType";
import { FileIcon } from "../ui/FileIcon";
import { Badge, getFileTypeBadgeVariant } from "../ui/Badge";
import { ConfidenceBadge } from "../ui/ConfidenceBadge";
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
  /** 결과 내 검색 키워드 (추가 하이라이트용) */
  refineKeywords?: string[];
  /** 검색어 (파일명 하이라이트용) */
  query?: string;
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

  // 경로에서 폴더 추출
  const folderPath = result.file_path.replace(/[/\\][^/\\]+$/, "");

  // 수정일 포맷팅
  const modifiedAtMs = result.modified_at ? result.modified_at * 1000 : null;
  const relativeTime = modifiedAtMs ? formatRelativeTime(modifiedAtMs) : null;
  const absoluteDate = modifiedAtMs
    ? new Date(modifiedAtMs).toLocaleString("ko-KR", {
        year: "numeric", month: "2-digit", day: "2-digit",
        hour: "2-digit", minute: "2-digit",
      })
    : null;

  // 컨텍스트 메뉴
  const { contextMenu, handleContextMenu, closeContextMenu } = useContextMenu();

  // ⚡ full_content 대신 snippet/content_preview 사용 (성능 최적화)
  // snippet에서 [[HL]] 마커 제거 (펼친 상태에서 태그 노출 방지)
  const cleanSnippet = result.snippet?.replace(/\[\[HL\]\]/g, '').replace(/\[\[\/HL\]\]/g, '');
  const effectiveFullText = result.full_content || cleanSnippet || result.content_preview;
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
  const matchBadge = getMatchTypeBadge(result.match_type);

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
      id={`search-result-${index}`}
      className="search-result-item result-card"
      style={{
        "--item-index": index,
        padding: isCompact ? "0.375rem 0.5rem" : "0.75rem 1rem",
        ...(isSelected && {
          borderColor: "var(--color-accent)",
          backgroundColor: "var(--color-accent-light)",
          boxShadow: "0 0 0 3px var(--color-accent-muted)",
        }),
      } as React.CSSProperties}
      role="option"
      aria-selected={isSelected}
      aria-label={`${result.file_name} - ${result.match_type} 검색 결과`}
      tabIndex={isSelected ? 0 : -1}
      onContextMenu={handleContextMenu}
    >
      {/* 헤더 */}
      <div className={`flex items-start justify-between ${isCompact ? "mb-1" : "mb-2"}`}>
        <div
          className={`flex items-center cursor-pointer flex-1 min-w-0 group/filename hover-accent-text ${isCompact ? "gap-2" : "gap-2.5"}`}
          onClick={() => onOpenFile(result.file_path, result.page_number)}
          title={[
            result.page_number ? `${result.page_number}페이지로 열기` : "파일 열기",
            absoluteDate ? `수정: ${relativeTime} (${absoluteDate})` : null,
          ].filter(Boolean).join("\n")}
        >
          <FileIcon fileName={result.file_name} size={isCompact ? "sm" : "md"} />
          <span
            className="truncate"
            style={{ fontSize: isCompact ? "0.9375rem" : "1.125rem", fontWeight: 600 }}
          >
            <HighlightedFilename filename={result.file_name} query={query} />
          </span>
          {relativeTime && (
            <Tooltip
              content={absoluteDate}
              position="bottom"
              delay={200}
            >
              <span
                className="flex-shrink-0 text-[10px] font-normal"
                style={{ color: "var(--color-text-muted)" }}
              >
                {relativeTime}
              </span>
            </Tooltip>
          )}
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
              className="p-1.5 rounded btn-icon-hover"
              title="경로 복사"
              aria-label="파일 경로 복사"
            >
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M8 5H6a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2v-1M8 5a2 2 0 002 2h2a2 2 0 002-2M8 5a2 2 0 012-2h2a2 2 0 012 2m0 0h2a2 2 0 012 2v3m2 4H10m0 0l3-3m-3 3l3 3" />
              </svg>
            </button>

            {/* 폴더 열기 */}
            {onOpenFolder && (
              <button
                onClick={handleOpenFolder}
                className="p-1.5 rounded btn-icon-hover"
                title="폴더 열기"
                aria-label="상위 폴더 열기"
              >
                <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
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
          <Badge variant={matchBadge.variant}>{matchBadge.label}</Badge>
          <Badge variant={getFileTypeBadgeVariant(result.file_name)}>
            {fileExt.toUpperCase()}
          </Badge>
        </div>
      </div>

      {/* 내용 */}
      <div
        className={`cursor-pointer rounded-md flex gap-2 hover-bg-tertiary ${isCompact ? "p-1 -mx-1" : "p-2 -mx-2"}`}
        onClick={onToggleExpand}
      >
        {/* 토글 아이콘 */}
        <svg
          className={`w-3 h-3 flex-shrink-0 mt-0.5 transition-transform ${isExpanded ? "rotate-90" : ""}`}
          style={{ color: "var(--color-text-muted)" }}
          fill="currentColor"
          viewBox="0 0 20 20"
        >
          <path fillRule="evenodd" d="M7.293 14.707a1 1 0 010-1.414L10.586 10 7.293 6.707a1 1 0 011.414-1.414l4 4a1 1 0 010 1.414l-4 4a1 1 0 01-1.414 0z" clipRule="evenodd" />
        </svg>
        <div className="flex-1 min-w-0">
          <p
            className={isCompact ? "text-xs" : "text-sm"}
            style={{
              color: "var(--color-text-secondary)",
              lineHeight: isCompact ? "1.5" : "1.6",
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

      {/* 경로 (Windows 스타일 배지) - 컴팩트 모드에서는 숨김 */}
      {!isCompact && (
        <div
          className="flex flex-wrap items-center gap-1 mt-3"
          title={result.file_path.replace(/^\\\\\?\\/, "")}
        >
          {formatPathToBadges(folderPath).map((part, i, arr) => (
            <div key={i} className="flex items-center">
              <span
                className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-medium"
                style={{
                  backgroundColor: "var(--color-bg-tertiary)",
                  border: "1px solid var(--color-border)",
                  color: "var(--color-text-muted)",
                }}
              >
                {part}
              </span>
              {i < arr.length - 1 && (
                <span className="mx-0.5" style={{ color: "var(--color-border-hover)" }}>
                  <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                  </svg>
                </span>
              )}
            </div>
          ))}
        </div>
      )}

      {/* 컨텍스트 메뉴 */}
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

/** 경로를 배열로 변환 (Windows 스타일) - 깊은 경로는 말줄임 */
function formatPathToBadges(path: string): string[] {
  // Windows long path prefix 제거 및 백슬래시 통일
  const cleanPath = path.replace(/^\\\\\?\\/, "").replace(/^\/\/\?\//, "");
  // 드라이브 레터와 폴더 분리
  const parts = cleanPath.split(/[/\\]/).filter(Boolean);
  // 5개 초과 시 처음 2개 + ... + 마지막 2개
  if (parts.length > 5) {
    return [...parts.slice(0, 2), "\u2026", ...parts.slice(-2)];
  }
  return parts;
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
    // 하이라이트 없으면 앞부분만 제한
    const limitedText = fullText.slice(0, 600);
    return {
      text: limitedText + (fullText.length > 600 ? "..." : ""),
      ranges,
    };
  }

  // 하이라이트 전후로 컨텍스트 제한
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

  // 앞뒤 생략 표시
  const prefix = startOffset > 0 ? "..." : "";
  const suffix = endOffset < fullText.length ? "..." : "";
  const finalText = prefix + trimmedText + suffix;

  // prefix로 인한 range offset 조정
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
    if (!segment.includes("[[HL]]")) {
      continue;
    }

    const parsed = parseSnippetSegment(segment);
    if (!parsed.text.trim()) {
      continue;
    }

    let searchStart = 0;
    while (true) {
      const index = fullText.indexOf(parsed.text, searchStart);
      if (index === -1) {
        break;
      }

      const candidate = parsed.ranges.length
        ? {
            start: index + parsed.ranges[0][0],
            end: index + parsed.ranges[0][1],
          }
        : { start: index, end: index + parsed.text.length };

      if (!fallback) {
        fallback = candidate;
      }

      if (ranges.some(([rangeStart, rangeEnd]) => candidate.start >= rangeStart && candidate.end <= rangeEnd)) {
        return candidate;
      }

      searchStart = index + parsed.text.length;
    }
  }

  return fallback;
}

function parseSnippetSegment(
  segment: string
): { text: string; ranges: [number, number][] } {
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

function findFirstRangeAnchor(
  ranges: [number, number][]
): { start: number; end: number } | null {
  if (ranges.length === 0) {
    return null;
  }

  let [start, end] = ranges[0];
  for (const [rangeStart, rangeEnd] of ranges) {
    if (rangeStart < start) {
      start = rangeStart;
      end = rangeEnd;
    }
  }

  return { start, end };
}
