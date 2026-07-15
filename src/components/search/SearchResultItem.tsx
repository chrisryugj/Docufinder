import { useCallback, useMemo, memo, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { startDrag } from "@crabnebula/tauri-plugin-drag";
import { ExternalLink, ChevronDown, ClipboardCopy, FolderOpen, Search, AlertTriangle } from "lucide-react";
import type { SearchResult } from "../../types/search";
import { HighlightedText } from "./HighlightedText";
import { buildPreviewContext, buildExpandedContext, stripHtmlTags } from "../../utils/searchTextUtils";
import { PathBreadcrumb } from "./PathBreadcrumb";
import { HighlightedFilename } from "./HighlightedFilename";
import { FileIcon } from "../ui/FileIcon";
import { Badge, getFileTypeBadgeVariant } from "../ui/Badge";
import { Tooltip } from "../ui/Tooltip";
import { formatRelativeTime } from "../../utils/formatRelativeTime";
import { useContextMenu, ResultContextMenu } from "./ResultContextMenu";
import { LineageBadge } from "./LineageBadge";

// 네이티브 드래그아웃용 프리뷰 아이콘 경로 — 앱당 1회만 백엔드에서 가져와 캐시.
// startDrag(icon) 가 동기 경로를 요구하므로 드래그 전에 미리 확보해 둔다.
let dragIconPath = "";
let dragIconRequested = false;
function ensureDragIcon() {
  if (dragIconRequested) return;
  dragIconRequested = true;
  invoke<string>("drag_preview_icon").then((p) => { dragIconPath = p; }).catch(() => {});
}

interface SearchResultItemProps {
  result: SearchResult;
  index: number;
  isExpanded: boolean;
  isSelected?: boolean;
  isCompact?: boolean;
  onToggleExpand: (index: number) => void;
  onOpenFile: (filePath: string, page?: number | null) => void;
  onCopyPath?: (path: string) => void;
  onOpenFolder?: (path: string) => void;
  refineKeywords?: string[];
  query?: string;
  onFindSimilar?: (filePath: string) => void;
  category?: string;
  /** false: 두 번 클릭으로 열기 (한 번 클릭은 카드 선택·미리보기) */
  openOnSingleClick?: boolean;
  /** 저장 위치(경로) 줄 표시 — 컴팩트 보기에선 꼬리 우선 축약 한 줄 */
  showPath?: boolean;
  /** 수정 날짜/시간을 절대값으로 상시 표시 (false: 상대시간 "3일 전", 절대값은 툴팁) */
  showAbsoluteTime?: boolean;
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
  openOnSingleClick = true,
  showPath = true,
  showAbsoluteTime = true,
}: SearchResultItemProps) {
  const fileExt = result.file_name.split(".").pop()?.toLowerCase() || "";
  const folderPath = result.file_path.replace(/[/\\][^/\\]+$/, "");

  // 네이티브 드래그아웃 — 파일을 다른 앱/폴더로 끌어다 놓기 (탐색기 드래그처럼)
  // Shift를 누른 채 드래그하면 복사 대신 이동(원본 옮김) — 탐색기 관례와 동일.
  useEffect(() => { ensureDragIcon(); }, []);
  const handleDragStart = useCallback((e: React.DragEvent) => {
    e.preventDefault(); // 브라우저 기본 드래그 대신 네이티브 드래그아웃 사용
    startDrag({ item: [result.file_path], icon: dragIconPath, mode: e.shiftKey ? "move" : "copy" }).catch(() => {});
  }, [result.file_path]);

  // 신뢰도 %는 시맨틱/하이브리드 매칭에서만 노출 (ux-audit-8)
  // — 키워드/파일명 매칭의 RRF 기반 점수는 일반 사용자에게 의미가 약해 숨긴다.
  //   (대소문자 무관 비교: serde 직렬화 호환성 — useSearch.ts keywordOnly 필터와 동일 패턴)
  const matchType = (result.match_type ?? "").toLowerCase();
  const showConfidence = matchType === "semantic" || matchType === "hybrid";

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

  // Text processing — HTML 태그 제거 + 스니펫/하이라이트 범위 계산을 메모이즈.
  // query(=searchedQuery, P2-2)가 타이핑 중 안정적이라, 부모 리렌더로 이 컴포넌트가
  // 재실행돼도 displayRanges(배열) 참조가 유지돼 memo 된 HighlightedText 의 regex
  // 재계산을 건너뛴다(onToggleExpand 등 불안정 prop 으로 셸이 재렌더돼도 비용 없음).
  const { displayText, displayRanges } = useMemo(() => {
    const cleanSnippet = result.snippet?.replace(/\[\[HL\]\]/g, '').replace(/\[\[\/HL\]\]/g, '');
    const rawFullText = cleanSnippet || result.content_preview;
    const clean = {
      snippet: result.snippet ? stripHtmlTags(result.snippet) : undefined,
      preview: stripHtmlTags(result.content_preview),
      fullText: stripHtmlTags(rawFullText),
    };
    const expandedView = isExpanded
      ? buildExpandedContext(clean.fullText, result.highlight_ranges, clean.snippet)
      : null;
    const previewView = !isExpanded
      ? buildPreviewContext({
          previewText: clean.preview,
          fullText: clean.fullText,
          highlightRanges: result.highlight_ranges,
          snippet: clean.snippet,
          query,
        })
      : null;
    return {
      displayText: isExpanded
        ? (expandedView?.text ?? clean.fullText)
        : (previewView?.text ?? clean.preview),
      displayRanges: isExpanded
        ? (expandedView?.ranges ?? result.highlight_ranges)
        : (previewView?.ranges ?? []),
    };
  }, [result, isExpanded, query]);

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

  // "파일 위치 열기" — 파일 경로를 넘기면 백엔드가 탐색기/Finder에서 해당 파일을
  // 선택(reveal)한 채로 폴더를 연다. (경로 표시줄의 폴더 클릭은 folderPath 유지)
  const handleRevealFile = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      onOpenFolder?.(result.file_path);
    },
    [result.file_path, onOpenFolder]
  );

  return (
    <div
      id={`search-result-${index}`}
      className={`search-result-item result-card ${getStripeClass(result.file_name)}`}
      style={{
        "--item-index": index,
        padding: isCompact ? "0.5rem 0.625rem" : "1rem 0.625rem",
        ...(isSelected && {
          backgroundColor: "var(--color-accent-light)",
          outline: "1.5px solid var(--color-accent)",
          outlineOffset: "-1.5px",
          borderRadius: "var(--radius-card)",
        }),
      } as React.CSSProperties}
      role="option"
      aria-selected={isSelected}
      aria-label={`${result.file_name} 검색 결과`}
      tabIndex={isSelected ? 0 : -1}
      onContextMenu={handleContextMenu}
      onKeyDown={(e) => {
        if (e.key === "ContextMenu" || (e.shiftKey && e.key === "F10")) {
          e.preventDefault();
          const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
          handleContextMenu({ clientX: rect.left + 40, clientY: rect.top + rect.height / 2, preventDefault: () => {} } as React.MouseEvent);
        }
      }}
      data-context-menu
    >
      {/* Row 1: Filename + confidence + time */}
      <div className="flex items-center justify-between mb-1.5">
        <div
          className="flex items-center cursor-pointer flex-1 min-w-0 group/filename hover-accent-text gap-2"
          draggable
          onDragStart={handleDragStart}
          onClick={(e) => {
            // 두 번 클릭 모드: 한 번 클릭은 카드 래퍼로 버블 → 선택·미리보기
            if (!openOnSingleClick) return;
            // 카드 래퍼(미리보기 선택)로 버블되면 외부 열기와 미리보기가 동시에 일어난다
            e.stopPropagation();
            onOpenFile(result.file_path, result.page_number);
          }}
          onDoubleClick={(e) => {
            if (openOnSingleClick) return;
            e.stopPropagation();
            onOpenFile(result.file_path, result.page_number);
          }}
          onMouseDown={(e) => {
            // 더블클릭 시 파일명 텍스트 선택 방지 (드래그아웃·단일 클릭은 유지)
            if (e.detail >= 2) e.preventDefault();
          }}
          title={
            openOnSingleClick
              ? (result.page_number ? `${result.page_number}페이지로 열기 · 끌어서 다른 앱으로` : "파일 열기 · 끌어서 다른 앱으로")
              : (result.page_number ? `두 번 클릭: ${result.page_number}페이지로 열기 · 한 번 클릭: 미리보기` : "두 번 클릭: 열기 · 한 번 클릭: 미리보기")
          }
        >
          <FileIcon fileName={result.file_name} size="sm" />
          <span
            className="truncate ts-lg"
            style={{ fontWeight: 600, letterSpacing: "-0.015em" }}
          >
            <HighlightedFilename filename={result.file_name} query={query} />
          </span>
          <ExternalLink className="w-3.5 h-3.5 flex-shrink-0 opacity-30 group-hover/filename:opacity-60 transition-opacity" />
        </div>

        {/* Right side: confidence % + time + file type */}
        <div className="flex items-center gap-2 ml-2 flex-shrink-0">
          {/* 관련도 — semantic/hybrid 매칭 한정. 숫자 % 대신 점 신호(●●○)로 불안 제거 */}
          {showConfidence && (() => {
            const level = result.confidence >= 70 ? 3 : result.confidence >= 40 ? 2 : 1;
            const levelLabel = level === 3 ? "높음" : level === 2 ? "보통" : "낮음";
            const dotColor = level === 3
              ? "var(--color-success)"
              : level === 2
                ? "var(--color-accent-warm)"
                : "var(--color-text-muted)";
            return (
              <span
                className="inline-flex items-center gap-0.5 leading-none"
                aria-label={`관련도 ${levelLabel} (${Math.round(result.confidence)}%)`}
                title={`관련도: ${levelLabel} (${Math.round(result.confidence)}%)`}
              >
                {[0, 1, 2].map((i) => (
                  <span
                    key={i}
                    className="w-1.5 h-1.5 rounded-full"
                    style={{ backgroundColor: i < level ? dotColor : "var(--color-border)" }}
                  />
                ))}
              </span>
            );
          })()}

          {/* 수정 시각 — 기본은 절대 날짜/시간 상시, 툴팁에 상대시간. 토글 off면 반대 */}
          {relativeTime && (
            <Tooltip content={showAbsoluteTime ? relativeTime : absoluteDate} position="bottom" delay={200}>
              <span
                className="text-[11px] tabular-nums leading-none"
                style={{ color: "var(--color-text-muted)" }}
              >
                {showAbsoluteTime ? absoluteDate : relativeTime}
              </span>
            </Tooltip>
          )}

          {/* 복사 시 글자 깨짐 경고 — CID/PUA 매핑 손상 문서 */}
          {result.garbled && (
            <Tooltip content="복사 시 글자가 깨질 수 있는 문서" position="bottom" delay={200}>
              <Badge variant="warning" aria-label="복사 시 글자가 깨질 수 있는 문서">
                <AlertTriangle className="w-3 h-3" />
              </Badge>
            </Tooltip>
          )}

          {/* File type badge */}
          <Badge variant={getFileTypeBadgeVariant(result.file_name)} aria-label={`파일 형식: ${fileExt.toUpperCase()}`}>
            {fileExt.toUpperCase()}
          </Badge>
          {/* 카테고리 — 기본 숨김, 펼친 카드에서만 노출 (ux-audit-8 메타 과밀 정리) */}
          {isExpanded && category && category !== "기타" && (
            <Badge variant="secondary">{category}</Badge>
          )}
          {/* Document Lineage: 같은 문서의 다른 버전이 있을 때만 표시 */}
          {result.lineage_id && result.version_count && result.version_count >= 2 && (
            <LineageBadge
              lineageId={result.lineage_id}
              versionCount={result.version_count}
              versionLabel={result.version_label}
              currentFilePath={result.file_path}
              onOpenFile={(path) => onOpenFile(path)}
            />
          )}
        </div>
      </div>

      {/* Row 2: Content preview (pl-6 = FileIcon 16px + gap 8px 정렬) */}
      <div
        className="cursor-pointer rounded flex gap-1.5 -mx-1.5 px-1.5 py-1 pl-6"
        onClick={() => onToggleExpand(index)}
        aria-expanded={isExpanded}
        role="button"
        aria-label={isExpanded ? "미리보기 접기" : "미리보기 펼치기"}
      >
        <ChevronDown
          className={`w-3 h-3 flex-shrink-0 mt-1 transition-transform duration-200 ${isExpanded ? "rotate-180" : ""}`}
          style={{ color: "var(--color-text-muted)" }}
        />
        <div className="flex-1 min-w-0">
          {/* 축소 모드: line-clamp으로 2줄 미리보기 */}
          {!isExpanded && (
            <p
              style={{
                color: "var(--color-text-secondary)",
                fontSize: "var(--text-sm)",
                lineHeight: "1.7",
                letterSpacing: "0.3px",
                display: "-webkit-box",
                WebkitLineClamp: 2,
                WebkitBoxOrient: "vertical" as const,
                overflow: "hidden",
              }}
            >
              <HighlightedText
                text={displayText}
                ranges={displayRanges}
                refineKeywords={refineKeywords}
                searchQuery={query}
                formatMode="preview"
              />
            </p>
          )}
          {/* 확장 모드: grid 트랜지션으로 부드러운 펼침 */}
          <div
            className="grid transition-[grid-template-rows] duration-200 ease-out"
            style={{ gridTemplateRows: isExpanded ? "1fr" : "0fr" }}
          >
            <div className="overflow-hidden">
              {isExpanded && (
                <p
                  style={{
                    color: "var(--color-text-secondary)",
                    fontSize: "var(--text-sm)",
                    lineHeight: "1.7",
                    letterSpacing: "0.3px",
                  }}
                >
                  <HighlightedText
                    text={displayText}
                    ranges={displayRanges}
                    refineKeywords={refineKeywords}
                    searchQuery={query}
                    formatMode="full"
                  />
                </p>
              )}
            </div>
          </div>
        </div>
      </div>

      {/* Row 3: Path + action buttons (pl-6 = Row 1 FileIcon 정렬).
          컴팩트 보기도 저장 위치는 표시 — 꼬리 우선 축약 한 줄 (사용자 피드백:
          "어디 저장돼 있는지 보려고 한 번 더 눌러야" 해소). showPath=false면 줄 전체 숨김. */}
      {showPath && (
        <div className={`flex items-center justify-between pl-6 ${isCompact ? "mt-1" : "mt-1.5"}`}>
          {/* Breadcrumb path (컴팩트: 말단 폴더가 보이는 한 줄, 클릭 시 파일 위치 열기) */}
          <PathBreadcrumb
            filePath={result.file_path}
            folderPath={folderPath}
            onOpenFolder={onOpenFolder}
            compact={isCompact}
          />

          {/* Action buttons — muted 단색, hover 시 착색 (btn-icon-hover, ux-audit-8) */}
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
              <ClipboardCopy className="w-3.5 h-3.5" />
            </button>
            {onOpenFolder && (
              <button
                onClick={handleRevealFile}
                className="p-1 rounded btn-icon-hover"
                title="파일 위치 열기 (탐색기에서 선택)"
                aria-label="파일 위치 열기"
              >
                <FolderOpen className="w-3.5 h-3.5" />
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
                <Search className="w-3.5 h-3.5" />
              </button>
            )}
          </div>
        </div>
      )}

      <ResultContextMenu
        filePath={result.file_path}
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
