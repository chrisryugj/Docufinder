import { useState } from "react";
import type { GroupedSearchResult } from "../../types/search";
import { FileIcon } from "../ui/FileIcon";
import { Badge, getFileTypeBadgeVariant } from "../ui/Badge";
import { ConfidenceBadge } from "../ui/ConfidenceBadge";
import { HighlightedText } from "./HighlightedText";

interface GroupedSearchResultItemProps {
  group: GroupedSearchResult;
  onOpenFile: (filePath: string, page?: number | null) => void;
  onCopyPath?: (path: string) => void;
  onOpenFolder?: (path: string) => void;
}

/**
 * 파일별로 그룹핑된 검색 결과 아이템
 * - 접혀있을 때: 파일명 + 매칭 수 + 최고 신뢰도
 * - 펼쳐졌을 때: 각 청크 미리보기
 */
export function GroupedSearchResultItem({
  group,
  onOpenFile,
  onCopyPath,
  onOpenFolder,
}: GroupedSearchResultItemProps) {
  const [isExpanded, setIsExpanded] = useState(false);
  const fileExt = group.file_name.split(".").pop()?.toLowerCase() || "";
  const folderPath = group.file_path.replace(/[/\\][^/\\]+$/, "");

  // 기본 3개만 표시, 펼치면 전체
  const displayChunks = isExpanded ? group.chunks : group.chunks.slice(0, 3);
  const hasMore = group.chunks.length > 3;

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
    <div className="result-card" style={{ padding: "1rem 1.25rem" }}>
      {/* 그룹 헤더 */}
      <div className="flex items-center justify-between mb-3">
        <div
          className="flex items-center gap-2.5 cursor-pointer flex-1 min-w-0 group/filename"
          onClick={() => onOpenFile(group.file_path)}
          style={{ color: "var(--color-text-primary)" }}
          onMouseEnter={(e) => {
            e.currentTarget.style.color = "var(--color-accent)";
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.color = "var(--color-text-primary)";
          }}
        >
          <FileIcon fileName={group.file_name} size="md" />
          <span className="truncate font-semibold text-base">
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
      <div className="space-y-2">
        {displayChunks.map((chunk, idx) => (
          <div
            key={`${chunk.chunk_index}-${idx}`}
            className="flex gap-2 p-2 rounded-md cursor-pointer transition-colors"
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
            <div className="flex-shrink-0 w-16 text-xs" style={{ color: "var(--color-text-muted)" }}>
              {chunk.location_hint || (chunk.page_number ? `${chunk.page_number}p` : `#${chunk.chunk_index + 1}`)}
            </div>

            {/* 내용 미리보기 */}
            <div className="flex-1 min-w-0">
              <p
                className="text-sm truncate"
                style={{ color: "var(--color-text-secondary)" }}
              >
                <HighlightedText
                  text={chunk.content_preview}
                  ranges={chunk.highlight_ranges}
                />
              </p>
            </div>

            {/* 신뢰도 (컴팩트) */}
            <ConfidenceBadge confidence={chunk.confidence} compact showBar={false} />
          </div>
        ))}

        {/* 더보기/접기 */}
        {hasMore && (
          <button
            onClick={() => setIsExpanded(!isExpanded)}
            className="w-full py-1.5 text-xs rounded-md transition-colors"
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
            {isExpanded ? "접기" : `+${group.chunks.length - 3}개 더보기`}
          </button>
        )}
      </div>

      {/* 경로 */}
      <p
        className="text-xs mt-2 truncate font-mono"
        style={{ color: "var(--color-text-muted)" }}
        title={group.file_path}
      >
        {formatBreadcrumb(folderPath)}
      </p>
    </div>
  );
}

function formatBreadcrumb(path: string): string {
  let cleanPath = path.replace(/^\\\\\?\\/, "").replace(/^\/\/\?\//, "");
  const parts = cleanPath.replace(/\\/g, "/").split("/").filter(Boolean);
  if (parts.length <= 3) {
    return parts.join(" › ");
  }
  return `${parts[0]} › ... › ${parts.slice(-2).join(" › ")}`;
}
