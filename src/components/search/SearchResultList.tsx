import { useState } from "react";
import type { SearchResult } from "../../types/search";
import { SearchResultItem } from "./SearchResultItem";

interface SearchResultListProps {
  results: SearchResult[];
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

  // 결과가 있을 때
  if (results.length > 0) {
    return (
      <div className="space-y-3">
        {/* 내보내기 버튼 */}
        <div className="flex justify-end gap-2 mb-2">
          <button
            onClick={onCopyAll}
            className="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-md transition-all"
            style={{
              backgroundColor: "var(--color-bg-secondary)",
              color: "var(--color-text-muted)",
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.backgroundColor = "var(--color-bg-tertiary)";
              e.currentTarget.style.color = "var(--color-text-primary)";
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.backgroundColor = "var(--color-bg-secondary)";
              e.currentTarget.style.color = "var(--color-text-muted)";
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
            className="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-md transition-all"
            style={{
              backgroundColor: "var(--color-bg-secondary)",
              color: "var(--color-text-muted)",
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.backgroundColor = "var(--color-bg-tertiary)";
              e.currentTarget.style.color = "var(--color-text-primary)";
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.backgroundColor = "var(--color-bg-secondary)";
              e.currentTarget.style.color = "var(--color-text-muted)";
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

        {/* 결과 목록 */}
        <div role="listbox" aria-label="검색 결과" className="space-y-3">
        {results.map((result, index) => (
          <div key={`${result.file_path}-${result.chunk_index}-${index}`} className="group">
            <SearchResultItem
              result={result}
              index={index}
              isExpanded={expandedIndex === index}
              isSelected={selectedIndex === index}
              onToggleExpand={() =>
                setExpandedIndex(expandedIndex === index ? null : index)
              }
              onOpenFile={onOpenFile}
              onCopyPath={onCopyPath}
              onOpenFolder={onOpenFolder}
            />
          </div>
        ))}
        </div>
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
