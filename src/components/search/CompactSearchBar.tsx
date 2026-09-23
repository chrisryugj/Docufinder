import { forwardRef, memo, useCallback } from "react";
import { Search, HelpCircle, Settings, Download, X } from "lucide-react";
import type { UpdatePhase } from "../../hooks/useUpdater";
import { IS_LITE } from "../../utils/buildFlavor";
import { useSearchInput } from "../../hooks/useSearchInput";
import type { SearchParadigm } from "../../types/search";
import type { IndexStatus } from "../../types/index";
import SearchParadigmToggle from "./SearchParadigmToggle";

interface CompactSearchBarProps {
  query: string;
  onQueryChange: (query: string) => void;
  isLoading: boolean;
  status: IndexStatus | null;
  resultCount: number;
  onExpand: () => void;
  onAddFolder: () => void;
  onOpenSettings: () => void;
  onOpenHelp: () => void;
  isIndexing: boolean;
  isSidebarOpen: boolean;
  onCompositionStart?: () => void;
  onCompositionEnd?: (finalValue: string) => void;
  /** 검색 패러다임 */
  paradigm?: SearchParadigm;
  onParadigmChange?: (p: SearchParadigm) => void;
  /** 스마트 검색·질문 제출 (Enter) */
  onSubmitNatural?: () => void;
  /** 업데이트 배지 */
  updatePhase?: UpdatePhase;
  onOpenUpdate?: () => void;
}

export const CompactSearchBar = memo(forwardRef<HTMLInputElement, CompactSearchBarProps>(
  (
    {
      query,
      onQueryChange,
      isLoading,
      status,
      resultCount,
      onExpand,
      onOpenSettings,
      onOpenHelp,
      isSidebarOpen,
      onCompositionStart,
      onCompositionEnd,
      paradigm = "instant",
      onParadigmChange,
      onSubmitNatural,
      updatePhase,
      onOpenUpdate,
    },
    ref
  ) => {
    // SearchBar와 동일: 인덱싱 1회 완료 시 패러다임 토글 노출 (이슈 #32 — 시맨틱 게이트 제거)
    const canUseParadigms = (status?.indexed_files ?? 0) > 0;
    // lite: 업데이터 플러그인이 없어 업데이트 배지를 띄울 일이 없다
    const updateVisible =
      !IS_LITE && (
      updatePhase === "available" ||
      updatePhase === "downloading" ||
      updatePhase === "installing" ||
      updatePhase === "ready-to-restart");
    const isNatural = paradigm === "natural";
    const isQuestion = paradigm === "question";
    // 스마트 검색·질문은 Enter 로 제출한다 (종전엔 질문 모드에서 Enter 가 아무 일도 안 했다)
    const submitsOnEnter = isNatural || isQuestion;
    const { innerRef, imeHandlers } = useSearchInput({
      query,
      onQueryChange,
      onCompositionStart,
      onCompositionEnd,
      forwardedRef: ref,
    });

    const handleKeyDown = useCallback(
      (e: React.KeyboardEvent<HTMLInputElement>) => {
        if (submitsOnEnter && e.key === "Enter" && !e.nativeEvent.isComposing) {
          e.preventDefault();
          onSubmitNatural?.();
        }
      },
      [submitsOnEnter, onSubmitNatural]
    );

    return (
      <div
        // 좌우 여백은 펼친 헤더(Header)와 같게 — 접고 펼 때 로고가 옆으로 튀지 않는다
        className={`flex items-center gap-3 py-2 border-b transition-all duration-300 ${
          isSidebarOpen ? "px-5" : "pl-14 pr-5"
        }`}
        style={{
          backgroundColor: "var(--color-bg-primary)",
          borderColor: "var(--color-border)",
        }}
      >
        {/* 로고 (클릭 시 맨 위로 + 검색창 펼치기. 펼친 헤더의 로고는 홈) */}
        <button
          onClick={onExpand}
          className="flex items-center gap-2 flex-shrink-0 hover:opacity-80 transition-opacity"
          aria-label="맨 위로 가서 검색창 펼치기"
          title="맨 위로 (검색창 펼치기)"
        >
          <img src="/anything.png" alt="Anything" className="w-6 h-6 object-contain dark:hidden" />
          <img src="/anything-l.png" alt="Anything" className="w-6 h-6 object-contain hidden dark:block" />
        </button>

        {/* 패러다임 토글 */}
        {onParadigmChange && canUseParadigms && (
          <SearchParadigmToggle paradigm={paradigm} onChange={onParadigmChange} />
        )}

        {/* 검색 입력 */}
        <div
          className="flex items-center flex-1 min-w-0 px-3 py-1.5 rounded-lg focus-within:ring-2 focus-within:ring-[var(--color-accent)] focus-within:ring-offset-1"
          style={{
            backgroundColor: "var(--color-bg-secondary)",
            border: "1px solid var(--color-border)",
          }}
        >
          <Search className="w-4 h-4 flex-shrink-0" style={{ color: "var(--color-text-muted)" }} />
          <input
            ref={innerRef}
            type="text"
            defaultValue={query}
            {...imeHandlers}
            onKeyDown={submitsOnEnter ? handleKeyDown : undefined}
            placeholder={isQuestion ? "문서에 대해 무엇이든 물어보세요" : isNatural ? "작년 예산 한글 문서, 최근 30일 계약서 PDF만" : "예산 집행현황, 계약서, 인사발령"}
            className="flex-1 min-w-0 bg-transparent border-none text-sm focus:outline-none ml-2"
            style={{ color: "var(--color-text-primary)" }}
            aria-label="검색어 입력"
          />
          {query && (
            <button
              type="button"
              onClick={() => {
                onQueryChange("");
                innerRef.current?.focus();
              }}
              className="ml-2 p-0.5 rounded flex-shrink-0 hover:bg-[var(--color-bg-tertiary)]"
              style={{ color: "var(--color-text-muted)" }}
              title="검색어 지우기"
              aria-label="검색어 지우기"
            >
              <X className="w-3.5 h-3.5" aria-hidden="true" />
            </button>
          )}
          {isLoading && (
            <div
              className="w-4 h-4 rounded-full border-2 animate-spin ml-2 flex-shrink-0"
              style={{
                borderColor: "var(--color-border)",
                borderTopColor: "var(--color-accent)",
              }}
              role="status"
              aria-label="검색 중"
            />
          )}

          {submitsOnEnter && (
            <span className="text-xs ml-2 flex-shrink-0" style={{ color: "var(--color-text-muted)" }}>
              Enter ↵
            </span>
          )}
        </div>

        {/* 결과 수 */}
        {resultCount > 0 && (
          <span className="text-xs font-medium flex-shrink-0" style={{ color: "var(--color-text-muted)" }}>
            {resultCount}건
          </span>
        )}

        {/* 구분선 */}
        <div className="w-px h-5 flex-shrink-0" style={{ backgroundColor: "var(--color-border)" }} />

        {/* 업데이트 배지 */}
        {updateVisible && onOpenUpdate && (
          <button
            onClick={onOpenUpdate}
            className="relative p-1.5 rounded hover:bg-[var(--color-bg-tertiary)] transition-colors flex-shrink-0"
            aria-label="업데이트"
            title={
              updatePhase === "available" ? "업데이트 사용 가능" :
              updatePhase === "downloading" ? "다운로드 중" :
              updatePhase === "installing" ? "설치 중" :
              "재시작 필요"
            }
          >
            <Download className="w-4 h-4" style={{ color: "var(--color-accent)" }} />
            <span
              className="absolute top-1 right-1 w-1.5 h-1.5 rounded-full animate-pulse"
              style={{ backgroundColor: "var(--color-accent)" }}
            />
          </button>
        )}

        {/* 도움말 */}
        <button
          onClick={onOpenHelp}
          className="p-1.5 rounded hover:bg-[var(--color-bg-tertiary)] transition-colors flex-shrink-0"
          style={{ color: "var(--color-text-muted)" }}
          aria-label="도움말"
        >
          <HelpCircle className="w-4 h-4" />
        </button>

        {/* 설정 */}
        <button
          onClick={onOpenSettings}
          className="p-1.5 rounded hover:bg-[var(--color-bg-tertiary)] transition-colors flex-shrink-0"
          style={{ color: "var(--color-text-muted)" }}
          aria-label="설정"
        >
          <Settings className="w-4 h-4" />
        </button>
      </div>
    );
  }
));

CompactSearchBar.displayName = "CompactSearchBar";
