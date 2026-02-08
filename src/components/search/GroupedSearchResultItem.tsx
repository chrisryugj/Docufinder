import { useState, useEffect, useRef, useCallback, memo, useMemo } from "react";
import { createPortal } from "react-dom";
import type { GroupedSearchResult } from "../../types/search";
import { FileIcon } from "../ui/FileIcon";
import { Badge, getFileTypeBadgeVariant } from "../ui/Badge";
import { ConfidenceBadge } from "../ui/ConfidenceBadge";
import { HighlightedText } from "./HighlightedText";
import { buildPreviewContext } from "./searchTextUtils";
import { getMatchTypeBadge } from "./matchType";

interface GroupedSearchResultItemProps {
  group: GroupedSearchResult;
  onOpenFile: (filePath: string, page?: number | null) => void;
  onCopyPath?: (path: string) => void;
  onOpenFolder?: (path: string) => void;
  isCompact?: boolean;
  /** 검색어 - snippet 없을 때 클라이언트 하이라이트용 */
  searchQuery?: string;
  /** 펼침 상태 (부모에서 관리) */
  isExpanded?: boolean;
  /** 펼침 토글 콜백 */
  onToggleExpand?: () => void;
}

interface ContextMenuState {
  isOpen: boolean;
  x: number;
  y: number;
}

/**
 * 파일별로 그룹핑된 검색 결과 아이템
 * - 접혀있을 때: 파일명 + 매칭 수 + 최고 신뢰도
 * - 펼쳐졌을 때: 각 청크 미리보기
 *
 * memo() 적용: 불필요한 리렌더링 방지
 */
export const GroupedSearchResultItem = memo(function GroupedSearchResultItem({
  group,
  onOpenFile,
  onCopyPath,
  onOpenFolder,
  isCompact = false,
  searchQuery,
  isExpanded = false,
  onToggleExpand,
}: GroupedSearchResultItemProps) {
  const fileExt = group.file_name.split(".").pop()?.toLowerCase() || "";
  const folderPath = group.file_path.replace(/[/\\][^/\\]+$/, "");

  // 검색어를 키워드로 분리 (snippet 없을 때 폴백 하이라이트용)
  const fallbackKeywords = useMemo(() => {
    if (!searchQuery) return [];
    // 공백으로 분리, 빈 문자열 제거, 2글자 이상만
    return searchQuery.split(/\s+/).filter(k => k.length >= 2);
  }, [searchQuery]);

  // 컴팩트 모드: 2개만, 기본: 3개, 펼치면 전체
  const defaultCount = isCompact ? 2 : 3;
  const displayChunks = isExpanded ? group.chunks : group.chunks.slice(0, defaultCount);
  const hasMore = group.chunks.length > defaultCount;

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

    // 뷰포트 경계 체크로 메뉴 오버플로 방지
    const menuWidth = 200;
    const menuHeight = 160;
    const padding = 8;

    let x = e.clientX;
    let y = e.clientY;

    if (x + menuWidth > window.innerWidth - padding) {
      x = window.innerWidth - menuWidth - padding;
    }
    if (y + menuHeight > window.innerHeight - padding) {
      y = window.innerHeight - menuHeight - padding;
    }

    setContextMenu({ isOpen: true, x, y });
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

  const handleCopyPath = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (onCopyPath) {
      onCopyPath(group.file_path);
    } else {
      navigator.clipboard.writeText(group.file_path);
    }
  };

  const handleOpenFolder = (e: React.MouseEvent) => {
    e.stopPropagation();
    onOpenFolder?.(folderPath);
  };


  return (
    <div className="result-card" style={{ padding: isCompact ? "0.75rem 1rem" : "1rem 1.25rem" }}>
      {/* 그룹 헤더 */}
      <div className={`flex items-center justify-between ${isCompact ? "mb-2" : "mb-3"}`}>
        <div
          className={`flex items-center cursor-pointer flex-1 min-w-0 group/filename ${isCompact ? "gap-2" : "gap-2.5"}`}
          onClick={() => onOpenFile(group.file_path)}
          onContextMenu={handleContextMenu}
          title="파일 열기 (우클릭: 더 많은 옵션)"
          style={{ color: "var(--color-text-primary)" }}
          onMouseEnter={(e) => {
            e.currentTarget.style.color = "var(--color-accent)";
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.color = "var(--color-text-primary)";
          }}
        >
          <FileIcon fileName={group.file_name} size={isCompact ? "sm" : "md"} />
          <span className={`truncate font-semibold ${isCompact ? "text-sm" : "text-base"}`}>
            {group.file_name}
          </span>
          <Badge variant="default">{group.total_matches}건</Badge>
        </div>

        <div className="flex items-center gap-2 ml-2 flex-shrink-0">
          {/* 액션 버튼 */}
          <div className="flex items-center gap-0.5 opacity-50 group-hover:opacity-100 transition-opacity">
            <button
              onClick={handleCopyPath}
              className="p-1 rounded transition-colors"
              style={{ color: "var(--color-text-muted)" }}
              title="경로 복사"
            >
              <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M8 5H6a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2v-1M8 5a2 2 0 002 2h2a2 2 0 002-2M8 5a2 2 0 012-2h2a2 2 0 012 2m0 0h2a2 2 0 012 2v3m2 4H10m0 0l3-3m-3 3l3 3" />
              </svg>
            </button>
            {onOpenFolder && (
              <button
                onClick={handleOpenFolder}
                className="p-1 rounded transition-colors"
                style={{ color: "var(--color-text-muted)" }}
                title="폴더 열기"
              >
                <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 19a2 2 0 01-2-2V7a2 2 0 012-2h4l2 2h4a2 2 0 012 2v1M5 19h14a2 2 0 002-2v-5a2 2 0 00-2-2H9a2 2 0 00-2 2v5a2 2 0 01-2 2z" />
                </svg>
              </button>
            )}
          </div>

          {/* 신뢰도 */}
          <ConfidenceBadge confidence={group.top_confidence} />

          {/* 파일 타입 */}
          <Badge variant={getFileTypeBadgeVariant(group.file_name)}>
            {fileExt.toUpperCase()}
          </Badge>
        </div>
      </div>

      {/* 청크 목록 */}
      <div className={isCompact ? "space-y-1" : "space-y-2"}>
        {displayChunks.map((chunk, idx) => {
          const matchBadge = getMatchTypeBadge(chunk.match_type);
          // ⚡ full_content 대신 snippet/content_preview 사용 (성능 최적화)
          const effectiveFullText = chunk.full_content || chunk.snippet || chunk.content_preview;
          const preview = buildPreviewContext({
            previewText: chunk.content_preview,
            fullText: effectiveFullText,
            highlightRanges: chunk.highlight_ranges,
            snippet: chunk.snippet,
            query: searchQuery,
          });
          return (
          <div
            key={`${chunk.chunk_index}-${idx}`}
            className={`flex rounded-md cursor-pointer transition-colors ${isCompact ? "gap-1.5 p-1.5" : "gap-2 p-2"}`}
            style={{ backgroundColor: "var(--color-bg-secondary)" }}
            onClick={() => onOpenFile(chunk.file_path, chunk.page_number)}
            onMouseEnter={(e) => {
              e.currentTarget.style.backgroundColor = "var(--color-bg-tertiary)";
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.backgroundColor = "var(--color-bg-secondary)";
            }}
          >
            {/* 위치 표시 */}
            <div className={`flex-shrink-0 ${isCompact ? "w-16" : "w-20"} text-xs`} style={{ color: "var(--color-text-muted)" }}>
              <div>
                {chunk.location_hint || (chunk.page_number ? `${chunk.page_number}p` : `#${chunk.chunk_index + 1}`)}
              </div>
              {!isCompact && (
                <div className="mt-1">
                  <Badge variant={matchBadge.variant}>
                    {matchBadge.label}
                  </Badge>
                </div>
              )}
            </div>

            {/* 내용 미리보기 */}
            <div className="flex-1 min-w-0">
              <p
                className={`leading-relaxed ${isCompact ? "text-xs" : "text-sm"}`}
                style={{
                  color: "var(--color-text-secondary)",
                  display: "-webkit-box",
                  WebkitLineClamp: isCompact ? 2 : 4,
                  WebkitBoxOrient: "vertical",
                  overflow: "hidden",
                  whiteSpace: "pre-line",
                }}
              >
                <HighlightedText
                  text={preview.text}
                  ranges={preview.ranges}
                  refineKeywords={!chunk.snippet ? fallbackKeywords : undefined}
                  searchQuery={searchQuery}
                  formatMode="preview"
                />
              </p>
            </div>

            {/* 신뢰도 (컴팩트) */}
            <ConfidenceBadge confidence={chunk.confidence} compact showBar={false} />
          </div>
        );
        })}

        {/* 더보기/접기 */}
        {hasMore && onToggleExpand && (
          <button
            onClick={onToggleExpand}
            className={`w-full text-xs rounded-md transition-colors ${isCompact ? "py-1" : "py-1.5"}`}
            style={{
              color: "var(--color-accent)",
              backgroundColor: "var(--color-bg-secondary)",
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.backgroundColor = "var(--color-bg-tertiary)";
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.backgroundColor = "var(--color-bg-secondary)";
            }}
          >
            {isExpanded ? "접기" : `+${group.chunks.length - defaultCount}개 더보기`}
          </button>
        )}
      </div>

      {/* 경로 - 컴팩트 모드에서 숨김 */}
      {!isCompact && (
        <p
          className="text-xs mt-2 truncate font-mono"
          style={{ color: "var(--color-text-muted)" }}
          title={group.file_path}
        >
          {formatBreadcrumb(folderPath)}
        </p>
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
                onOpenFile(group.file_path);
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
                  onCopyPath(group.file_path);
                } else {
                  navigator.clipboard.writeText(group.file_path);
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

function formatBreadcrumb(path: string): string {
  let cleanPath = path.replace(/^\\\\\?\\/, "").replace(/^\/\/\?\//, "");
  const parts = cleanPath.replace(/\\/g, "/").split("/").filter(Boolean);
  if (parts.length <= 3) {
    return parts.join(" › ");
  }
  return `${parts[0]} › ... › ${parts.slice(-2).join(" › ")}`;
}
