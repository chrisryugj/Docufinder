import { useState, useCallback, useEffect } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import type { SearchResult, GroupedSearchResult, ViewMode } from "../../types/search";
import type { ViewDensity } from "../../types/settings";
import { SearchResultItem } from "./SearchResultItem";
import { GroupedSearchResultItem } from "./GroupedSearchResultItem";
import { HighlightedFilename } from "./HighlightedFilename";
import { cleanPath } from "../../utils/cleanPath";
import { Badge } from "../ui/Badge";

interface SearchResultListProps {
  results: SearchResult[];
  /** 파일명 검색 결과 (통합 모드에서 상단 표시) */
  filenameResults?: SearchResult[];
  groupedResults?: GroupedSearchResult[];
  viewMode?: ViewMode;
  onViewModeChange?: (mode: ViewMode) => void;
  viewDensity?: ViewDensity;
  onViewDensityChange?: (density: ViewDensity) => void;
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
  /** 외부 스크롤 컨테이너 ref (가상화용) */
  scrollContainerRef?: React.RefObject<HTMLDivElement | null>;
}

// 점진적 로딩: 한 번에 표시할 결과 수
const PAGE_SIZE = 50;

export function SearchResultList({
  results,
  filenameResults = [],
  groupedResults = [],
  viewMode = "flat",
  onViewModeChange,
  viewDensity = "normal",
  onViewDensityChange,
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
  scrollContainerRef,
}: SearchResultListProps) {
  const [expandedIndex, setExpandedIndex] = useState<number | null>(null);
  const [isFilenameCollapsed, setIsFilenameCollapsed] = useState(false);
  // 그룹 뷰 펼침 상태 (file_path로 관리)
  const [expandedGroups, setExpandedGroups] = useState<Set<string>>(new Set());
  const isCompact = viewDensity === "compact";

  // 그룹 펼침 토글
  const handleToggleGroupExpand = useCallback((filePath: string) => {
    setExpandedGroups(prev => {
      const next = new Set(prev);
      if (next.has(filePath)) {
        next.delete(filePath);
      } else {
        next.add(filePath);
      }
      return next;
    });
  }, []);

  // 아이템 높이 추정 (컴팩트 vs 기본)
  const estimatedItemHeight = isCompact ? 80 : 120;

  // 가상화 설정 (100개 이상 + 외부 스크롤 컨테이너 있을 때만 적용)
  const shouldVirtualize = results.length > 100 && scrollContainerRef?.current != null;

  const virtualizer = useVirtualizer({
    count: results.length,
    getScrollElement: () => scrollContainerRef?.current ?? null,
    estimateSize: () => estimatedItemHeight,
    overscan: 10, // 버퍼 아이템 수 증가
    enabled: shouldVirtualize,
  });

  // 검색 결과 변경 시 상태 초기화
  useEffect(() => {
    setExpandedIndex(null);
    // 스크롤 위치 리셋 (외부 컨테이너 사용)
    if (scrollContainerRef?.current) {
      scrollContainerRef.current.scrollTop = 0;
    }
  }, [results, scrollContainerRef]);

  // 확장 토글 핸들러
  const handleToggleExpand = useCallback((index: number) => {
    setExpandedIndex((prev) => (prev === index ? null : index));
  }, []);

  // 전체 결과 (파일명 + 내용)
  const hasResults = results.length > 0 || filenameResults.length > 0;

  // 결과가 있을 때
  if (hasResults) {
    return (
      <div className="space-y-3">
        {/* 툴바: 뷰 모드 + 결과 수 (좌측) | 복사/CSV (우측) */}
        <div className="flex items-center gap-3 mb-2">
          {/* 좌측: 뷰 모드 토글 + 결과 수 */}
          <div className="flex items-center gap-2">
            {/* 뷰 모드 토글 */}
            {onViewModeChange && (
              <div className="flex items-center gap-0.5 border rounded-md p-0.5" style={{ backgroundColor: "var(--color-bg-tertiary)", borderColor: "var(--color-border)" }}>
                <button
                  onClick={() => onViewModeChange("flat")}
                  className="p-1 rounded-sm transition-colors"
                  style={{
                    backgroundColor: viewMode === "flat" ? "var(--color-bg-secondary)" : "transparent",
                    color: viewMode === "flat" ? "var(--color-accent)" : "var(--color-text-muted)",
                    boxShadow: viewMode === "flat" ? "0 1px 2px rgba(0,0,0,0.05)" : "none",
                  }}
                  title="목록 보기"
                  aria-label="목록 보기"
                  aria-pressed={viewMode === "flat"}
                >
                  <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 6h16M4 12h16M4 18h16" />
                  </svg>
                </button>
                <button
                  onClick={() => onViewModeChange("grouped")}
                  className="p-1 rounded-sm transition-colors"
                  style={{
                    backgroundColor: viewMode === "grouped" ? "var(--color-bg-secondary)" : "transparent",
                    color: viewMode === "grouped" ? "var(--color-accent)" : "var(--color-text-muted)",
                    boxShadow: viewMode === "grouped" ? "0 1px 2px rgba(0,0,0,0.05)" : "none",
                  }}
                  title="파일별 그룹 보기"
                  aria-label="파일별 그룹 보기"
                  aria-pressed={viewMode === "grouped"}
                >
                  <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
                  </svg>
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
              className="flex items-center gap-1.5 px-2.5 py-1 text-xs rounded-md transition-colors border font-medium"
              style={{
                backgroundColor: "var(--color-bg-secondary)",
                borderColor: "var(--color-border)",
                color: "var(--color-text-secondary)",
              }}
              onMouseEnter={(e) => {
                e.currentTarget.style.borderColor = "var(--color-accent)";
                e.currentTarget.style.color = "var(--color-accent)";
                e.currentTarget.style.backgroundColor = "var(--color-accent-light)";
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.borderColor = "var(--color-border)";
                e.currentTarget.style.color = "var(--color-text-secondary)";
                e.currentTarget.style.backgroundColor = "var(--color-bg-secondary)";
              }}
              title="검색 결과 클립보드 복사"
            >
              <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
                  d="M8 5H6a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2v-1M8 5a2 2 0 002 2h2a2 2 0 002-2M8 5a2 2 0 012-2h2a2 2 0 012 2m0 0h2a2 2 0 012 2v3m2 4H10m0 0l3-3m-3 3l3 3" />
              </svg>
              복사
            </button>
            <button
              onClick={onExportCSV}
              className="flex items-center gap-1.5 px-2.5 py-1 text-xs rounded-md transition-colors border font-medium"
              style={{
                backgroundColor: "var(--color-bg-secondary)",
                borderColor: "var(--color-border)",
                color: "var(--color-text-secondary)",
              }}
              onMouseEnter={(e) => {
                e.currentTarget.style.borderColor = "var(--color-accent)";
                e.currentTarget.style.color = "var(--color-accent)";
                e.currentTarget.style.backgroundColor = "var(--color-accent-light)";
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.borderColor = "var(--color-border)";
                e.currentTarget.style.color = "var(--color-text-secondary)";
                e.currentTarget.style.backgroundColor = "var(--color-bg-secondary)";
              }}
              title="CSV 파일로 내보내기"
            >
              <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
                  d="M12 10v6m0 0l-3-3m3 3l3-3m2 8H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
              </svg>
              CSV
            </button>
          </div>
        </div>

        {/* 파일명 매치 섹션 (토글 가능) */}
        {filenameResults.length > 0 && (
          <div className="mb-4">
            <button
              type="button"
              onClick={() => setIsFilenameCollapsed(!isFilenameCollapsed)}
              className="flex items-center gap-2 px-3 py-2 rounded-lg mb-2 w-full text-left transition-colors"
              style={{ backgroundColor: "var(--color-bg-tertiary)" }}
              onMouseEnter={(e) => {
                e.currentTarget.style.backgroundColor = "var(--color-bg-subtle)";
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.backgroundColor = "var(--color-bg-tertiary)";
              }}
            >
              <svg
                className={`w-4 h-4 transition-transform ${isFilenameCollapsed ? "" : "rotate-90"}`}
                style={{ color: "var(--color-text-muted)" }}
                fill="currentColor"
                viewBox="0 0 20 20"
              >
                <path fillRule="evenodd" d="M7.293 14.707a1 1 0 010-1.414L10.586 10 7.293 6.707a1 1 0 011.414-1.414l4 4a1 1 0 010 1.414l-4 4a1 1 0 01-1.414 0z" clipRule="evenodd" />
              </svg>
              <svg className="w-4 h-4" style={{ color: "var(--color-accent)" }} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M7 21h10a2 2 0 002-2V9.414a1 1 0 00-.293-.707l-5.414-5.414A1 1 0 0012.586 3H7a2 2 0 00-2 2v14a2 2 0 002 2z" />
              </svg>
              <span className="text-sm font-medium" style={{ color: "var(--color-text-primary)" }}>
                파일명 매치
              </span>
              <span className="text-xs px-1.5 py-0.5 rounded" style={{ backgroundColor: "var(--color-accent-light)", color: "var(--color-accent)" }}>
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
                    className="flex items-center gap-3 px-3 py-2 rounded-lg cursor-pointer transition-colors"
                    style={{ backgroundColor: "var(--color-bg-secondary)" }}
                    onClick={() => onOpenFile(result.file_path)}
                    onMouseEnter={(e) => {
                      e.currentTarget.style.backgroundColor = "var(--color-bg-tertiary)";
                    }}
                    onMouseLeave={(e) => {
                      e.currentTarget.style.backgroundColor = "var(--color-bg-secondary)";
                    }}
                  >
                    <svg className="w-5 h-5 flex-shrink-0" style={{ color: "var(--color-text-muted)" }} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
                    </svg>
                    <div className="flex-1 min-w-0">
                      <div className="font-medium truncate" style={{ color: "var(--color-text-primary)" }}>
                        <HighlightedFilename filename={result.file_name} query={query} />
                      </div>
                      <div className="text-xs truncate" style={{ color: "var(--color-text-muted)" }}>
                        {cleanPath(result.file_path)}
                      </div>
                    </div>
                    <div
                      className="text-xs px-2 py-0.5 rounded"
                      style={{ backgroundColor: "var(--color-bg-tertiary)", color: "var(--color-text-muted)" }}
                    >
                      {result.location_hint || result.file_path.split('.').pop()?.toUpperCase()}
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {/* 내용 매치 섹션 헤더 */}
        {filenameResults.length > 0 && results.length > 0 && (
          <div
            className="flex items-center gap-2 px-3 py-2 rounded-lg mb-2"
            style={{ backgroundColor: "var(--color-bg-tertiary)" }}
          >
            <svg className="w-4 h-4" style={{ color: "var(--color-text-muted)" }} fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
            </svg>
            <span className="text-sm font-medium" style={{ color: "var(--color-text-primary)" }}>
              내용 매치
            </span>
            <span className="text-xs px-1.5 py-0.5 rounded" style={{ backgroundColor: "var(--color-bg-tertiary)", color: "var(--color-text-muted)" }}>
              {results.length}
            </span>
          </div>
        )}

        {/* 결과 목록 */}
        {results.length > 0 && (
          viewMode === "grouped" && groupedResults.length > 0 ? (
            // 그룹 뷰 (가상화 미적용)
            <div role="listbox" aria-label="검색 결과" className={isCompact ? "space-y-1" : "space-y-3"}>
              {groupedResults.map((group) => (
                <GroupedSearchResultItem
                  key={group.file_path}
                  group={group}
                  onOpenFile={onOpenFile}
                  onCopyPath={onCopyPath}
                  onOpenFolder={onOpenFolder}
                  isCompact={isCompact}
                  searchQuery={query}
                  isExpanded={expandedGroups.has(group.file_path)}
                  onToggleExpand={() => handleToggleGroupExpand(group.file_path)}
                />
              ))}
            </div>
          ) : shouldVirtualize ? (
            // 플랫 뷰 + 가상화 (100개 이상: 외부 스크롤 컨테이너 사용)
            <div
              role="listbox"
              aria-label="검색 결과"
              style={{
                height: `${virtualizer.getTotalSize()}px`,
                width: "100%",
                position: "relative",
              }}
            >
              {virtualizer.getVirtualItems().map((virtualItem) => {
                const result = results[virtualItem.index];
                const index = virtualItem.index;
                return (
                  <div
                    key={`${result.file_path}-${result.chunk_index}-${index}`}
                    data-index={index}
                    ref={virtualizer.measureElement}
                    className="group"
                    style={{
                      position: "absolute",
                      top: 0,
                      left: 0,
                      width: "100%",
                      transform: `translateY(${virtualItem.start}px)`,
                      paddingBottom: isCompact ? "4px" : "12px",
                    }}
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
                );
              })}
            </div>
          ) : (
            // 플랫 뷰 (100개 이하: 일반 렌더링)
            <div role="listbox" aria-label="검색 결과" className={isCompact ? "space-y-1" : "space-y-3"}>
              {results.map((result, index) => (
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
          <svg
            className="w-10 h-10 opacity-60"
            style={{ color: "var(--color-text-muted)" }}
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
            aria-hidden="true"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={1.5}
              d="M9.172 16.172a4 4 0 015.656 0M9 10h.01M15 10h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
            />
          </svg>
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
        <p className="text-sm" style={{ color: "var(--color-text-muted)" }}>
          다른 검색어를 시도하거나, 검색 모드를 변경해보세요
        </p>
      </div>
    );
  }

  // 초기 상태 - 온보딩 가이드
  return (
    <div className="text-center py-16">
      <div
        className="w-20 h-20 mx-auto mb-6 rounded-2xl flex items-center justify-center"
        style={{ backgroundColor: "var(--color-accent-light)" }}
      >
        <svg
          className="w-10 h-10"
          style={{ color: "var(--color-accent)" }}
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
          aria-hidden="true"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={1.5}
            d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
          />
        </svg>
      </div>
      <h3
        className="text-lg font-semibold mb-2"
        style={{ color: "var(--color-text-primary)" }}
      >
        검색을 시작하세요
      </h3>
      <p style={{ color: "var(--color-text-muted)" }}>
        폴더를 선택하고 검색어를 입력하면 문서를 찾을 수 있습니다
      </p>
    </div>
  );
}
