import { memo, useMemo } from "react";
import { ChevronRight, FileText, FileSearch } from "lucide-react";
import type { SearchResult } from "../../../types/search";
import { HighlightedFilename } from "../HighlightedFilename";
import { PathBreadcrumb } from "../PathBreadcrumb";
import { formatRelativeTime } from "../../../utils/formatRelativeTime";
import { Badge, getFileTypeBadgeVariant } from "../../ui/Badge";
import { FileIcon } from "../../ui/FileIcon";
import { useContextMenu, ResultContextMenu } from "../ResultContextMenu";
import { FilenameCopiesBadge } from "../FilenameCopiesBadge";
import { isInteractiveTarget } from "./listUtils";

// 같은 file_name 이 이만큼 이상이면 대표 1개로 접고 복사본 뱃지 표시 (Everything 스타일)
const GROUP_THRESHOLD = 3;

interface FilenameRow {
  key: string;
  result: SearchResult;
  copies?: SearchResult[];
}

/** 이름이 같은 파일을 묶어 행 목록으로. 첫 등장 순서를 유지하고, 3개 이상이면 대표 1행 + 복사본 목록 */
function buildFilenameRows(results: SearchResult[]): FilenameRow[] {
  const groupsByName = new Map<string, SearchResult[]>();
  for (const r of results) {
    const arr = groupsByName.get(r.file_name);
    if (arr) arr.push(r);
    else groupsByName.set(r.file_name, [r]);
  }
  const rows: FilenameRow[] = [];
  for (const [name, group] of groupsByName) {
    if (group.length >= GROUP_THRESHOLD) {
      rows.push({ key: `filename-group-${name}`, result: group[0], copies: group });
    } else {
      for (const r of group) rows.push({ key: `filename-${r.file_path}`, result: r });
    }
  }
  return rows;
}

interface FilenameResultsSectionProps {
  filenameResults: SearchResult[];
  contentResultCount: number;
  isCollapsed: boolean;
  onToggleCollapse: () => void;
  isCompact: boolean;
  query: string;
  onOpenFile: (filePath: string, page?: number | null) => void;
  onCopyPath?: (path: string) => void;
  onOpenFolder?: (path: string) => void;
  openOnSingleClick?: boolean;
  onPreviewFile?: (path: string) => void;
  showPath?: boolean;
  showAbsoluteTime?: boolean;
}

/** 파일명 매치 섹션 (토글 가능) + 내용 매치 헤더 */
export const FilenameResultsSection = memo(function FilenameResultsSection({
  filenameResults,
  contentResultCount,
  isCollapsed,
  onToggleCollapse,
  isCompact,
  query,
  onOpenFile,
  onCopyPath,
  onOpenFolder,
  openOnSingleClick = true,
  onPreviewFile,
  showPath = true,
  showAbsoluteTime = true,
}: FilenameResultsSectionProps) {
  const rows = useMemo(() => buildFilenameRows(filenameResults), [filenameResults]);
  if (filenameResults.length === 0) return null;

  return (
    <>
      <div className="mb-2">
        <button
          type="button"
          onClick={onToggleCollapse}
          aria-expanded={!isCollapsed}
          className="flex items-center gap-2 px-3 py-2 rounded-lg mb-2 w-full text-left hover-bg-subtle"
          style={{ backgroundColor: "var(--color-bg-tertiary)" }}
        >
          <ChevronRight
            className={`w-3.5 h-3.5 transition-transform ${isCollapsed ? "" : "rotate-90"}`}
            style={{ color: "var(--color-text-muted)" }}
            aria-hidden="true"
          />
          <FileText className="w-4 h-4" style={{ color: "var(--color-text-muted)" }} aria-hidden="true" />
          <span className="text-sm" style={{ color: "var(--color-text-secondary)" }}>
            파일명 매치
          </span>
          <span
            className="text-xs px-1.5 py-0.5 rounded-full"
            style={{
              border: "1px solid var(--color-border-hover)",
              color: "var(--color-text-muted)",
            }}
          >
            {filenameResults.length}
          </span>
          {isCollapsed && (
            <span className="text-xs ml-auto" style={{ color: "var(--color-text-muted)" }}>
              클릭하여 펼치기
            </span>
          )}
        </button>
        {!isCollapsed && (
          <div className={isCompact ? "space-y-1" : "space-y-2"}>
            {rows.map((row) => (
              <FilenameResultItem
                key={row.key}
                result={row.result}
                copies={row.copies}
                query={query}
                onOpenFile={onOpenFile}
                onCopyPath={onCopyPath}
                onOpenFolder={onOpenFolder}
                openOnSingleClick={openOnSingleClick}
                onPreviewFile={onPreviewFile}
                showPath={showPath}
                showAbsoluteTime={showAbsoluteTime}
              />
            ))}
          </div>
        )}
      </div>

      {contentResultCount > 0 && (
        <>
          <div className="my-4" style={{ borderTop: "1px solid var(--color-border)" }} />
          <div
            className="flex items-center gap-2 px-3 py-2 rounded-lg mb-2"
            style={{ backgroundColor: "var(--color-accent-subtle)" }}
          >
            <FileSearch className="w-4 h-4" style={{ color: "var(--color-accent)" }} aria-hidden="true" />
            <span className="text-sm font-medium" style={{ color: "var(--color-text-primary)" }}>
              내용 매치
            </span>
            <span
              className="text-xs px-1.5 py-0.5 rounded-full font-medium"
              style={{
                backgroundColor: "var(--color-accent-subtle)",
                color: "var(--color-accent)",
              }}
            >
              {contentResultCount}
            </span>
          </div>
        </>
      )}
    </>
  );
});

interface FilenameResultItemProps {
  result: SearchResult;
  query: string;
  onOpenFile: (filePath: string, page?: number | null) => void;
  onCopyPath?: (path: string) => void;
  onOpenFolder?: (path: string) => void;
  copies?: SearchResult[];
  openOnSingleClick?: boolean;
  onPreviewFile?: (path: string) => void;
  showPath?: boolean;
  showAbsoluteTime?: boolean;
}

/** 파일명 매치 결과 아이템 (컨텍스트 메뉴 포함).
 *  copies prop이 주어지면(≥3개) 복사본 뱃지를 표시하고 대표 경로만 노출. */
const FilenameResultItem = memo(function FilenameResultItem({
  result,
  query,
  onOpenFile,
  onCopyPath,
  onOpenFolder,
  copies,
  openOnSingleClick = true,
  onPreviewFile,
  showPath = true,
  showAbsoluteTime = true,
}: FilenameResultItemProps) {
  const { contextMenu, handleContextMenu, closeContextMenu } = useContextMenu();
  const folderPath = result.file_path.replace(/[/\\][^/\\]+$/, "");

  return (
    <div
      className="flex items-center gap-3 px-3 py-2 rounded-lg cursor-pointer transition-colors hover:bg-[var(--color-bg-tertiary)]"
      style={{ backgroundColor: "var(--color-bg-secondary)" }}
      role="button"
      tabIndex={0}
      aria-label={`${result.file_name} 열기`}
      onClick={() => {
        // 두 번 클릭 모드: 한 번 클릭은 인앱 미리보기 (훑어보다 파일이 연달아 열리는 사고 방지)
        if (openOnSingleClick) onOpenFile(result.file_path);
        else onPreviewFile?.(result.file_path);
      }}
      onDoubleClick={openOnSingleClick ? undefined : (e) => {
        // 복사본 배지 등 내부 버튼 더블클릭이 행 열기로 오발되지 않게 (래퍼와 동일 규칙)
        if (isInteractiveTarget(e)) return;
        onOpenFile(result.file_path);
      }}
      onMouseDown={(e) => {
        // 더블클릭 시 텍스트 선택 방지
        if (e.detail >= 2) e.preventDefault();
      }}
      title={openOnSingleClick ? undefined : "두 번 클릭: 열기 · 한 번 클릭: 미리보기"}
      onKeyDown={(e) => {
        // 내부 버튼(경로 세그먼트 등)에 포커스가 있으면 그 컨트롤의 Enter를 가로채지 않는다
        if (e.target !== e.currentTarget) return;
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onOpenFile(result.file_path);
        }
      }}
      onContextMenu={handleContextMenu}
      data-context-menu
    >
      <FileIcon fileName={result.file_name} size="sm" />
      <div className="flex-1 min-w-0">
        <div className="font-medium min-w-0 overflow-hidden" style={{ color: "var(--color-text-primary)" }}>
          <HighlightedFilename filename={result.file_name} query={query} middle />
        </div>
        {/* 내용 매치와 동일한 세그먼트 브레드크럼 — 중간 경로는 해당 폴더 열기,
            말단(파일이 든) 폴더는 탐색기에서 파일 선택(reveal) */}
        {showPath && (
          <PathBreadcrumb
            filePath={result.file_path}
            folderPath={folderPath}
            onOpenFolder={onOpenFolder}
            revealLast
          />
        )}
      </div>
      {result.modified_at && (
        <span
          className="text-2xs flex-shrink-0 tabular-nums"
          style={{ color: "var(--color-text-muted)" }}
          title={
            showAbsoluteTime
              ? formatRelativeTime(result.modified_at * 1000)
              : new Date(result.modified_at * 1000).toLocaleString("ko-KR")
          }
        >
          {showAbsoluteTime
            ? new Date(result.modified_at * 1000).toLocaleString("ko-KR", {
                year: "numeric", month: "2-digit", day: "2-digit",
                hour: "2-digit", minute: "2-digit",
              })
            : formatRelativeTime(result.modified_at * 1000)}
        </span>
      )}
      {result.has_hwp_pair && (
        <span
          className="text-2xs px-1.5 py-0.5 rounded font-medium"
          style={{ backgroundColor: "var(--color-warning-bg)", color: "var(--color-warning)" }}
          title="같은 위치에 원본 HWP 파일이 있습니다"
        >
          HWP
        </span>
      )}
      <Badge variant={getFileTypeBadgeVariant(result.file_name)}>
        {(result.file_name.split('.').pop() || '').toUpperCase()}
      </Badge>
      {copies && copies.length >= 2 && (
        <FilenameCopiesBadge
          copies={copies}
          currentFilePath={result.file_path}
          onOpenFile={onOpenFile}
        />
      )}
      <ResultContextMenu
        filePath={result.file_path}
        onOpenFile={onOpenFile}
        onCopyPath={onCopyPath}
        onOpenFolder={onOpenFolder}
        contextMenu={contextMenu}
        closeContextMenu={closeContextMenu}
      />
    </div>
  );
});
