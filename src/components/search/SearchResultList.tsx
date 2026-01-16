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
}

export function SearchResultList({
  results,
  query,
  isLoading,
  selectedIndex,
  onOpenFile,
  onCopyPath,
  onOpenFolder,
}: SearchResultListProps) {
  const [expandedIndex, setExpandedIndex] = useState<number | null>(null);

  // 결과가 있을 때
  if (results.length > 0) {
    return (
      <div className="space-y-3" role="listbox" aria-label="검색 결과">
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
    );
  }

  // 검색어가 있지만 결과 없음
  if (query.trim() && !isLoading) {
    return (
      <div className="text-center text-gray-500 py-12">
        <svg
          className="w-16 h-16 mx-auto mb-4 opacity-50"
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
        <p>검색 결과가 없습니다</p>
      </div>
    );
  }

  // 초기 상태
  return (
    <div className="text-center text-gray-500 py-12">
      <svg
        className="w-16 h-16 mx-auto mb-4 opacity-50"
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
      <p>폴더를 선택하고 검색을 시작하세요</p>
    </div>
  );
}
