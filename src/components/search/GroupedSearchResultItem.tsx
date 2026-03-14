import { memo, useMemo } from "react";
import { ClipboardCopy, FolderOpen } from "lucide-react";
import type { GroupedSearchResult } from "../../types/search";
import { FileIcon } from "../ui/FileIcon";
import { Badge, getFileTypeBadgeVariant } from "../ui/Badge";
import { HighlightedText } from "./HighlightedText";
import { buildPreviewContext } from "./searchTextUtils";
import { useContextMenu, ResultContextMenu } from "./ResultContextMenu";

interface GroupedSearchResultItemProps {
  domId?: string;
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

/**
 * 파일별로 그룹핑된 검색 결과 아이템
 * - 접혀있을 때: 파일명 + 매칭 수 + 최고 신뢰도
 * - 펼쳐졌을 때: 각 청크 미리보기
 *
 * memo() 적용: 불필요한 리렌더링 방지
 */
export const GroupedSearchResultItem = memo(function GroupedSearchResultItem({
  domId,
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
  const stripeClass = getStripeClass(group.file_name);

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

  // 컨텍스트 메뉴 (공용 훅 사용)
  const { contextMenu, handleContextMenu, closeContextMenu } = useContextMenu();

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
    <div id={domId} className={`result-card ${stripeClass}`} style={{ padding: isCompact ? "0.625rem 0.875rem" : "0.75rem 1rem" }} onContextMenu={handleContextMenu} data-context-menu>
      {/* 그룹 헤더 */}
      <div className={`flex items-center justify-between ${isCompact ? "mb-2" : "mb-3"}`}>
        <div
          className={`flex items-center cursor-pointer flex-1 min-w-0 group/filename hover-accent-text ${isCompact ? "gap-2" : "gap-2.5"}`}
          onClick={() => onOpenFile(group.file_path)}
          title="파일 열기 (우클릭: 더 많은 옵션)"
        >
          <FileIcon fileName={group.file_name} size={isCompact ? "sm" : "md"} />
          <span className={`truncate font-semibold ${isCompact ? "text-sm" : "text-base"}`}>
            {group.file_name}
          </span>
          <Badge variant="default">{group.total_matches}건</Badge>
        </div>

        <div className="flex items-center gap-2 ml-2 flex-shrink-0">
          {/* 액션 버튼 — colored */}
          <div className="flex items-center gap-1">
            <button
              onClick={handleCopyPath}
              className="p-1.5 rounded transition-colors btn-icon-hover"
              style={{ color: "var(--color-accent)" }}
              title="경로 복사"
            >
              <ClipboardCopy className="w-4 h-4" />
            </button>
            {onOpenFolder && (
              <button
                onClick={handleOpenFolder}
                className="p-1.5 rounded transition-colors btn-icon-hover"
                style={{ color: "var(--color-warning)" }}
                title="폴더 열기"
              >
                <FolderOpen className="w-4 h-4" />
              </button>
            )}
          </div>

          {/* 신뢰도 — number only */}
          <span
            className="text-xs font-semibold tabular-nums"
            style={{
              color: group.top_confidence >= 70
                ? "var(--color-success)"
                : group.top_confidence >= 40
                  ? "var(--color-warning)"
                  : "var(--color-text-muted)",
            }}
          >
            {Math.round(group.top_confidence)}%
          </span>

          {/* 파일 타입 */}
          <Badge variant={getFileTypeBadgeVariant(group.file_name)}>
            {fileExt.toUpperCase()}
          </Badge>
        </div>
      </div>

      {/* 청크 목록 */}
      <div className={isCompact ? "space-y-1" : "space-y-2"}>
        {displayChunks.map((chunk, idx) => {
          const effectiveFullText = chunk.snippet || chunk.content_preview;
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
            className={`flex rounded cursor-pointer result-item-hover ${isCompact ? "gap-1.5 p-1" : "gap-2 p-1.5"}`}
            onClick={() => onOpenFile(chunk.file_path, chunk.page_number)}
          >
            {/* Location */}
            <div className="flex-shrink-0 w-12 text-[11px]" style={{ color: "var(--color-text-muted)" }}>
              {chunk.location_hint || (chunk.page_number ? `${chunk.page_number}p` : `#${chunk.chunk_index + 1}`)}
            </div>

            {/* Preview */}
            <div className="flex-1 min-w-0">
              <p
                style={{
                  color: "var(--color-text-secondary)",
                  fontSize: "13px",
                  lineHeight: "1.7",
                  display: "-webkit-box",
                  WebkitLineClamp: isCompact ? 2 : 3,
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

            {/* Confidence — number only */}
            <span
              className="text-[11px] font-medium tabular-nums flex-shrink-0"
              style={{
                color: chunk.confidence >= 70
                  ? "var(--color-success)"
                  : chunk.confidence >= 40
                    ? "var(--color-warning)"
                    : "var(--color-text-muted)",
              }}
            >
              {Math.round(chunk.confidence)}%
            </span>
          </div>
        );
        })}

        {/* 더보기/접기 */}
        {hasMore && onToggleExpand && (
          <button
            onClick={onToggleExpand}
            className={`w-full text-xs rounded-md result-item-hover ${isCompact ? "py-1" : "py-1.5"}`}
            style={{ color: "var(--color-accent)" }}
          >
            {isExpanded ? "접기" : `+${group.chunks.length - defaultCount}개 더보기`}
          </button>
        )}
      </div>

      {/* 경로 - 컴팩트 모드에서 숨김 */}
      {!isCompact && (
        <p
          className="text-[13px] mt-2 truncate font-mono"
          style={{ color: "var(--color-text-muted)" }}
          title={group.file_path}
        >
          {formatBreadcrumb(folderPath)}
        </p>
      )}

      {/* 컨텍스트 메뉴 (공용 컴포넌트 사용) */}
      <ResultContextMenu
        filePath={group.file_path}
        folderPath={folderPath}
        onOpenFile={onOpenFile}
        onCopyPath={onCopyPath}
        onOpenFolder={onOpenFolder}
        contextMenu={contextMenu}
        closeContextMenu={closeContextMenu}
      />
    </div>
  );
});

function getStripeClass(fileName: string): string {
  const ext = fileName.split(".").pop()?.toLowerCase() || "";
  const map: Record<string, string> = {
    hwpx: "result-stripe-hwpx", hwp: "result-stripe-hwp",
    docx: "result-stripe-docx", doc: "result-stripe-docx",
    xlsx: "result-stripe-xlsx", xls: "result-stripe-xlsx",
    pdf: "result-stripe-pdf", pptx: "result-stripe-pptx",
    txt: "result-stripe-txt",
  };
  return map[ext] || "result-stripe-txt";
}

function formatBreadcrumb(path: string): string {
  let cleanPath = path.replace(/^\\\\\?\\/, "").replace(/^\/\/\?\//, "");
  const parts = cleanPath.replace(/\\/g, "/").split("/").filter(Boolean);
  if (parts.length <= 3) {
    return parts.join(" › ");
  }
  return `${parts[0]} › ... › ${parts.slice(-2).join(" › ")}`;
}
