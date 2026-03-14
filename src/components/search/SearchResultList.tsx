import { useState, useCallback, useEffect, useLayoutEffect, useRef, memo } from "react";
import { List, LayoutGrid, ClipboardCopy, FileDown, ChevronRight, FileText, FileSearch, Frown, PenLine, ArrowLeftRight, Filter, ChevronDown } from "lucide-react";
import type { SearchResult, GroupedSearchResult, ViewMode, RecentSearch } from "../../types/search";
import type { ViewDensity } from "../../types/settings";
import { SearchResultItem } from "./SearchResultItem";
import { GroupedSearchResultItem } from "./GroupedSearchResultItem";
import { HighlightedFilename } from "./HighlightedFilename";
import { WelcomeHero } from "./WelcomeHero";
import { cleanPath } from "../../utils/cleanPath";
import { Badge, getFileTypeBadgeVariant } from "../ui/Badge";
import { FileIcon } from "../ui/FileIcon";

interface SearchResultListProps {
  results: SearchResult[];
  /** 파일명 검색 결과 (통합 모드에서 상단 표시) */
  filenameResults?: SearchResult[];
  groupedResults?: GroupedSearchResult[];
  viewMode?: ViewMode;
  onViewModeChange?: (mode: ViewMode) => void;
  viewDensity?: ViewDensity;
  query: string;
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
  /** 시맨틱 검색 활성 여부 */
  semanticEnabled?: boolean;
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
  semanticEnabled,
}: SearchResultListProps) {
  const pageSize = resultsPerPage || DEFAULT_RESULTS_PER_PAGE;
  const [expandedIndex, setExpandedIndex] = useState<number | null>(null);
  const [isFilenameCollapsed, setIsFilenameCollapsed] = useState(false);
  // 그룹 뷰 펼침 상태 (file_path로 관리)
  const [expandedGroups, setExpandedGroups] = useState<Set<string>>(new Set());
  const [visibleCount, setVisibleCount] = useState(pageSize);
  const isCompact = viewDensity === "compact";
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

  // 키보드로 선택 변경 시 스크롤 따라가기
  useEffect(() => {
    if (selectedIndex == null || selectedIndex < 0) return;
    const el = document.getElementById(`search-result-${selectedIndex}`);
    el?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }, [selectedIndex]);

  // 확장 토글 핸들러
  const handleToggleExpand = useCallback((index: number) => {
    captureScrollAnchor(`search-result-${index}`);
    setExpandedIndex((prev) => (prev === index ? null : index));
  }, [captureScrollAnchor]);

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

  // 검색 중 (결과 없음) — 전체 화면 스피너
  if (isLoading && !hasResults && query.trim()) {
    return (
      <div className="flex flex-col items-center justify-center py-20 gap-4">
        <div className="relative w-12 h-12">
          <div
            className="absolute inset-0 rounded-full animate-spin"
            style={{
              border: "2.5px solid var(--color-border)",
              borderTopColor: "var(--color-accent)",
            }}
          />
          <div
            className="absolute inset-2 rounded-full animate-spin"
            style={{
              border: "2px solid transparent",
              borderBottomColor: "var(--color-accent-hover)",
              animationDirection: "reverse",
              animationDuration: "0.6s",
            }}
          />
        </div>
        <p className="text-sm font-medium animate-pulse" style={{ color: "var(--color-text-muted)" }}>
          검색 중...
        </p>
      </div>
    );
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

        {/* 툴바: 뷰 모드 + 결과 수 (좌측) | 복사/CSV (우측) */}
        <div className="flex items-center gap-3 mb-2">
          {/* 좌측: 뷰 모드 토글 + 결과 수 */}
          <div className="flex items-center gap-2">
            {/* 뷰 모드 토글 */}
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

            {/* 결과 수 + 신뢰도 + 검색 시간 배지 */}
            {resultCount !== undefined && resultCount > 0 && (
              <div className="flex items-center gap-0.5">
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

          {/* 우측: 복사/CSV */}
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

        {/* 파일명 매치 섹션 (토글 가능) */}
        {filenameResults.length > 0 && (
          <div className="mb-2">
            <button
              type="button"
              onClick={() => setIsFilenameCollapsed(!isFilenameCollapsed)}
              aria-expanded={!isFilenameCollapsed}
              className="flex items-center gap-2 px-3 py-2 rounded-r-lg mb-2 w-full text-left hover-bg-subtle"
              style={{
                borderLeft: "3px solid var(--color-text-muted)",
                backgroundColor: "var(--color-bg-tertiary)",
              }}
            >
              <ChevronRight
                className={`w-3.5 h-3.5 transition-transform ${isFilenameCollapsed ? "" : "rotate-90"}`}
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
              {isFilenameCollapsed && (
                <span className="text-xs ml-auto" style={{ color: "var(--color-text-muted)" }}>
                  클릭하여 펼치기
                </span>
              )}
            </button>
            {!isFilenameCollapsed && (
              <div className={isCompact ? "space-y-1" : "space-y-2"}>
                {filenameResults.map((result, index) => (
                  <div
                    key={`filename-${result.file_path}-${index}`}
                    className="flex items-center gap-3 px-3 py-2 rounded-lg cursor-pointer transition-colors hover:bg-[var(--color-bg-tertiary)]"
                    style={{ backgroundColor: "var(--color-bg-secondary)" }}
                    role="button"
                    tabIndex={0}
                    onClick={() => onOpenFile(result.file_path)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" || e.key === " ") {
                        e.preventDefault();
                        onOpenFile(result.file_path);
                      }
                    }}
                  >
                    <FileIcon fileName={result.file_name} size="sm" />
                    <div className="flex-1 min-w-0">
                      <div className="font-medium truncate" style={{ color: "var(--color-text-primary)" }}>
                        <HighlightedFilename filename={result.file_name} query={query} />
                      </div>
                      <div className="text-xs truncate" style={{ color: "var(--color-text-muted)" }}>
                        {cleanPath(result.file_path)}
                      </div>
                    </div>
                    {result.has_hwp_pair && (
                      <span
                        className="text-[10px] px-1.5 py-0.5 rounded font-medium"
                        style={{ backgroundColor: "rgba(245, 158, 11, 0.15)", color: "var(--color-warning, #f59e0b)" }}
                        title="같은 위치에 원본 HWP 파일이 있습니다"
                      >
                        HWP
                      </span>
                    )}
                    <Badge variant={getFileTypeBadgeVariant(result.file_name)}>
                      {(result.file_name.split('.').pop() || '').toUpperCase()}
                    </Badge>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {/* 섹션 구분선 */}
        {filenameResults.length > 0 && results.length > 0 && (
          <div className="my-4" style={{ borderTop: "1px solid var(--color-border)" }} />
        )}

        {/* 내용 매치 섹션 헤더 */}
        {filenameResults.length > 0 && results.length > 0 && (
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
              {results.length}
            </span>
          </div>
        )}

        {/* 결과 목록 */}
        {results.length > 0 && (
          viewMode === "grouped" && groupedResults.length > 0 ? (
            // 그룹 뷰
            <>
              <div ref={listRef} role="listbox" aria-label="검색 결과" aria-activedescendant={selectedIndex != null && selectedIndex >= 0 ? `search-result-${selectedIndex}` : undefined} className={isCompact ? "space-y-1" : "space-y-3"}>
                {groupedResults.slice(0, visibleCount).map((group, index) => (
                  <GroupedSearchResultItem
                    key={group.file_path}
                    domId={`grouped-search-result-${index}`}
                    group={group}
                    onOpenFile={onOpenFile}
                    onCopyPath={onCopyPath}
                    onOpenFolder={onOpenFolder}
                    isCompact={isCompact}
                    searchQuery={query}
                    isExpanded={expandedGroups.has(group.file_path)}
                    onToggleExpand={() => handleToggleGroupExpand(group.file_path, `grouped-search-result-${index}`)}
                  />
                ))}
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
              <div ref={listRef} role="listbox" aria-label="검색 결과" aria-activedescendant={selectedIndex != null && selectedIndex >= 0 ? `search-result-${selectedIndex}` : undefined} className={isCompact ? "space-y-1" : "space-y-3"}>
                {results.slice(0, visibleCount).map((result, index) => (
                  <div
                    key={`${result.file_path}-${result.chunk_index}-${index}`}
                    className="group"
                    style={{ contain: "layout style" }}
                  >
                    <SearchResultItem
                      result={result}
                      index={index}
                      isExpanded={expandedIndex === index}
                      isSelected={selectedIndex === index}
                      isCompact={isCompact}
                      onToggleExpand={() => handleToggleExpand(index)}
                      onOpenFile={onOpenFile}
                      onCopyPath={onCopyPath}
                      onOpenFolder={onOpenFolder}
                      refineKeywords={refineKeywords}
                      query={query}
                    />
                  </div>
                ))}
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
      </div>
    );
  }

  // 검색어가 있지만 결과 없음 - 맥락 있는 피드백
  if (query.trim() && !isLoading) {
    const truncatedQuery = query.length > 30 ? query.slice(0, 30) + "..." : query;
    return (
      <div className="text-center py-16">
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
        <div className="space-y-2 text-sm" style={{ color: "var(--color-text-muted)" }}>
          <p>다음을 시도해보세요:</p>
          <div className="flex flex-wrap justify-center gap-2 mt-3">
            <span className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-full text-xs" style={{ backgroundColor: "var(--color-bg-tertiary)", border: "1px solid var(--color-border)" }}>
              <PenLine className="w-3.5 h-3.5" />
              다른 검색어 입력
            </span>
            <span className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-full text-xs" style={{ backgroundColor: "var(--color-bg-tertiary)", border: "1px solid var(--color-border)" }}>
              <ArrowLeftRight className="w-3.5 h-3.5" />
              검색 모드 변경
            </span>
            <span className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-full text-xs" style={{ backgroundColor: "var(--color-bg-tertiary)", border: "1px solid var(--color-border)" }}>
              <Filter className="w-3.5 h-3.5" />
              필터 조건 완화
            </span>
          </div>
        </div>
      </div>
    );
  }

  // 초기 상태 — 웰컴 히어로
  return (
    <WelcomeHero
      indexedFiles={indexedFiles}
      indexedFolders={indexedFolders}
      recentSearches={recentSearches}
      onSelectSearch={onSelectSearch}
      semanticEnabled={semanticEnabled}
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
