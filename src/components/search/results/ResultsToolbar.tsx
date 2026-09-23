import { memo } from "react";
import { List, LayoutGrid, ClipboardCopy, FileDown } from "lucide-react";
import type { ViewMode } from "../../../types/search";
import { Badge } from "../../ui/Badge";

interface ResultsToolbarProps {
  viewMode: ViewMode;
  onViewModeChange?: (mode: ViewMode) => void;
  resultCount?: number;
  totalResultCount?: number;
  minConfidence?: number;
  searchTime?: number | null;
  onCopyAll?: () => void;
  onExportCSV?: () => void;
  /** 파일 형식·기간·결과 내 검색 조건을 풀어 가려진 결과까지 보기 */
  onClearFilters?: () => void;
}

/** 결과 툴바: 뷰 모드 + 결과 수 + 복사/CSV */
export const ResultsToolbar = memo(function ResultsToolbar({
  viewMode,
  onViewModeChange,
  resultCount,
  totalResultCount,
  minConfidence = 0,
  searchTime,
  onCopyAll,
  onExportCSV,
  onClearFilters,
}: ResultsToolbarProps) {
  const isFiltered = totalResultCount !== undefined && resultCount !== undefined && totalResultCount > resultCount;
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
              {isFiltered ? `${totalResultCount}개 중 ${resultCount}개` : `${resultCount}개`}
            </Badge>
            {minConfidence > 0 && (
              <Badge variant="primary">{minConfidence}%↑</Badge>
            )}
            {searchTime !== null && searchTime !== undefined && (
              <Badge variant="secondary">{`${(searchTime / 1000).toFixed(2)}초`}</Badge>
            )}
          </div>
        )}
        {isFiltered && resultCount !== undefined && resultCount > 0 && onClearFilters && (
          <button
            type="button"
            onClick={onClearFilters}
            className="text-xs font-medium px-1.5 py-0.5 rounded hover:underline underline-offset-2"
            style={{ color: "var(--color-accent)" }}
            title="파일 형식·기간·결과 내 검색 조건을 풀고 모든 결과 보기"
          >
            필터 풀기
          </button>
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
});
