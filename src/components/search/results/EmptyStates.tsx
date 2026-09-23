import type { ReactNode } from "react";
import { FileSearch, Frown, PenLine, ArrowLeftRight, Filter, FolderPlus, FileText, CalendarDays, Ban, Layers, type LucideIcon } from "lucide-react";
import type { ParsedQueryInfo } from "../../../types/search";

/** 결과 영역 빈 화면 공통 틀: 아이콘 상자 + 제목 + 본문 */
function EmptyState({ icon: Icon, title, children }: { icon: LucideIcon; title: string; children?: ReactNode }) {
  return (
    <div className="text-center py-16" role="status" aria-live="polite">
      <div
        className="w-20 h-20 mx-auto mb-6 rounded-2xl flex items-center justify-center"
        style={{ backgroundColor: "var(--color-bg-tertiary)" }}
      >
        <Icon
          className="w-10 h-10 opacity-60"
          style={{ color: "var(--color-text-muted)" }}
          strokeWidth={1.5}
          aria-hidden="true"
        />
      </div>
      <h3 className="text-lg font-semibold mb-2" style={{ color: "var(--color-text-primary)" }}>
        {title}
      </h3>
      {children}
    </div>
  );
}

/** 검색어 인용 — 긴 검색어는 글자 수로 자르지 않고 폭에서 말줄임 */
function QuotedQuery({ query }: { query: string }) {
  return (
    <>
      "<span className="inline-block max-w-[18em] truncate align-bottom" style={{ color: "var(--color-accent)" }}>{query}</span>"
    </>
  );
}

/** 자연어 모드: 아직 Enter 안 눌렀을 때 (결과 없고 로딩도 아닌 상태) */
export function NlReadyState() {
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
            color: "var(--color-on-accent)",
          }}
        >Enter</kbd> 키를 누르면 검색합니다
      </p>
    </div>
  );
}

/** 문서 읽는 중 0건 — "결과 없음"이 아니라 아직 다 읽지 못했음을 안내.
 *  첫 실행이면 색인 문서가 0개라도 이 화면이 먼저다 ("폴더를 추가하세요"로 오해 방지) */
export function IndexingState({
  query,
  indexProgress,
}: {
  query: string;
  indexProgress?: { processed_files: number; total_files: number } | null;
}) {
  return (
    <EmptyState icon={FileSearch} title="아직 문서를 읽는 중입니다">
      <p style={{ color: "var(--color-text-muted)" }}>
        다 읽고 나면 <QuotedQuery query={query} /> 결과가 나올 수 있어요
      </p>
      {indexProgress && indexProgress.total_files > 0 && (
        // 진행 수치는 이벤트마다 바뀐다 — 낭독기가 매번 읽지 않게 알림 영역에서 뺀다
        <p className="mt-2 text-sm tabular-nums" style={{ color: "var(--color-text-muted)" }} aria-live="off">
          {indexProgress.processed_files.toLocaleString()} / {indexProgress.total_files.toLocaleString()}개 읽음
        </p>
      )}
    </EmptyState>
  );
}

/** 색인된 문서가 0개 — "결과 없음"이 아니라 "아직 검색할 문서 없음"을 안내
 *  (신규 사용자가 폴더 추가 전 검색했을 때 "다른 검색어를 쓰세요"라는 오해 유발을 방지) */
export function NoIndexState({ onAddFolder }: { onAddFolder?: () => void }) {
  return (
    <EmptyState icon={FolderPlus} title="아직 검색할 문서가 없습니다">
      <p className="mb-6" style={{ color: "var(--color-text-muted)" }}>
        폴더를 추가하면 안의 문서를 읽어서 검색할 수 있어요
      </p>
      {onAddFolder && (
        <button
          onClick={onAddFolder}
          className="inline-flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium transition-colors active:scale-[0.98]"
          style={{ backgroundColor: "var(--color-accent)", color: "var(--color-on-accent)" }}
        >
          <FolderPlus className="w-4 h-4" aria-hidden="true" />
          폴더 추가하여 시작하기
        </button>
      )}
    </EmptyState>
  );
}

/** 결과는 있는데 필터(파일 형식·기간·결과 내 검색·최소 신뢰도)에 전부 가려진 상태.
 *  종전엔 "결과를 찾을 수 없습니다"로 떠서 검색어 탓으로 오해했다 */
export function FilteredOutState({
  query,
  hiddenCount,
  minConfidence = 0,
  onClearFilters,
}: {
  query: string;
  hiddenCount: number;
  minConfidence?: number;
  onClearFilters?: () => void;
}) {
  return (
    <EmptyState icon={Filter} title="조건에 맞는 결과가 없습니다">
      <p className="mb-6" style={{ color: "var(--color-text-muted)" }}>
        <QuotedQuery query={query} /> 결과 {hiddenCount.toLocaleString()}개가 필터에 가려져 있어요
      </p>
      {onClearFilters && (
        <button
          onClick={onClearFilters}
          className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-full text-xs border btn-outline-accent-hover"
        >
          <Filter className="w-3.5 h-3.5" aria-hidden="true" />
          필터 풀고 모두 보기
        </button>
      )}
      {minConfidence > 0 && (
        <p className="mt-4 text-xs" style={{ color: "var(--color-text-muted)" }}>
          설정 &gt; 검색의 최소 신뢰도({minConfidence}%)보다 낮은 결과도 가려집니다
        </p>
      )}
    </EmptyState>
  );
}

/** 검색어가 있지만 결과 없음 — 맥락 있는 피드백과 다음 행동 제안 */
export function NoResultsState({
  query,
  paradigm,
  parsedQuery,
  searchMode,
  onRetryWithoutFilters,
  onFocusSearch,
  onSwitchToFilenameSearch,
}: {
  query: string;
  paradigm: "instant" | "natural";
  parsedQuery?: ParsedQueryInfo | null;
  searchMode?: string;
  onRetryWithoutFilters?: () => void;
  onFocusSearch?: () => void;
  onSwitchToFilenameSearch?: () => void;
}) {
  // 자연어 모드: 파싱 결과 표시로 왜 결과가 없는지 힌트 제공
  const hasNlFilters = parsedQuery && (parsedQuery.date_filter || parsedQuery.file_type || parsedQuery.exclude_keywords.length > 0);

  return (
    <EmptyState icon={Frown} title="결과를 찾을 수 없습니다">
      <p className="mb-6" style={{ color: "var(--color-text-muted)" }}>
        <QuotedQuery query={query} />에 대한 결과가 없습니다
      </p>

      {/* 자연어 모드: 파싱 결과 칩 표시 */}
      {paradigm === "natural" && parsedQuery && parsedQuery.parse_log.length > 0 && (
        <div className="mb-6">
          <p className="text-xs mb-2" style={{ color: "var(--color-text-muted)" }}>분석된 검색 조건</p>
          <div className="flex flex-wrap justify-center gap-1.5">
            {parsedQuery.parse_log.map((log, i) => (
              <span
                key={i}
                className="inline-flex items-center px-2.5 py-1 rounded-full text-xs font-medium"
                style={{ backgroundColor: "var(--color-accent-light)", color: "var(--color-accent)", border: "1px solid var(--color-accent-border)" }}
              >
                {log}
              </span>
            ))}
          </div>
        </div>
      )}

      <div className="space-y-2 text-sm" style={{ color: "var(--color-text-muted)" }}>
        <p>이렇게 해 보세요</p>
        <div className="flex flex-wrap justify-center gap-2 mt-3">
          {hasNlFilters && onRetryWithoutFilters && (
            <button
              onClick={onRetryWithoutFilters}
              className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-full text-xs border btn-outline-accent-hover"
            >
              <Filter className="w-3.5 h-3.5" aria-hidden="true" />
              날짜·파일 형식 조건 빼고 검색
            </button>
          )}
          {onFocusSearch && (
            <button
              onClick={onFocusSearch}
              className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-full text-xs border btn-outline-accent-hover"
            >
              <PenLine className="w-3.5 h-3.5" aria-hidden="true" />
              다른 검색어 입력
            </button>
          )}
          {paradigm === "instant" && searchMode !== "filename" && onSwitchToFilenameSearch && (
            <button
              onClick={onSwitchToFilenameSearch}
              className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-full text-xs border btn-outline-accent-hover"
            >
              <ArrowLeftRight className="w-3.5 h-3.5" aria-hidden="true" />
              파일명으로 검색
            </button>
          )}
        </div>
      </div>
    </EmptyState>
  );
}

const SMART_GUIDE_CATEGORIES: { label: string; Icon: LucideIcon; examples: string[] }[] = [
  { label: "파일 형식", Icon: FileText, examples: ["예산 한글 문서", "계약서 PDF만", "매출 엑셀 파일"] },
  { label: "날짜 범위", Icon: CalendarDays, examples: ["최근 30일 보고서", "작년 인사발령", "3월 회의록"] },
  { label: "제외 검색", Icon: Ban, examples: ["계약서 초안 제외", "인사 관련 공지 빼고", "임시파일 아닌 것만"] },
  { label: "조건 조합", Icon: Layers, examples: ["올해 예산 엑셀 파일", "최근 7일 계약서 PDF", "작년 보고서 한글문서"] },
];

/** 스마트 검색 모드 초기 안내 화면. 예시를 누르면 그대로 검색한다 */
export function SmartSearchGuide({ onSelectExample }: { onSelectExample?: (text: string) => void }) {
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
              <cat.Icon className="w-4 h-4" style={{ color: "var(--color-accent)" }} aria-hidden="true" />
              <span className="text-xs font-semibold text-[var(--color-text-secondary)]">
                {cat.label}
              </span>
            </div>
            <ul className="space-y-1">
              {cat.examples.map((ex) => (
                <li key={ex}>
                  {onSelectExample ? (
                    <button
                      type="button"
                      onClick={() => onSelectExample(ex)}
                      className="w-full text-left text-sm leading-snug px-1.5 py-0.5 -mx-1.5 rounded text-[var(--color-text-muted)] hover:text-[var(--color-accent)] hover:bg-[var(--color-bg-tertiary)] transition-colors"
                    >
                      "{ex}"
                    </button>
                  ) : (
                    <span className="text-sm leading-snug text-[var(--color-text-muted)]">"{ex}"</span>
                  )}
                </li>
              ))}
            </ul>
          </div>
        ))}
      </div>

      {/* 하단 안내 */}
      <div className="mt-auto pb-4 flex items-center justify-center gap-4 text-xs text-[var(--color-text-tertiary)]">
        <span>{onSelectExample ? "예시를 누르면 검색창에 들어가요 · Enter로 검색" : "Enter로 검색 · 키워드 + 조건을 자연스럽게 입력하세요"}</span>
      </div>
    </div>
  );
}
