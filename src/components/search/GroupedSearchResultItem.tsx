import { memo, useMemo } from "react";
import { ChevronUp, ClipboardCopy, FolderOpen } from "lucide-react";
import type { GroupedSearchResult } from "../../types/search";
import { FileIcon } from "../ui/FileIcon";
import { Badge, getFileTypeBadgeVariant } from "../ui/Badge";
import { HighlightedText } from "./HighlightedText";
import { buildPreviewContext } from "./searchTextUtils";
import { formatPathSegments, stripHtmlTags } from "../../utils/searchTextUtils";
import { useContextMenu, ResultContextMenu } from "./ResultContextMenu";
import { MatchDensityBar } from "./MatchDensityBar";

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

  // Top-1 Progressive Disclosure: 기본 1개만 표시, 펼치면 전체
  const defaultCount = 1;
  const displayChunks = isExpanded ? group.chunks : group.chunks.slice(0, defaultCount);
  const hasMore = group.chunks.length > defaultCount;
  const isSingleMatch = group.total_matches === 1;

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


  const isStickyHeader = isExpanded && hasMore;
  const cardPaddingX = isCompact ? "0.875rem" : "1rem";
  const cardPaddingY = isCompact ? "0.625rem" : "0.75rem";

  return (
    <div
      id={domId}
      className={`result-card ${stripeClass}`}
      style={{
        padding: `${cardPaddingY} ${cardPaddingX}`,
        // sticky 헤더가 카드 경계에서 잘리지 않도록 확장 시 overflow 해제
        overflow: isStickyHeader ? "visible" : undefined,
      }}
      onContextMenu={handleContextMenu}
      data-context-menu
    >
      {/* 그룹 헤더 — 펼친 상태에선 sticky로 상단 고정 */}
      <div
        className={`flex items-center justify-between ${isCompact ? "mb-2" : "mb-3"}`}
        style={
          isStickyHeader
            ? {
                position: "sticky",
                top: 0,
                zIndex: 2,
                // 카드 패딩 바깥으로 배경을 확장해서 스크롤 시 내용이 뒤로 비치지 않도록
                marginLeft: `calc(-1 * ${cardPaddingX})`,
                marginRight: `calc(-1 * ${cardPaddingX})`,
                marginTop: `calc(-1 * ${cardPaddingY})`,
                paddingLeft: cardPaddingX,
                paddingRight: cardPaddingX,
                paddingTop: cardPaddingY,
                paddingBottom: isCompact ? "0.5rem" : "0.625rem",
                // 완전 불투명 + blur 폴백으로 뒤 내용이 절대 비치지 않게
                background: "var(--color-bg-secondary)",
                backdropFilter: "blur(8px) saturate(1.2)",
                WebkitBackdropFilter: "blur(8px) saturate(1.2)",
                borderBottom: "1px solid var(--color-border)",
                boxShadow: "0 2px 8px -4px rgba(0, 0, 0, 0.08)",
              }
            : undefined
        }
      >
        <div
          className={`flex items-center cursor-pointer flex-1 min-w-0 group/filename hover-accent-text ${isCompact ? "gap-2" : "gap-2.5"}`}
          onClick={() => onOpenFile(group.file_path)}
          title="파일 열기 (우클릭: 더 많은 옵션)"
        >
          <FileIcon fileName={group.file_name} size={isCompact ? "sm" : "md"} />
          <span className={`truncate font-semibold ${isCompact ? "text-sm" : "text-base"}`}>
            {group.file_name}
          </span>
          {!isSingleMatch && (
            <Badge variant="default">{group.total_matches}개 매칭</Badge>
          )}
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

          {/* 접기 버튼 — 펼친 상태에서만 표시 */}
          {isExpanded && hasMore && onToggleExpand && (
            <button
              onClick={(e) => {
                e.stopPropagation();
                onToggleExpand();
              }}
              className="p-1 rounded transition-colors btn-icon-hover"
              style={{ color: "var(--color-accent)" }}
              title="접기"
              aria-label="그룹 접기"
            >
              <ChevronUp className="w-4 h-4" />
            </button>
          )}
        </div>
      </div>

      {/* 매칭 밀도 히트맵 - 2개 이상일 때만 */}
      {!isSingleMatch && (
        <div className={isCompact ? "mb-1.5 px-1" : "mb-2 px-1"}>
          <MatchDensityBar
            chunks={group.chunks}
            compact={isCompact}
            onJump={() => {
              if (!isExpanded) onToggleExpand?.();
            }}
          />
        </div>
      )}

      {/* 청크 목록 */}
      <div className={isCompact ? "space-y-1" : "space-y-2"}>
        {displayChunks.map((chunk, idx) => {
          // HTML 태그 제거 (HWPX/DOCX 테이블 파싱 잔재: <td>, <tr>, colspan 등)
          const cleanSnippet = chunk.snippet ? stripHtmlTags(chunk.snippet) : undefined;
          const cleanPreview = stripHtmlTags(chunk.content_preview);
          const effectiveFullText = cleanSnippet || cleanPreview;
          const preview = buildPreviewContext({
            previewText: cleanPreview,
            fullText: effectiveFullText,
            highlightRanges: chunk.highlight_ranges,
            snippet: cleanSnippet,
            query: searchQuery,
          });
          return (
          <div
            key={`${chunk.chunk_index}-${idx}`}
            className={`flex rounded cursor-pointer result-item-hover ${isCompact ? "gap-1.5 p-1" : "gap-2 p-1.5"}`}
            onClick={() => { if (hasMore) onToggleExpand?.(); }}
            title={hasMore ? (isExpanded ? "클릭: 접기" : "클릭: 모든 매칭 펼치기") : "파일명 클릭: 외부 실행"}
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
                  fontSize: "var(--text-sm)",
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
            {isExpanded ? "접기" : `나머지 ${group.chunks.length - defaultCount}개 매칭 보기`}
          </button>
        )}
      </div>

      {/* 경로 + 액션 버튼 — SearchResultItem Row 3와 동일 */}
      {!isCompact && (
        <div className="flex items-center justify-between mt-2">
          <div
            className="flex flex-wrap items-center gap-0.5 flex-1 min-w-0"
            title={group.file_path.replace(/^\\\\\?\\/, "")}
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
                  <span className="text-xs px-0.5 py-0.5 clr-muted" style={{ opacity: 0.5 }}>
                    {seg.label}
                  </span>
                )}
                {i < arr.length - 1 && (
                  <span className="text-[11px] mx-px clr-muted" style={{ opacity: 0.3 }}>/</span>
                )}
              </div>
            ))}
          </div>
        </div>
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

