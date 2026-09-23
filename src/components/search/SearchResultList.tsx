import { useState, useCallback, useEffect, useImperativeHandle, useLayoutEffect, useMemo, useRef, memo } from "react";
import { ChevronDown, Filter } from "lucide-react";
import type { SearchResult, GroupedSearchResult, ViewMode, RecentSearch, ParsedQueryInfo } from "../../types/search";
import type { ViewDensity } from "../../types/settings";
import { SearchResultItem } from "./SearchResultItem";
import { GroupedSearchResultItem } from "./GroupedSearchResultItem";
import { FilenameColumnHeader } from "./FilenameColumnHeader";
import { useFilenameColumns, type ResizableCol } from "../../hooks/useFilenameColumns";
import { formatFileSize } from "../../utils/formatFileSize";
import { SearchResultSkeleton } from "./SearchResultSkeleton";
import { WelcomeHero } from "./WelcomeHero";
import { formatRelativeTime } from "../../utils/formatRelativeTime";
import { ResultsToolbar } from "./results/ResultsToolbar";
import { FilenameResultsSection } from "./results/FilenameMatches";
import { NlReadyState, IndexingState, NoIndexState, FilteredOutState, NoResultsState, SmartSearchGuide } from "./results/EmptyStates";
import { compareFilename, measureText, isInteractiveTarget, findScrollContainer, getOffsetTopWithinContainer } from "./results/listUtils";

/** 키보드 ↑↓ 가 화면에 보이는 순서를 따르도록 목록이 부모에 내주는 이동 함수 */
export interface SearchResultListNav {
  /** 선택을 화면 순서로 한 칸 옮긴 결과의 flat index. 맨 위에서 위로 가면 -1 */
  step: (delta: 1 | -1) => number;
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
  /** 웰컴 화면: 최근 검색 클릭 (스마트 검색 예시 클릭도 같은 동작) */
  onSelectSearch?: (query: string) => void;
  /** 웰컴 화면: 최근 검색 삭제 (우클릭 메뉴) */
  onRemoveSearch?: (query: string) => void;
  /** 시맨틱 검색 활성 여부 */
  semanticEnabled?: boolean;
  /** 결과 선택 시 콜백 (미리보기 연동) */
  onSelectResult?: (index: number) => void;
  /** 유사 문서 찾기 콜백 */
  onFindSimilar?: (filePath: string) => void;
  onOcrReindex?: (filePath: string) => void;
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
  /** 인덱싱 진행 수치 (0건 안내·첫 화면에 표시). 결과가 보일 땐 부모가 null 로 넘겨 재렌더를 막는다 */
  indexProgress?: { processed_files: number; total_files: number } | null;
  /** 현재 검색 모드 (0건 제안 칩 노출 판단) */
  searchMode?: string;
  /** 0건 제안: 자연어 필터 조건 없이 재검색 */
  onRetryWithoutFilters?: () => void;
  /** 0건 제안: 검색창 포커스 */
  onFocusSearch?: () => void;
  /** 0건 제안: 파일명 검색으로 전환 */
  onSwitchToFilenameSearch?: () => void;
  /** 파일 형식·기간·결과 내 검색 필터 풀기 (걸린 필터가 없으면 넘기지 않는다) */
  onClearResultFilters?: () => void;
  /** 한 번 클릭으로 파일 열기 (false: 두 번 클릭으로 열기, 한 번 클릭은 선택·미리보기) */
  openOnSingleClick?: boolean;
  /** 결과 카드에 저장 위치(경로) 표시 */
  showResultPath?: boolean;
  /** 결과 카드에 수정 날짜/시간을 절대값으로 상시 표시 (false: 상대시간, 절대값은 툴팁) */
  showAbsoluteTime?: boolean;
  /** 파일명 매치: 두 번 클릭 모드에서 한 번 클릭 시 인앱 미리보기 열기 */
  onPreviewFile?: (path: string) => void;
  /** 키보드 이동 함수를 받을 ref */
  navRef?: React.Ref<SearchResultListNav>;
}

const DEFAULT_RESULTS_PER_PAGE = 50;

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
  selectedIndex = -1,
  onOpenFile,
  onCopyPath,
  onOpenFolder,
  onExportCSV,
  onCopyAll,
  refineKeywords,
  resultCount,
  totalResultCount = 0,
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
  onOcrReindex,
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
  onClearResultFilters,
  openOnSingleClick = true,
  showResultPath = true,
  showAbsoluteTime = true,
  onPreviewFile,
  navRef,
}: SearchResultListProps) {
  const pageSize = resultsPerPage || DEFAULT_RESULTS_PER_PAGE;
  const [expandedIndex, setExpandedIndex] = useState<number | null>(null);
  const [isFilenameCollapsed, setIsFilenameCollapsed] = useState(false);
  const toggleFilenameCollapsed = useCallback(() => setIsFilenameCollapsed((c) => !c), []);
  // 그룹 뷰 펼침 상태 (file_path로 관리)
  const [expandedGroups, setExpandedGroups] = useState<Set<string>>(new Set());
  const [visibleCount, setVisibleCount] = useState(pageSize);
  const isCompact = viewDensity === "compact";
  const isGroupedView = viewMode === "grouped" && groupedResults.length > 0;
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

  // 파일 경로 → 그 파일의 첫(대표) 결과 flat index. 그룹 카드 클릭·키보드 이동이 이 결과를 고른다
  const firstIndexByPath = useMemo(() => {
    const m = new Map<string, number>();
    results.forEach((r, i) => {
      if (!m.has(r.file_path)) m.set(r.file_path, i);
    });
    return m;
  }, [results]);

  // 화면에 보이는 순서의 flat index 목록 — 그룹 보기는 파일별 대표 결과, 파일명 컬럼은 정렬 순서.
  // 키보드 ↑↓ 가 결과 배열 순서를 따르면 그룹 보기에선 같은 파일 안을 맴돌거나 위아래로 튀었다
  const navOrder = useMemo(
    () => (isGroupedView
      ? groupedResults.map((g) => firstIndexByPath.get(g.file_path) ?? -1)
      : flatResults.map((x) => x.i)),
    [isGroupedView, groupedResults, firstIndexByPath, flatResults]
  );
  const selectedResult = selectedIndex >= 0 ? results[selectedIndex] : undefined;
  // 선택 결과의 화면 위치 (그룹 보기는 그 파일의 그룹 위치)
  const selectedPos = useMemo(() => {
    if (!selectedResult) return -1;
    if (isGroupedView) return groupedResults.findIndex((g) => g.file_path === selectedResult.file_path);
    return navOrder.indexOf(selectedIndex);
  }, [selectedResult, isGroupedView, groupedResults, navOrder, selectedIndex]);
  const selectedDomId = selectedPos < 0
    ? undefined
    : isGroupedView ? `grouped-search-result-${selectedPos}` : `search-result-${selectedIndex}`;

  // 키보드로 옮긴 선택인지 — 이때만 화면을 따라 스크롤한다
  const keyboardNavRef = useRef(false);
  useImperativeHandle(navRef, () => ({
    step: (delta) => {
      let next = -1;
      if (navOrder.length > 0) {
        if (delta < 0) next = selectedPos <= 0 ? -1 : navOrder[selectedPos - 1];
        else next = selectedPos < 0 ? navOrder[0] : navOrder[Math.min(selectedPos + 1, navOrder.length - 1)];
      }
      keyboardNavRef.current = next >= 0 && next !== selectedIndex;
      return next;
    },
  }), [navOrder, selectedPos, selectedIndex]);

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
    if (selectedPos >= visibleCount) {
      setVisibleCount(prev => Math.max(prev, selectedPos + 1));
    }
  }, [selectedPos, visibleCount]);

  // 키보드로 옮긴 선택만 화면을 따라 스크롤한다.
  // 재발 방지 1: 클릭 시 `scrollIntoView({block:"nearest"})`가 viewport 하단에 걸친 카드를
  //   아래쪽 edge로 끌어당겨 "저 밑으로 스크롤" 버그를 만든다.
  // 재발 방지 2: 정렬/필터 변경으로 같은 파일이 다른 index 로 자동 재매핑(useResultSelection)되면
  //   "관련도 → 최신순 누르니 스크롤이 중간으로 튄다"가 된다. 둘 다 step() 을 거치지 않아 제외된다.
  // 더 보기 밖의 결과는 확장 커밋 뒤에야 DOM 에 생기므로 찾을 때까지 플래그를 유지한다.
  useEffect(() => {
    if (!keyboardNavRef.current || !selectedDomId) return;
    const el = document.getElementById(selectedDomId);
    if (!el) return;
    keyboardNavRef.current = false;
    el.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }, [selectedDomId, visibleCount]);

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
  const hasQuery = query.trim().length > 0;

  // 검색 중 (결과 없음) — 스켈레톤 로더
  if (isLoading && !hasResults && hasQuery) {
    return <SearchResultSkeleton count={6} />;
  }

  // 결과가 있을 때
  if (hasResults) {
    return (
      <div className="space-y-3" aria-busy={isLoading}>
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

        {/* 키보드로 고른 결과를 화면 낭독기에 알린다 (포커스는 검색창에 남아 있어 option 이 읽히지 않는다) */}
        <div className="sr-only" aria-live="polite" aria-atomic="true">
          {selectedResult && selectedPos >= 0 ? `${selectedResult.file_name}, ${navOrder.length}개 중 ${selectedPos + 1}번째` : ""}
        </div>

        <ResultsToolbar
          viewMode={viewMode}
          onViewModeChange={onViewModeChange}
          resultCount={resultCount}
          totalResultCount={totalResultCount}
          minConfidence={minConfidence}
          searchTime={searchTime}
          onCopyAll={onCopyAll}
          onExportCSV={onExportCSV}
          onClearFilters={onClearResultFilters}
        />

        <FilenameResultsSection
          filenameResults={filenameResults}
          contentResultCount={results.length}
          isCollapsed={isFilenameCollapsed}
          onToggleCollapse={toggleFilenameCollapsed}
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

        {/* 파일명 결과만 보이고 내용 결과는 필터에 전부 가려진 경우 */}
        {results.length === 0 && totalResultCount > 0 && (
          <div
            className="flex items-center gap-2 px-3 py-2 rounded-lg text-sm"
            style={{ backgroundColor: "var(--color-bg-tertiary)", color: "var(--color-text-muted)" }}
            role="status"
          >
            <Filter className="w-3.5 h-3.5 shrink-0" aria-hidden="true" />
            <span className="flex-1">내용 결과 {totalResultCount.toLocaleString()}개가 필터에 가려져 있어요</span>
            {onClearResultFilters && (
              <button
                type="button"
                onClick={onClearResultFilters}
                className="text-xs font-medium hover:underline underline-offset-2"
                style={{ color: "var(--color-accent)" }}
              >
                필터 풀기
              </button>
            )}
          </div>
        )}

        {/* 결과 목록 */}
        {results.length > 0 && (
          isGroupedView ? (
            // 그룹 뷰
            <>
              <div ref={listRef} role="listbox" aria-label="검색 결과" aria-activedescendant={selectedDomId} className={`result-list-divided ${isCompact ? "space-y-0.5" : "space-y-1.5"}`}>
                {groupedResults.slice(0, visibleCount).map((group, index) => {
                  const flatIdx = firstIndexByPath.get(group.file_path) ?? -1;
                  const selectForPreview = () => { if (flatIdx >= 0) onSelectResult?.(flatIdx); };
                  const domId = `grouped-search-result-${index}`;
                  const isSelected = index === selectedPos;

                  // 단일 매칭: flat 뷰 UX로 렌더링 (인라인 +300자 컨텍스트 펼침 지원)
                  if (group.chunks.length === 1) {
                    const result = group.chunks[0];
                    return (
                      <div
                        key={group.file_path}
                        className={index < 10 ? "stagger-item" : ""}
                        style={index < 10 ? { animationDelay: `${index * 30}ms` } : undefined}
                        onClick={selectForPreview}
                        onDoubleClick={openOnSingleClick ? undefined : (e) => {
                          if (isInteractiveTarget(e)) return;
                          onOpenFile(result.file_path, result.page_number);
                        }}
                      >
                        <SearchResultItem
                          result={result}
                          index={index}
                          domId={domId}
                          isExpanded={expandedGroups.has(group.file_path)}
                          isSelected={isSelected}
                          isCompact={isCompact}
                          onToggleExpand={handleToggleGroupByIndex}
                          onOpenFile={onOpenFile}
                          onCopyPath={onCopyPath}
                          onOpenFolder={onOpenFolder}
                          refineKeywords={refineKeywords}
                          query={highlightQuery}
                          onFindSimilar={onFindSimilar}
                          onOcrReindex={onOcrReindex}
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
                      onClick={selectForPreview}
                      onDoubleClick={openOnSingleClick ? undefined : (e) => {
                        if (isInteractiveTarget(e)) return;
                        onOpenFile(group.file_path);
                      }}
                    >
                      <GroupedSearchResultItem
                        domId={domId}
                        isSelected={isSelected}
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
                // overflow-x-clip: 컨테이너가 컬럼 MIN 총합보다 좁으면 트랙이 넘치는데,
                // 그 페인팅을 래퍼 경계에서 자른다 (clip은 스크롤 컨테이너를 만들지
                // 않아 sticky 헤더 유지 — hidden 금지)
                className={isFilenameMode ? "overflow-x-clip" : undefined}
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
                <div ref={listRef} role="listbox" aria-label="검색 결과" aria-activedescendant={selectedDomId} className={`result-list-divided ${isCompact ? "space-y-0.5" : "space-y-1.5"}`}>
                {flatResults.slice(0, visibleCount).map(({ r: result, i: index }, pos) => (
                  <div
                    key={`${result.file_path}-${result.chunk_index ?? 0}-${result.start_offset ?? index}`}
                    className={`group ${pos < 10 ? "stagger-item" : ""}`}
                    style={{
                      contain: "layout style",
                      ...(pos < 10 && { animationDelay: `${pos * 30}ms` }),
                    }}
                    onClick={() => onSelectResult?.(index)}
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
                      onOcrReindex={onOcrReindex}
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
              <svg width="13" height="13" viewBox="0 0 24 24" fill="var(--color-on-accent-ai)" stroke="none" aria-hidden="true">
                <path d="M12 2l2.4 6.4L21 11l-6.6 2.4L12 21l-2.4-7.6L3 11l6.6-2.4L12 2z" />
              </svg>
            </div>
            <div className="flex-1 text-left min-w-0">
              <span className="text-xs font-medium" style={{ color: "var(--color-accent-ai)" }}>
                Anything에게 물어보기
              </span>
              <p className="text-2xs text-[var(--color-text-muted)] truncate">
                검색 결과를 AI가 분석하여 답변합니다
              </p>
            </div>
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="var(--color-accent-ai)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="shrink-0 opacity-50 group-hover:opacity-100 transition-opacity" aria-hidden="true">
              <path d="M5 12h14M12 5l7 7-7 7" />
            </svg>
          </button>
        )}
      </div>
    );
  }

  // 자연어 모드: 아직 Enter 안 눌렀을 때 (결과 없고 로딩도 아닌 상태)
  if (paradigm === "natural" && hasQuery && !isLoading && !nlSubmitted) {
    return <NlReadyState />;
  }

  if (hasQuery && !isLoading) {
    // 결과는 있는데 필터에 전부 가려짐 — 검색어 탓이 아니므로 "결과 없음"보다 먼저
    if (totalResultCount > 0) {
      return (
        <FilteredOutState
          query={query}
          hiddenCount={totalResultCount}
          minConfidence={minConfidence}
          onClearFilters={onClearResultFilters}
        />
      );
    }
    // 문서를 읽는 중 — 첫 실행이면 색인 문서가 0개라 "문서 없음"보다 먼저 봐야 한다
    if (isIndexing) {
      return <IndexingState query={query} indexProgress={indexProgress} />;
    }
    if (indexedFiles === 0) {
      return <NoIndexState onAddFolder={onAddFolder} />;
    }
    return (
      <NoResultsState
        query={query}
        paradigm={paradigm}
        parsedQuery={parsedQuery}
        searchMode={searchMode}
        onRetryWithoutFilters={onRetryWithoutFilters}
        onFocusSearch={onFocusSearch}
        onSwitchToFilenameSearch={onSwitchToFilenameSearch}
      />
    );
  }

  // 스마트 검색 모드 — 초기 안내 화면
  if (paradigm === "natural") {
    return <SmartSearchGuide onSelectExample={onSelectSearch} />;
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
      isIndexing={isIndexing}
      indexProgress={indexProgress}
    />
  );
});

/** 더 보기 버튼 */
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
        <ChevronDown className="w-4 h-4" aria-hidden="true" />
        {remaining}개 더 보기
      </button>
    </div>
  );
}
