import { useCallback, memo, useState, useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import type { SearchResult } from "../../types/search";
import { HighlightedText } from "./HighlightedText";
import { buildPreviewContext } from "./searchTextUtils";
import { HighlightedFilename } from "./HighlightedFilename";
import { getMatchTypeBadge } from "./matchType";
import { FileIcon } from "../ui/FileIcon";
import { Badge, getFileTypeBadgeVariant } from "../ui/Badge";
import { ConfidenceBadge } from "../ui/ConfidenceBadge";


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

interface ContextMenuState {
  isOpen: boolean;
  x: number;
  y: number;
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

  // 컨텍스트 메뉴 상태
  const [contextMenu, setContextMenu] = useState<ContextMenuState>({
    isOpen: false,
    x: 0,
    y: 0,
  });
  const contextMenuRef = useRef<HTMLDivElement>(null);

  // 컨텍스트 메뉴 열기
  const handleContextMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setContextMenu({
      isOpen: true,
      x: e.clientX,
      y: e.clientY,
    });
  }, []);

  // 컨텍스트 메뉴 닫기
  const closeContextMenu = useCallback(() => {
    setContextMenu((prev) => ({ ...prev, isOpen: false }));
  }, []);

  // 외부 클릭 시 메뉴 닫기
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (contextMenuRef.current && !contextMenuRef.current.contains(e.target as Node)) {
        closeContextMenu();
      }
    };
    if (contextMenu.isOpen) {
      document.addEventListener("mousedown", handleClickOutside);
    }
    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
    };
  }, [contextMenu.isOpen, closeContextMenu]);
  const expandedView = isExpanded
    ? buildExpandedContext(result.full_content, result.highlight_ranges, result.snippet)
    : null;
  const previewView = !isExpanded
    ? buildPreviewContext({
        previewText: result.content_preview,
        fullText: result.full_content,
        highlightRanges: result.highlight_ranges,
        snippet: result.snippet,
        query,
      })
    : null;
  const displayText = isExpanded
    ? expandedView?.text ?? result.full_content
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
      tabIndex={isSelected ? 0 : -1}
    >
      {/* 헤더 */}
      <div className={`flex items-start justify-between ${isCompact ? "mb-1" : "mb-2"}`}>
        <div
          className={`flex items-center cursor-pointer flex-1 min-w-0 group/filename transition-colors duration-200 ${isCompact ? "gap-2" : "gap-2.5"}`}
          onClick={() => onOpenFile(result.file_path, result.page_number)}
          onContextMenu={handleContextMenu}
          title={result.page_number ? `${result.page_number}페이지로 열기` : "파일 열기 (우클릭: 더 많은 옵션)"}
          style={{ color: "var(--color-text-primary)" }}
          onMouseEnter={(e) => {
            e.currentTarget.style.color = "var(--color-accent)";
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.color = "var(--color-text-primary)";
          }}
        >
          <FileIcon fileName={result.file_name} size={isCompact ? "sm" : "md"} />
          <span
            className="truncate"
            style={{ fontSize: isCompact ? "0.9375rem" : "1.125rem", fontWeight: 600 }}
          >
            <HighlightedFilename filename={result.file_name} query={query} />
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
          <Badge variant={matchBadge.variant}>{matchBadge.label}</Badge>
          <Badge variant={getFileTypeBadgeVariant(result.file_name)}>
            {fileExt.toUpperCase()}
          </Badge>
        </div>
      </div>

      {/* 내용 */}
      <div
        className={`cursor-pointer rounded-md transition-colors flex gap-2 ${isCompact ? "p-1 -mx-1" : "p-2 -mx-2"}`}
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

      {/* 컨텍스트 메뉴 (Portal로 body에 렌더링) */}
      {contextMenu.isOpen &&
        createPortal(
          <div
            ref={contextMenuRef}
            className="fixed min-w-[140px] py-1 rounded-lg shadow-xl border"
            style={{
              left: contextMenu.x,
              top: contextMenu.y,
              zIndex: 9999,
              backgroundColor: "var(--color-bg-secondary)",
              borderColor: "var(--color-border)",
            }}
          >
            {/* 파일 열기 */}
            <button
              onClick={() => {
                closeContextMenu();
                onOpenFile(result.file_path, result.page_number);
              }}
              className="w-full px-3 py-2 text-left text-sm flex items-center gap-2 transition-colors"
              style={{ color: "var(--color-text-primary)" }}
              onMouseEnter={(e) => {
                e.currentTarget.style.backgroundColor = "var(--color-accent-light)";
                e.currentTarget.style.color = "var(--color-accent)";
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.backgroundColor = "transparent";
                e.currentTarget.style.color = "var(--color-text-primary)";
              }}
            >
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14" />
              </svg>
              파일 열기
            </button>

            {/* 폴더 열기 */}
            {onOpenFolder && (
              <button
                onClick={() => {
                  closeContextMenu();
                  onOpenFolder(folderPath);
                }}
                className="w-full px-3 py-2 text-left text-sm flex items-center gap-2 transition-colors"
                style={{ color: "var(--color-text-primary)" }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.backgroundColor = "var(--color-accent-light)";
                  e.currentTarget.style.color = "var(--color-accent)";
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.backgroundColor = "transparent";
                  e.currentTarget.style.color = "var(--color-text-primary)";
                }}
              >
                <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 19a2 2 0 01-2-2V7a2 2 0 012-2h4l2 2h4a2 2 0 012 2v1M5 19h14a2 2 0 002-2v-5a2 2 0 00-2-2H9a2 2 0 00-2 2v5a2 2 0 01-2 2z" />
                </svg>
                폴더 열기
              </button>
            )}

            {/* 경로 복사 */}
            <button
              onClick={() => {
                closeContextMenu();
                if (onCopyPath) {
                  onCopyPath(result.file_path);
                } else {
                  navigator.clipboard.writeText(result.file_path);
                }
              }}
              className="w-full px-3 py-2 text-left text-sm flex items-center gap-2 transition-colors"
              style={{ color: "var(--color-text-primary)" }}
              onMouseEnter={(e) => {
                e.currentTarget.style.backgroundColor = "var(--color-accent-light)";
                e.currentTarget.style.color = "var(--color-accent)";
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.backgroundColor = "transparent";
                e.currentTarget.style.color = "var(--color-text-primary)";
              }}
            >
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M8 5H6a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2v-1M8 5a2 2 0 002 2h2a2 2 0 002-2M8 5a2 2 0 012-2h2a2 2 0 012 2m0 0h2a2 2 0 012 2v3m2 4H10m0 0l3-3m-3 3l3 3" />
              </svg>
              경로 복사
            </button>
          </div>,
          document.body
        )}
    </div>
  );
});

/** 경로를 배열로 변환 (Windows 스타일) */
function formatPathToBadges(path: string): string[] {
  // Windows long path prefix 제거 및 백슬래시 통일
  let cleanPath = path.replace(/^\\\\\?\\/, "").replace(/^\/\/\?\//, "");
  // 드라이브 레터와 폴더 분리
  return cleanPath.split(/[/\\]/).filter(Boolean);
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
