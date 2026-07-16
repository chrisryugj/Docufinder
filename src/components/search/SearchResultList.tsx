import { useState, useCallback, useEffect, useLayoutEffect, useMemo, useRef, memo } from "react";
import { List, LayoutGrid, ClipboardCopy, FileDown, ChevronRight, FileText, FileSearch, Frown, PenLine, ArrowLeftRight, Filter, ChevronDown, FolderPlus } from "lucide-react";
import type { SearchResult, GroupedSearchResult, ViewMode, RecentSearch, ParsedQueryInfo } from "../../types/search";
import type { ViewDensity } from "../../types/settings";
import { SearchResultItem } from "./SearchResultItem";
import { GroupedSearchResultItem } from "./GroupedSearchResultItem";
import { FilenameColumnHeader } from "./FilenameColumnHeader";
import { useFilenameColumns, type FilenameSort, type ResizableCol } from "../../hooks/useFilenameColumns";
import { formatFileSize } from "../../utils/formatFileSize";
import { SearchResultSkeleton } from "./SearchResultSkeleton";
import { HighlightedFilename } from "./HighlightedFilename";
import { WelcomeHero } from "./WelcomeHero";
import { PathBreadcrumb } from "./PathBreadcrumb";
import { formatRelativeTime } from "../../utils/formatRelativeTime";
import { Badge, getFileTypeBadgeVariant } from "../ui/Badge";
import { FileIcon } from "../ui/FileIcon";
import { useContextMenu, ResultContextMenu } from "./ResultContextMenu";
import { FilenameCopiesBadge } from "./FilenameCopiesBadge";

/** 파일명 컬럼 정렬 비교자 (이름/경로/크기/수정일시/유형) */
function compareFilename(a: SearchResult, b: SearchResult, sort: FilenameSort): number {
  const mul = sort.dir === "asc" ? 1 : -1;
  const ext = (r: SearchResult) => (r.file_name.split(".").pop() || "").toLowerCase();
  switch (sort.field) {
    case "name": return mul * a.file_name.localeCompare(b.file_name, "ko");
    case "path": return mul * a.file_path.localeCompare(b.file_path, "ko");
    case "size": return mul * ((a.size ?? 0) - (b.size ?? 0));
    case "time": return mul * ((a.modified_at ?? 0) - (b.modified_at ?? 0));
    case "type": return mul * (ext(a).localeCompare(ext(b)) || a.file_name.localeCompare(b.file_name, "ko"));
    default: return 0;
  }
}

// 컬럼 자동 맞춤용 텍스트 폭 측정 (canvas 재사용)
let measureCanvas: HTMLCanvasElement | null = null;
function measureText(text: string, font: string): number {
  if (!measureCanvas) measureCanvas = document.createElement("canvas");
  const ctx = measureCanvas.getContext("2d");
  if (!ctx) return 0;
  ctx.font = font;
  return ctx.measureText(text).width;
}

interface SearchResultListProps {
  results: SearchResult[];
  /** 파일명 검색 결과 (통합 모드에서 상단 표시) */
  filenameResults?: SearchResult[];
  groupedResults?: GroupedSearchResult[];
  viewMode?: ViewMode;
  onViewModeChange?: (mode: ViewMode) => void;
  viewDensity?: ViewDensity;
  /** 라이브 입력 쿼리 — 로딩/빈결과 등 렌더 분기 판단용 */
  query: string;
  /** 결과 하이라이트용 쿼리(=searchedQuery). 결과와 함께만 갱신돼 타이핑 중 재하이라이트를
   *  막는다. 미지정 시 query 로 폴백(P2-2). */
  highlightQuery?: string;
  isLoading: boolean;
  selectedIndex?: number;
  onOpenFile: (filePath: string, page?: number | null) => void;
  onCopyPath?: (path: string) => void;
  onOpenFolder?: (path: string) => void;
  onExportCSV?: () => void;
  onCopyAll?: () => void;
  /** 결과 내 검색 키워드 (추가 하이라이트용) */
  refineKeywords?: string[];
  /** 필터 적용 후 결과 수 */
  resultCount?: number;
  /** 필터 적용 전 전체 결과 수 */
  totalResultCount?: number;
  /** 최소 신뢰도 설정값 (%) */
  minConfidence?: number;
  /** 검색 소요 시간 (ms) */
  searchTime?: number | null;
  /** 결과 표시 단위 (더 보기 개수) */
  resultsPerPage?: number;
  /** 웰컴 화면: 인덱싱된 파일 수 */
  indexedFiles?: number;
  /** 웰컴 화면: 인덱싱된 폴더 수 */
  indexedFolders?: number;
  /** 웰컴 화면: 최근 검색 */
  recentSearches?: RecentSearch[];
  /** 웰컴 화면: 최근 검색 클릭 */
  onSelectSearch?: (query: string) => void;
  /** 웰컴 화면: 최근 검색 삭제 (우클릭 메뉴) */
  onRemoveSearch?: (query: string) => void;
  /** 시맨틱 검색 활성 여부 */
  semanticEnabled?: boolean;
  /** 결과 선택 시 콜백 (미리보기 연동) */
  onSelectResult?: (index: number) => void;
  /** 유사 문서 찾기 콜백 */
  onFindSimilar?: (filePath: string) => void;
  /** 폴더 추가 콜백 */
  onAddFolder?: () => void;
  /** 파일별 카테고리 맵 */
  categories?: Record<string, string>;
  /** 검색 패러다임 (즉시/자연어) */
  paradigm?: "instant" | "natural";
  /** 자연어 검색 실행 여부 (결과 0건 vs 미실행 구분) */
  nlSubmitted?: boolean;
  /** NL 파서 결과 (자연어 모드 결과 없음 시 표시) */
  parsedQuery?: ParsedQueryInfo | null;
  /** Anything(AI) 모드 전환 콜백 */
  onSwitchToAnything?: () => void;
  /** 인덱싱 진행 중 여부 — 0건일 때 "아직 읽는 중" 안내 분기 */
  isIndexing?: boolean;
  /** 인덱싱 진행 수치 (0건 안내에 표시) */
  indexProgress?: { processed_files: number; total_files: number } | null;
  /** 현재 검색 모드 (0건 제안 칩 노출 판단) */
  searchMode?: string;
  /** 0건 제안: 자연어 필터 조건 없이 재검색 */
  onRetryWithoutFilters?: () => void;
  /** 0건 제안: 검색창 포커스 */
  onFocusSearch?: () => void;
  /** 0건 제안: 파일명 검색으로 전환 */
  onSwitchToFilenameSearch?: () => void;
  /** 한 번 클릭으로 파일 열기 (false: 두 번 클릭으로 열기, 한 번 클릭은 선택·미리보기) */
  openOnSingleClick?: boolean;
  /** 결과 카드에 저장 위치(경로) 표시 */
  showResultPath?: boolean;
  /** 결과 카드에 수정 날짜/시간을 절대값으로 상시 표시 (false: 상대시간, 절대값은 툴팁) */
  showAbsoluteTime?: boolean;
  /** 파일명 매치: 두 번 클릭 모드에서 한 번 클릭 시 인앱 미리보기 열기 */
  onPreviewFile?: (path: string) => void;
}

const DEFAULT_RESULTS_PER_PAGE = 50;

/** 래퍼 더블클릭 열기에서 제외할 대상 — 자체 클릭 동작(버튼·토글·링크)이 있는 *내부* 요소.
 *  핸들러가 걸린 요소 자신(예: role=button인 파일명 매치 행)은 제외 대상이 아니다. */
function isInteractiveTarget(e: React.MouseEvent): boolean {
  const hit = (e.target as HTMLElement).closest("button, [role='button'], a");
  return !!hit && hit !== e.currentTarget;
}

interface PendingScrollAnchor {
  itemId: string;
  offsetTop: number;
}

export const SearchResultList = memo(function SearchResultList({
  results,
  filenameResults = [],
  groupedResults = [],
  viewMode = "flat",
  onViewModeChange,
  viewDensity = "normal",
  query,
  highlightQuery = query,
  isLoading,
  selectedIndex,
  onOpenFile,
  onCopyPath,
  onOpenFolder,
  onExportCSV,
  onCopyAll,
  refineKeywords,
  resultCount,
  totalResultCount,
  minConfidence = 0,
  searchTime,
  resultsPerPage = DEFAULT_RESULTS_PER_PAGE,
  indexedFiles,
  indexedFolders,
  recentSearches,
  onSelectSearch,
  onRemoveSearch,
  semanticEnabled,
  onSelectResult,
  onFindSimilar,
  onAddFolder,
  categories,
  paradigm = "instant",
  nlSubmitted = false,
  parsedQuery,
  onSwitchToAnything,
  isIndexing = false,
  indexProgress,
  searchMode,
  onRetryWithoutFilters,
  onFocusSearch,
  onSwitchToFilenameSearch,
  openOnSingleClick = true,
  showResultPath = true,
  showAbsoluteTime = true,
  onPreviewFile,
}: SearchResultListProps) {
  const pageSize = resultsPerPage || DEFAULT_RESULTS_PER_PAGE;
  const [expandedIndex, setExpandedIndex] = useState<number | null>(null);
  const [isFilenameCollapsed, setIsFilenameCollapsed] = useState(false);
  // 그룹 뷰 펼침 상태 (file_path로 관리)
  const [expandedGroups, setExpandedGroups] = useState<Set<string>>(new Set());
  const [visibleCount, setVisibleCount] = useState(pageSize);
  const isCompact = viewDensity === "compact";
  // 파일명 검색 모드 — Everything식 컬럼(정렬 + 개별 너비 조절 + 기억)
  const isFilenameMode = searchMode === "filename";
  const { containerRef: filenameContainerRef, widths, visible, sort, gridTemplate, startResize, setColumnWidth, resetWidths, toggleSort, toggleColumn } = useFilenameColumns();
  // 컬럼 정렬 — 원본 인덱스(i)를 보존해 선택·미리보기·키보드 인덱스 정합성 유지
  const flatResults = useMemo(() => {
    const withIdx = results.map((r, i) => ({ r, i }));
    if (isFilenameMode && sort) {
      withIdx.sort((a, b) => compareFilename(a.r, b.r, sort));
    }
    return withIdx;
  }, [results, isFilenameMode, sort]);

  // 컬럼 경계 더블클릭 자동 맞춤 — 상위 결과의 해당 컬럼 최장 텍스트에 맞춰 너비 설정
  const autoFitColumn = useCallback((col: ResizableCol) => {
    const nameFont = "600 15px system-ui, sans-serif";
    const metaFont = "11px system-ui, sans-serif";
    const timeText = (r: SearchResult) => {
      if (!r.modified_at) return "";
      const ms = r.modified_at * 1000;
      return showAbsoluteTime
        ? new Date(ms).toLocaleString("ko-KR", { year: "numeric", month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit" })
        : formatRelativeTime(ms);
    };
    let max = 0;
    for (const r of results.slice(0, 300)) {
      let text = "";
      let font = metaFont;
      switch (col) {
        case "name": text = r.file_name; font = nameFont; break;
        case "path": text = r.file_path; break;
        case "size": text = formatFileSize(r.size); break;
        case "time": text = timeText(r); break;
      }
      const w = measureText(text, font);
      if (w > max) max = w;
    }
    setColumnWidth(col, max + (col === "name" ? 44 : 20));
  }, [results, showAbsoluteTime, setColumnWidth]);
  const listRef = useRef<HTMLDivElement>(null);
  const pendingScrollAnchorRef = useRef<PendingScrollAnchor | null>(null);
  const pointerSelectRef = useRef(false);
  // 마지막으로 scroll 대상이었던 파일 경로. 정렬/필터 변경으로 같은 파일이
  // 다른 index 로 재매핑되면 scrollIntoView 를 건너뛰기 위해 사용.
  const lastScrolledPathRef = useRef<string | null>(null);

  const captureScrollAnchor = useCallback((itemId: string) => {
    const listElement = listRef.current;
    const itemElement = document.getElementById(itemId);
    const scrollContainer = findScrollContainer(listElement);

    if (!listElement || !itemElement || !scrollContainer) {
      pendingScrollAnchorRef.current = null;
      return;
    }

    pendingScrollAnchorRef.current = {
      itemId,
      offsetTop: getOffsetTopWithinContainer(itemElement, scrollContainer),
    };
  }, []);

  // 그룹 펼침 토글
  const handleToggleGroupExpand = useCallback((filePath: string, itemId: string) => {
    captureScrollAnchor(itemId);
    setExpandedGroups(prev => {
      const next = new Set(prev);
      if (next.has(filePath)) {
        next.delete(filePath);
      } else {
        next.add(filePath);
      }
      return next;
    });
  }, [captureScrollAnchor]);

  // 검색 결과 변경 시 상태 초기화 (스크롤은 건드리지 않음 — 타이핑 중 포커스 이탈 방지)
  useEffect(() => {
    setExpandedIndex(null);
    setVisibleCount(pageSize);
  }, [results, pageSize]);

  useEffect(() => {
    pendingScrollAnchorRef.current = null;
  }, [results, groupedResults, viewMode]);

  // 키보드 선택이 visibleCount를 넘으면 자동 확장
  useEffect(() => {
    if (selectedIndex != null && selectedIndex >= visibleCount) {
      setVisibleCount(prev => Math.max(prev, selectedIndex + 1));
    }
  }, [selectedIndex, visibleCount]);

  // 키보드로 선택 변경 시만 스크롤 따라가기 (마우스 클릭은 pointerSelectRef로 스킵)
  // 재발 방지 1: 클릭 시 `scrollIntoView({block:"nearest"})`가 viewport 하단에 걸친 카드를
  //   아래쪽 edge로 끌어당겨 "저 밑으로 스크롤" 버그를 만든다. 키보드 내비게이션만 대상으로 제한.
  // 재발 방지 2: 정렬/필터 변경으로 같은 파일이 다른 index 로 자동 재매핑
  //   (useResultSelection) 되면 selectedIndex 가 바뀐 것처럼 보여 scrollIntoView 가 호출되고
  //   결과적으로 사용자는 "관련도 → 최신순 누르니 스크롤이 중간으로 튄다"로 체감. 선택된
  //   파일 경로가 같으면 index 가 바뀌어도 scroll 은 건드리지 않는다.
  useEffect(() => {
    if (pointerSelectRef.current) {
      pointerSelectRef.current = false;
      return;
    }
    if (selectedIndex == null || selectedIndex < 0) {
      lastScrolledPathRef.current = null;
      return;
    }
    const currentPath = results[selectedIndex]?.file_path ?? null;
    if (currentPath !== null && currentPath === lastScrolledPathRef.current) {
      return;
    }
    lastScrolledPathRef.current = currentPath;
    const flatId = `search-result-${selectedIndex}`;
    const groupedId = `grouped-search-result-${selectedIndex}`;
    const el = document.getElementById(flatId) || document.getElementById(groupedId);
    el?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }, [selectedIndex, results]);

  // 확장 토글 핸들러
  const handleToggleExpand = useCallback((index: number) => {
    captureScrollAnchor(`search-result-${index}`);
    setExpandedIndex((prev) => (prev === index ? null : index));
  }, [captureScrollAnchor]);

  // index 기반 안정 그룹 토글: 아이템에 인라인 화살표를 내리지 않아 memo 를 유지한다.
  // 스크롤 앵커/토글 로직(handleToggleGroupExpand)은 그대로라 동작은 불변.
  const handleToggleGroupByIndex = useCallback((index: number) => {
    const group = groupedResults[index];
    if (!group) return;
    handleToggleGroupExpand(group.file_path, `grouped-search-result-${index}`);
  }, [groupedResults, handleToggleGroupExpand]);

  useLayoutEffect(() => {
    const pendingAnchor = pendingScrollAnchorRef.current;
    if (!pendingAnchor) return;

    const listElement = listRef.current;
    const itemElement = document.getElementById(pendingAnchor.itemId);
    const scrollContainer = findScrollContainer(listElement);
    pendingScrollAnchorRef.current = null;

    if (!listElement || !itemElement || !scrollContainer) return;

    const nextOffsetTop = getOffsetTopWithinContainer(itemElement, scrollContainer);
    const offsetDelta = nextOffsetTop - pendingAnchor.offsetTop;

    if (Math.abs(offsetDelta) < 1) return;

    scrollContainer.scrollTop += offsetDelta;
  }, [expandedIndex, expandedGroups]);

  // 전체 결과 (파일명 + 내용)
  const hasResults = results.length > 0 || filenameResults.length > 0;

  // 검색 중 (결과 없음) — 스켈레톤 로더
  if (isLoading && !hasResults && query.trim()) {
    return <SearchResultSkeleton count={6} />;
  }

  // 결과가 있을 때
  if (hasResults) {
    return (
      <div className="space-y-3" aria-busy={isLoading} aria-live="polite">
        {/* 검색 중 인라인 인디케이터 */}
        {isLoading && (
          <div
            className="h-0.5 rounded-full overflow-hidden"
            style={{ backgroundColor: "var(--color-border)" }}
          >
            <div
              className="h-full rounded-full animate-search-bar"
              style={{ backgroundColor: "var(--color-accent)", width: "40%" }}
            />
          </div>
        )}

        <ResultsToolbar
          viewMode={viewMode}
          onViewModeChange={onViewModeChange}
          resultCount={resultCount}
          totalResultCount={totalResultCount}
          minConfidence={minConfidence}
          searchTime={searchTime}
          onCopyAll={onCopyAll}
          onExportCSV={onExportCSV}
        />

        <FilenameResultsSection
          filenameResults={filenameResults}
          contentResultCount={results.length}
          isCollapsed={isFilenameCollapsed}
          onToggleCollapse={() => setIsFilenameCollapsed(!isFilenameCollapsed)}
          isCompact={isCompact}
          query={highlightQuery}
          onOpenFile={onOpenFile}
          onCopyPath={onCopyPath}
          onOpenFolder={onOpenFolder}
          openOnSingleClick={openOnSingleClick}
          onPreviewFile={onPreviewFile}
          showPath={showResultPath}
          showAbsoluteTime={showAbsoluteTime}
        />

        {/* 결과 목록 */}
        {results.length > 0 && (
          viewMode === "grouped" && groupedResults.length > 0 ? (
            // 그룹 뷰
            <>
              <div ref={listRef} role="listbox" aria-label="검색 결과" aria-activedescendant={selectedIndex != null && selectedIndex >= 0 ? `grouped-search-result-${selectedIndex}` : undefined} className={`result-list-divided ${isCompact ? "space-y-0.5" : "space-y-1.5"}`}>
                {groupedResults.slice(0, visibleCount).map((group, index) => {
                  const flatIdx = results.findIndex(r => r.file_path === group.file_path);
                  const selectForPreview = () => { if (flatIdx >= 0) onSelectResult?.(flatIdx); };

                  // 단일 매칭: flat 뷰 UX로 렌더링 (인라인 +300자 컨텍스트 펼침 지원)
                  if (group.chunks.length === 1) {
                    const result = group.chunks[0];
                    return (
                      <div
                        key={group.file_path}
                        className={index < 10 ? "stagger-item" : ""}
                        style={index < 10 ? { animationDelay: `${index * 30}ms` } : undefined}
                        onClick={() => {
                          pointerSelectRef.current = true;
                          selectForPreview();
                        }}
                        onDoubleClick={openOnSingleClick ? undefined : (e) => {
                          if (isInteractiveTarget(e)) return;
                          onOpenFile(result.file_path, result.page_number);
                        }}
                      >
                        <SearchResultItem
                          result={result}
                          index={index}
                          isExpanded={expandedGroups.has(group.file_path)}
                          isCompact={isCompact}
                          onToggleExpand={handleToggleGroupByIndex}
                          onOpenFile={onOpenFile}
                          onCopyPath={onCopyPath}
                          onOpenFolder={onOpenFolder}
                          refineKeywords={refineKeywords}
                          query={highlightQuery}
                          onFindSimilar={onFindSimilar}
                          category={categories?.[group.file_path]}
                          openOnSingleClick={openOnSingleClick}
                          showPath={showResultPath}
                          showAbsoluteTime={showAbsoluteTime}
                        />
                      </div>
                    );
                  }

                  // 다중 매칭: 기존 GroupedSearchResultItem
                  return (
                    <div
                      key={group.file_path}
                      className={index < 10 ? "stagger-item" : ""}
                      style={index < 10 ? { animationDelay: `${index * 30}ms` } : undefined}
                      onClick={() => {
                        pointerSelectRef.current = true;
                        selectForPreview();
                      }}
                      onDoubleClick={openOnSingleClick ? undefined : (e) => {
                        if (isInteractiveTarget(e)) return;
                        onOpenFile(group.file_path);
                      }}
                    >
                      <GroupedSearchResultItem
                        domId={`grouped-search-result-${index}`}
                        group={group}
                        onOpenFile={onOpenFile}
                        onCopyPath={onCopyPath}
                        onOpenFolder={onOpenFolder}
                        isCompact={isCompact}
                        searchQuery={highlightQuery}
                        isExpanded={expandedGroups.has(group.file_path)}
                        onToggleExpand={handleToggleGroupByIndex}
                        index={index}
                        openOnSingleClick={openOnSingleClick}
                        showPath={showResultPath}
                      />
                    </div>
                  );
                })}
              </div>
              {groupedResults.length > visibleCount && (
                <ShowMoreButton
                  visibleCount={visibleCount}
                  totalCount={groupedResults.length}
                  onShowMore={() => setVisibleCount(prev => prev + pageSize)}
                />
              )}
            </>
          ) : (
            // 플랫 뷰
            <>
              <div
                ref={isFilenameMode ? filenameContainerRef : undefined}
                style={isFilenameMode ? ({
                  "--fn-name": `${widths.name}px`,
                  "--fn-path": `${widths.path}px`,
                  "--fn-size": `${widths.size}px`,
                  "--fn-time": `${widths.time}px`,
                } as React.CSSProperties) : undefined}
              >
                {isFilenameMode && (
                  <FilenameColumnHeader
                    gridTemplate={gridTemplate}
                    visible={visible}
                    sort={sort}
                    onSort={toggleSort}
                    startResize={startResize}
                    onAutoFit={autoFitColumn}
                    toggleColumn={toggleColumn}
                    resetWidths={resetWidths}
                  />
                )}
                <div ref={listRef} role="listbox" aria-label="검색 결과" aria-activedescendant={selectedIndex != null && selectedIndex >= 0 ? `search-result-${selectedIndex}` : undefined} className={`result-list-divided ${isCompact ? "space-y-0.5" : "space-y-1.5"}`}>
                {flatResults.slice(0, visibleCount).map(({ r: result, i: index }) => (
                  <div
                    key={`${result.file_path}-${result.chunk_index ?? 0}-${result.start_offset ?? index}`}
                    className={`group ${index < 10 ? "stagger-item" : ""}`}
                    style={{
                      contain: "layout style",
                      ...(index < 10 && { animationDelay: `${index * 30}ms` }),
                    }}
                    onClick={() => {
                      pointerSelectRef.current = true;
                      onSelectResult?.(index);
                    }}
                    onDoubleClick={openOnSingleClick ? undefined : (e) => {
                      if (isInteractiveTarget(e)) return;
                      onOpenFile(result.file_path, result.page_number);
                    }}
                  >
                    <SearchResultItem
                      result={result}
                      index={index}
                      isExpanded={expandedIndex === index}
                      isSelected={selectedIndex === index}
                      isCompact={isCompact}
                      onToggleExpand={handleToggleExpand}
                      onOpenFile={onOpenFile}
                      onCopyPath={onCopyPath}
                      onOpenFolder={onOpenFolder}
                      refineKeywords={refineKeywords}
                      query={highlightQuery}
                      onFindSimilar={onFindSimilar}
                      category={categories?.[result.file_path]}
                      openOnSingleClick={openOnSingleClick}
                      showPath={showResultPath}
                      showAbsoluteTime={showAbsoluteTime}
                      filenameGridTemplate={gridTemplate}
                      filenameVisible={visible}
                    />
                  </div>
                ))}
                </div>
              </div>
              {results.length > visibleCount && (
                <ShowMoreButton
                  visibleCount={visibleCount}
                  totalCount={results.length}
                  onShowMore={() => setVisibleCount(prev => prev + pageSize)}
                />
              )}
            </>
          )
        )}

        {/* Anything 진입점 배너 */}
        {onSwitchToAnything && results.length > 0 && (
          <button
            onClick={onSwitchToAnything}
            className="w-full mt-3 flex items-center gap-3 px-4 py-3 rounded-lg transition-all duration-150 hover:scale-[1.005] active:scale-[0.995] group"
            style={{
              background: "linear-gradient(135deg, var(--color-accent-ai-subtle) 0%, rgba(99,102,241,0.04) 100%)",
              border: "1px solid color-mix(in srgb, var(--color-accent-ai) 15%, transparent)",
            }}
          >
            <div
              className="w-7 h-7 rounded-lg shrink-0 flex items-center justify-center"
              style={{ background: "linear-gradient(135deg, var(--color-accent-ai) 0%, var(--color-accent-ai-hover) 100%)" }}
            >
              <svg width="13" height="13" viewBox="0 0 24 24" fill="white" stroke="none">
                <path d="M12 2l2.4 6.4L21 11l-6.6 2.4L12 21l-2.4-7.6L3 11l6.6-2.4L12 2z" />
              </svg>
            </div>
            <div className="flex-1 text-left min-w-0">
              <span className="text-xs font-medium" style={{ color: "var(--color-accent-ai)" }}>
                Anything에게 물어보기
              </span>
              <p className="text-[10px] text-[var(--color-text-muted)] truncate">
                검색 결과를 AI가 분석하여 답변합니다
              </p>
            </div>
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="var(--color-accent-ai)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="shrink-0 opacity-50 group-hover:opacity-100 transition-opacity">
              <path d="M5 12h14M12 5l7 7-7 7" />
            </svg>
          </button>
        )}
      </div>
    );
  }

  // 자연어 모드: 아직 Enter 안 눌렀을 때 (결과 없고 로딩도 아닌 상태)
  if (paradigm === "natural" && query.trim() && !isLoading && results.length === 0 && !nlSubmitted) {
    return (
      <div className="text-center py-16">
        <div
          className="w-20 h-20 mx-auto mb-5 rounded-2xl flex items-center justify-center"
          style={{
            backgroundColor: "var(--color-accent-subtle)",
            border: "2px solid var(--color-accent)",
          }}
        >
          <FileSearch
            className="w-10 h-10"
            style={{ color: "var(--color-accent)" }}
            strokeWidth={1.5}
            aria-hidden="true"
          />
        </div>
        <h3
          className="text-lg font-semibold mb-2"
          style={{ color: "var(--color-text-primary)" }}
        >
          검색 준비 완료
        </h3>
        <p className="mb-4" style={{ color: "var(--color-text-muted)" }}>
          질문을 완성한 후 <kbd
            className="inline-flex items-center px-2 py-0.5 mx-0.5 rounded text-xs font-semibold"
            style={{
              backgroundColor: "var(--color-accent)",
              color: "white",
            }}
          >Enter</kbd> 키를 누르면 검색합니다
        </p>
      </div>
    );
  }

  // 인덱싱된 문서가 0개 — "결과 없음"이 아니라 "아직 인덱스 없음"임을 명확히 안내
  // (신규 사용자가 폴더 추가 전 검색했을 때 "다른 검색어를 쓰세요"라는 오해 유발을 방지)
  if (query.trim() && !isLoading && indexedFiles === 0) {
    return (
      <div className="text-center py-16" role="status" aria-live="polite">
        <div
          className="w-20 h-20 mx-auto mb-6 rounded-2xl flex items-center justify-center"
          style={{ backgroundColor: "var(--color-bg-tertiary)" }}
        >
          <FolderPlus
            className="w-10 h-10 opacity-60"
            style={{ color: "var(--color-text-muted)" }}
            strokeWidth={1.5}
            aria-hidden="true"
          />
        </div>
        <h3 className="text-lg font-semibold mb-2" style={{ color: "var(--color-text-primary)" }}>
          아직 인덱싱된 문서가 없습니다
        </h3>
        <p className="mb-6" style={{ color: "var(--color-text-muted)" }}>
          폴더를 추가하면 검색을 시작할 수 있어요
        </p>
        {onAddFolder && (
          <button
            onClick={onAddFolder}
            className="inline-flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium transition-colors active:scale-[0.98]"
            style={{ backgroundColor: "var(--color-accent)", color: "white" }}
          >
            <FolderPlus className="w-4 h-4" aria-hidden="true" />
            폴더 추가하여 시작하기
          </button>
        )}
      </div>
    );
  }

  // 인덱싱 진행 중 0건 — "결과 없음"이 아니라 "아직 문서를 읽는 중"임을 안내
  // (인덱싱 완료 전 검색한 사용자가 검색어 탓으로 오해하는 것을 방지)
  if (query.trim() && !isLoading && isIndexing) {
    const truncatedQuery = query.length > 30 ? query.slice(0, 30) + "..." : query;
    return (
      <div className="text-center py-16" role="status" aria-live="polite">
        <div
          className="w-20 h-20 mx-auto mb-6 rounded-2xl flex items-center justify-center"
          style={{ backgroundColor: "var(--color-bg-tertiary)" }}
        >
          <FileSearch
            className="w-10 h-10 opacity-60"
            style={{ color: "var(--color-text-muted)" }}
            strokeWidth={1.5}
            aria-hidden="true"
          />
        </div>
        <h3 className="text-lg font-semibold mb-2" style={{ color: "var(--color-text-primary)" }}>
          아직 문서를 읽는 중입니다
        </h3>
        <p style={{ color: "var(--color-text-muted)" }}>
          "<span style={{ color: "var(--color-accent)" }}>{truncatedQuery}</span>"의 결과가
          인덱싱이 끝나면 나타날 수 있어요
          {indexProgress && indexProgress.total_files > 0 && (
            <span> ({indexProgress.processed_files}/{indexProgress.total_files} 파일 처리됨)</span>
          )}
        </p>
      </div>
    );
  }

  // 검색어가 있지만 결과 없음 - 맥락 있는 피드백
  if (query.trim() && !isLoading) {
    const truncatedQuery = query.length > 30 ? query.slice(0, 30) + "..." : query;

    // 자연어 모드: 파싱 결과 표시로 왜 결과가 없는지 힌트 제공
    const hasNlFilters = parsedQuery && (parsedQuery.date_filter || parsedQuery.file_type || parsedQuery.exclude_keywords.length > 0);

    return (
      <div className="text-center py-16" role="status" aria-live="polite">
        <div
          className="w-20 h-20 mx-auto mb-6 rounded-2xl flex items-center justify-center"
          style={{ backgroundColor: "var(--color-bg-tertiary)" }}
        >
          <Frown
            className="w-10 h-10 opacity-60"
            style={{ color: "var(--color-text-muted)" }}
            strokeWidth={1.5}
            aria-hidden="true"
          />
        </div>
        <h3
          className="text-lg font-semibold mb-2"
          style={{ color: "var(--color-text-primary)" }}
        >
          결과를 찾을 수 없습니다
        </h3>
        <p className="mb-6" style={{ color: "var(--color-text-muted)" }}>
          "<span style={{ color: "var(--color-accent)" }}>{truncatedQuery}</span>"에 대한 결과가 없습니다
        </p>

        {/* 자연어 모드: 파싱 결과 칩 표시 */}
        {paradigm === "natural" && parsedQuery && parsedQuery.parse_log.length > 0 && (
          <div className="mb-6">
            <p className="text-xs mb-2" style={{ color: "var(--color-text-muted)" }}>분석된 검색 조건:</p>
            <div className="flex flex-wrap justify-center gap-1.5">
              {parsedQuery.parse_log.map((log, i) => (
                <span
                  key={i}
                  className="inline-flex items-center px-2.5 py-1 rounded-full text-xs font-medium"
                  style={{ backgroundColor: "var(--color-accent-light, rgba(234,88,12,0.1))", color: "var(--color-accent)", border: "1px solid var(--color-accent-border, rgba(234,88,12,0.2))" }}
                >
                  {log}
                </span>
              ))}
            </div>
          </div>
        )}

        <div className="space-y-2 text-sm" style={{ color: "var(--color-text-muted)" }}>
          <p>다음을 시도해보세요:</p>
          <div className="flex flex-wrap justify-center gap-2 mt-3">
            {hasNlFilters && onRetryWithoutFilters && (
              <button
                onClick={onRetryWithoutFilters}
                className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-full text-xs border btn-outline-accent-hover"
              >
                <Filter className="w-3.5 h-3.5" />
                날짜/파일타입 조건 없이 검색
              </button>
            )}
            {onFocusSearch && (
              <button
                onClick={onFocusSearch}
                className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-full text-xs border btn-outline-accent-hover"
              >
                <PenLine className="w-3.5 h-3.5" />
                다른 검색어 입력
              </button>
            )}
            {paradigm === "instant" && searchMode !== "filename" && onSwitchToFilenameSearch && (
              <button
                onClick={onSwitchToFilenameSearch}
                className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-full text-xs border btn-outline-accent-hover"
              >
                <ArrowLeftRight className="w-3.5 h-3.5" />
                파일명으로 검색
              </button>
            )}
          </div>
        </div>
      </div>
    );
  }

  // 스마트 검색 모드 — 초기 안내 화면
  if (paradigm === "natural") {
    return <SmartSearchGuide />;
  }

  // 초기 상태 — 웰컴 히어로
  return (
    <WelcomeHero
      indexedFiles={indexedFiles}
      indexedFolders={indexedFolders}
      recentSearches={recentSearches}
      onSelectSearch={onSelectSearch}
      onRemoveSearch={onRemoveSearch}
      semanticEnabled={semanticEnabled}
      onAddFolder={onAddFolder}
      onOpenFile={onOpenFile}
    />
  );
});

/** 더 보기 버튼 */
function findScrollContainer(element: HTMLElement | null): HTMLElement | null {
  let current = element?.parentElement ?? null;

  while (current) {
    const { overflowY } = window.getComputedStyle(current);
    if (overflowY === "auto" || overflowY === "scroll") {
      return current;
    }
    current = current.parentElement;
  }

  return null;
}

function getOffsetTopWithinContainer(element: HTMLElement, container: HTMLElement): number {
  const elementRect = element.getBoundingClientRect();
  const containerRect = container.getBoundingClientRect();
  return elementRect.top - containerRect.top;
}

function ShowMoreButton({ visibleCount, totalCount, onShowMore }: {
  visibleCount: number;
  totalCount: number;
  onShowMore: () => void;
}) {
  const remaining = totalCount - visibleCount;
  return (
    <div className="flex justify-center pt-2">
      <button
        onClick={onShowMore}
        className="flex items-center gap-2 px-4 py-2 text-sm font-medium rounded-lg border btn-outline-accent-hover"
      >
        <ChevronDown className="w-4 h-4" />
        {remaining}개 더 보기
      </button>
    </div>
  );
}

const SMART_GUIDE_CATEGORIES: { label: string; icon: string; examples: string[] }[] = [
  { label: "파일 타입", icon: "📄", examples: ["예산 한글 문서", "계약서 PDF만", "매출 엑셀 파일"] },
  { label: "날짜 범위", icon: "📅", examples: ["최근 30일 보고서", "작년 인사발령", "3월 회의록"] },
  { label: "제외 검색", icon: "🚫", examples: ["계약서 초안 제외", "인사 관련 공지 빼고", "임시파일 아닌 것만"] },
  { label: "조건 조합", icon: "🔗", examples: ["올해 예산 엑셀 파일", "최근 7일 계약서 PDF", "작년 보고서 한글문서"] },
];

/** 스마트 검색 모드 초기 안내 화면 */
function SmartSearchGuide() {
  return (
    <div className="flex flex-col h-full px-4 sm:px-8 pt-2">
      {/* 예시 카드 그리드 */}
      <div className="grid grid-cols-2 sm:grid-cols-2 gap-2.5">
        {SMART_GUIDE_CATEGORIES.map((cat) => (
          <div
            key={cat.label}
            className="flex flex-col gap-2.5 px-4 py-3.5 rounded-xl"
            style={{
              backgroundColor: "var(--color-bg-secondary)",
              border: "1px solid var(--color-border)",
            }}
          >
            <div className="flex items-center gap-2">
              <span className="text-base">{cat.icon}</span>
              <span className="text-xs font-semibold text-[var(--color-text-secondary)]">
                {cat.label}
              </span>
            </div>
            <ul className="space-y-1.5">
              {cat.examples.map((ex) => (
                <li key={ex} className="text-[13px] leading-snug text-[var(--color-text-muted)]">
                  "{ex}"
                </li>
              ))}
            </ul>
          </div>
        ))}
      </div>

      {/* 하단 안내 */}
      <div className="mt-auto pb-4 flex items-center justify-center gap-4 text-xs text-[var(--color-text-tertiary)]">
        <span>Enter로 검색 · 키워드 + 조건을 자연스럽게 입력하세요</span>
      </div>
    </div>
  );
}

/** 결과 툴바: 뷰 모드 + 결과 수 + 복사/CSV */
function ResultsToolbar({
  viewMode,
  onViewModeChange,
  resultCount,
  totalResultCount,
  minConfidence = 0,
  searchTime,
  onCopyAll,
  onExportCSV,
}: {
  viewMode: ViewMode;
  onViewModeChange?: (mode: ViewMode) => void;
  resultCount?: number;
  totalResultCount?: number;
  minConfidence?: number;
  searchTime?: number | null;
  onCopyAll?: () => void;
  onExportCSV?: () => void;
}) {
  return (
    <div className="flex items-center gap-3 mb-2 relative z-30 isolate">
      <div className="flex items-center gap-2">
        {onViewModeChange && (
          <div className="flex items-center gap-0.5 border rounded-md p-0.5" style={{ backgroundColor: "var(--color-bg-tertiary)", borderColor: "var(--color-border)" }}>
            <button
              onClick={() => onViewModeChange("flat")}
              className="p-1.5 rounded-sm transition-colors"
              style={{
                backgroundColor: viewMode === "flat" ? "var(--color-bg-secondary)" : "transparent",
                color: viewMode === "flat" ? "var(--color-accent)" : "var(--color-text-muted)",
                boxShadow: viewMode === "flat" ? "0 1px 2px rgba(0,0,0,0.05)" : "none",
              }}
              title="목록 보기"
              aria-label="목록 보기"
              aria-pressed={viewMode === "flat"}
            >
              <List className="w-4 h-4" />
            </button>
            <button
              onClick={() => onViewModeChange("grouped")}
              className="p-1.5 rounded-sm transition-colors"
              style={{
                backgroundColor: viewMode === "grouped" ? "var(--color-bg-secondary)" : "transparent",
                color: viewMode === "grouped" ? "var(--color-accent)" : "var(--color-text-muted)",
                boxShadow: viewMode === "grouped" ? "0 1px 2px rgba(0,0,0,0.05)" : "none",
              }}
              title="파일별 그룹 보기"
              aria-label="파일별 그룹 보기"
              aria-pressed={viewMode === "grouped"}
            >
              <LayoutGrid className="w-4 h-4" />
            </button>
          </div>
        )}
        {resultCount !== undefined && resultCount > 0 && (
          <div className="flex items-center gap-0.5" role="status" aria-live="polite" aria-atomic="true">
            <Badge variant="secondary">
              {totalResultCount !== undefined && totalResultCount !== resultCount
                ? `${totalResultCount}개 중 ${resultCount}개`
                : `${resultCount}개`}
            </Badge>
            {minConfidence > 0 && (
              <Badge variant="primary">{minConfidence}%↑</Badge>
            )}
            {searchTime !== null && searchTime !== undefined && (
              <Badge variant="secondary">{searchTime}ms</Badge>
            )}
          </div>
        )}
      </div>
      <div className="flex gap-2 ml-auto">
        <button
          onClick={onCopyAll}
          className="flex items-center gap-1.5 px-2.5 py-1 text-xs rounded-md border font-medium btn-outline-accent-hover"
          title="검색 결과 클립보드 복사"
          aria-label="검색 결과 클립보드 복사"
        >
          <ClipboardCopy className="w-3.5 h-3.5" />
          복사
        </button>
        <button
          onClick={onExportCSV}
          className="flex items-center gap-1.5 px-2.5 py-1 text-xs rounded-md border font-medium btn-outline-accent-hover"
          title="CSV 파일로 내보내기"
          aria-label="CSV 파일로 내보내기"
        >
          <FileDown className="w-3.5 h-3.5" />
          CSV
        </button>
      </div>
    </div>
  );
}

/** 파일명 매치 섹션 (토글 가능) + 내용 매치 헤더 */
function FilenameResultsSection({
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
}: {
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
}) {
  if (filenameResults.length === 0) return null;

  return (
    <>
      <div className="mb-2">
        <button
          type="button"
          onClick={onToggleCollapse}
          aria-expanded={!isCollapsed}
          className="flex items-center gap-2 px-3 py-2 rounded-r-lg mb-2 w-full text-left hover-bg-subtle"
          style={{
            borderLeft: "3px solid var(--color-text-muted)",
            backgroundColor: "var(--color-bg-tertiary)",
          }}
        >
          <ChevronRight
            className={`w-3.5 h-3.5 transition-transform ${isCollapsed ? "" : "rotate-90"}`}
            style={{ color: "var(--color-text-muted)" }}
          />
          <FileText className="w-4 h-4" style={{ color: "var(--color-text-muted)" }} />
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
            {(() => {
              // 같은 file_name 기준 그룹화 — 3개 이상이면 대표 1개로 접고 복사본 뱃지 표시 (Everything 스타일)
              const GROUP_THRESHOLD = 3;
              const groupsByName = new Map<string, SearchResult[]>();
              for (const r of filenameResults) {
                const arr = groupsByName.get(r.file_name) ?? [];
                arr.push(r);
                groupsByName.set(r.file_name, arr);
              }
              const rendered = new Set<string>();
              return filenameResults
                .map((result, index) => {
                  if (rendered.has(result.file_name)) return null;
                  rendered.add(result.file_name);
                  const group = groupsByName.get(result.file_name) ?? [result];
                  if (group.length >= GROUP_THRESHOLD) {
                    // 접기: 대표(첫 번째) + 복사본 뱃지
                    return (
                      <FilenameResultItem
                        key={`filename-group-${result.file_name}-${index}`}
                        result={group[0]}
                        query={query}
                        onOpenFile={onOpenFile}
                        onCopyPath={onCopyPath}
                        onOpenFolder={onOpenFolder}
                        copies={group}
                        openOnSingleClick={openOnSingleClick}
                        onPreviewFile={onPreviewFile}
                        showPath={showPath}
                        showAbsoluteTime={showAbsoluteTime}
                      />
                    );
                  }
                  // 2개 이하: 모두 개별 노출
                  return group.map((r, gi) => (
                    <FilenameResultItem
                      key={`filename-${r.file_path}-${index}-${gi}`}
                      result={r}
                      query={query}
                      onOpenFile={onOpenFile}
                      onCopyPath={onCopyPath}
                      onOpenFolder={onOpenFolder}
                      openOnSingleClick={openOnSingleClick}
                      onPreviewFile={onPreviewFile}
                      showPath={showPath}
                      showAbsoluteTime={showAbsoluteTime}
                    />
                  ));
                })
                .filter(Boolean);
            })()}
          </div>
        )}
      </div>

      {contentResultCount > 0 && (
        <>
          <div className="my-4" style={{ borderTop: "1px solid var(--color-border)" }} />
          <div
            className="flex items-center gap-2 px-3 py-2 rounded-r-lg mb-2"
            style={{
              borderLeft: "3px solid var(--color-accent)",
              backgroundColor: "var(--color-accent-subtle)",
            }}
          >
            <FileSearch className="w-4 h-4" style={{ color: "var(--color-accent)" }} />
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
}

/** 파일명 매치 결과 아이템 (컨텍스트 메뉴 포함).
 *  copies prop이 주어지면(≥3개) 복사본 뱃지를 표시하고 대표 경로만 노출. */
function FilenameResultItem({
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
}: {
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
}) {
  const { contextMenu, handleContextMenu, closeContextMenu } = useContextMenu();
  const folderPath = result.file_path.replace(/[/\\][^/\\]+$/, "");

  return (
    <div
      className="flex items-center gap-3 px-3 py-2 rounded-lg cursor-pointer transition-colors hover:bg-[var(--color-bg-tertiary)]"
      style={{ backgroundColor: "var(--color-bg-secondary)" }}
      role="button"
      tabIndex={0}
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
        <div className="font-medium truncate" style={{ color: "var(--color-text-primary)" }}>
          <HighlightedFilename filename={result.file_name} query={query} />
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
          className="text-[10px] flex-shrink-0 tabular-nums"
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
          className="text-[10px] px-1.5 py-0.5 rounded font-medium"
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
}
