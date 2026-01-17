import { useState } from "react";
import type { SearchResult, GroupedSearchResult, ViewMode } from "../../types/search";
import type { ViewDensity } from "../../types/settings";
import { SearchResultItem } from "./SearchResultItem";
import { GroupedSearchResultItem } from "./GroupedSearchResultItem";
import { cleanPath } from "../../utils/cleanPath";

interface SearchResultListProps {
  results: SearchResult[];
  /** 파일명 검색 결과 (통합 모드에서 상단 표시) */
  filenameResults?: SearchResult[];
  groupedResults?: GroupedSearchResult[];
  viewMode?: ViewMode;
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
}

export function SearchResultList({
  results,
  filenameResults = [],
  groupedResults = [],
  viewMode = "flat",
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
}: SearchResultListProps) {
  const [expandedIndex, setExpandedIndex] = useState<number | null>(null);
  const isCompact = viewDensity === "compact";

  // 전체 결과 (파일명 + 내용)
  const hasResults = results.length > 0 || filenameResults.length > 0;

  // 결과가 있을 때
  if (hasResults) {
    return (
      <div className="space-y-3">
        {/* 툴바: 보기 모드 토글 + 내보내기 */}
        <div className="flex justify-between items-center mb-2">
          {/* 보기 밀도 토글 */}
          {onViewDensityChange && (
            <div className="flex items-center gap-1 p-0.5 rounded-md" style={{ backgroundColor: "var(--color-bg-tertiary)" }}>
              <button
                onClick={() => onViewDensityChange("normal")}
                className={`flex items-center gap-1 px-2 py-1 text-xs rounded transition-all ${!isCompact ? "font-medium" : ""}`}
                style={{
                  backgroundColor: !isCompact ? "var(--color-bg-primary)" : "transparent",
                  color: !isCompact ? "var(--color-text-primary)" : "var(--color-text-muted)",
                  boxShadow: !isCompact ? "var(--shadow-sm)" : "none",
                }}
                title="기본 보기"
              >
                <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 6h16M4 12h16M4 18h16" />
                </svg>
                기본
              </button>
              <button
                onClick={() => onViewDensityChange("compact")}
                className={`flex items-center gap-1 px-2 py-1 text-xs rounded transition-all ${isCompact ? "font-medium" : ""}`}
                style={{
                  backgroundColor: isCompact ? "var(--color-bg-primary)" : "transparent",
                  color: isCompact ? "var(--color-text-primary)" : "var(--color-text-muted)",
                  boxShadow: isCompact ? "var(--shadow-sm)" : "none",
                }}
                title="컴팩트 보기"
              >
                <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 6h16M4 10h16M4 14h16M4 18h16" />
                </svg>
                컴팩트
              </button>
            </div>
          )}

          {/* 내보내기 버튼 */}
          <div className="flex gap-2 ml-auto">
            <button
              onClick={onCopyAll}
              className="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-md transition-colors border font-medium"
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
              className="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-md transition-colors border font-medium"
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

        {/* 파일명 매치 섹션 (통합 모드) */}
        {filenameResults.length > 0 && (
          <div className="mb-4">
            <div
              className="flex items-center gap-2 px-3 py-2 rounded-lg mb-2"
              style={{ backgroundColor: "var(--color-bg-tertiary)" }}
            >
              <svg className="w-4 h-4" style={{ color: "var(--color-accent)" }} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M7 21h10a2 2 0 002-2V9.414a1 1 0 00-.293-.707l-5.414-5.414A1 1 0 0012.586 3H7a2 2 0 00-2 2v14a2 2 0 002 2z" />
              </svg>
              <span className="text-sm font-medium" style={{ color: "var(--color-text-primary)" }}>
                파일명 매치
              </span>
              <span className="text-xs px-1.5 py-0.5 rounded" style={{ backgroundColor: "var(--color-accent-light)", color: "var(--color-accent)" }}>
                {filenameResults.length}
              </span>
            </div>
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
                      {result.file_name}
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
          <div role="listbox" aria-label="검색 결과" className={isCompact ? "space-y-1" : "space-y-3"}>
            {viewMode === "grouped" && groupedResults.length > 0 ? (
              // 그룹 뷰
              groupedResults.map((group) => (
                <GroupedSearchResultItem
                  key={group.file_path}
                  group={group}
                  onOpenFile={onOpenFile}
                  onCopyPath={onCopyPath}
                  onOpenFolder={onOpenFolder}
                  isCompact={isCompact}
                />
              ))
            ) : (
              // 플랫 뷰
              results.map((result, index) => (
                <div key={`${result.file_path}-${result.chunk_index}-${index}`} className="group">
                  <SearchResultItem
                    result={result}
                    index={index}
                    isExpanded={expandedIndex === index}
                    isSelected={selectedIndex === index}
                    isCompact={isCompact}
                    onToggleExpand={() =>
                      setExpandedIndex(expandedIndex === index ? null : index)
                    }
                    onOpenFile={onOpenFile}
                    onCopyPath={onCopyPath}
                    onOpenFolder={onOpenFolder}
                  />
                </div>
              ))
            )}
          </div>
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
