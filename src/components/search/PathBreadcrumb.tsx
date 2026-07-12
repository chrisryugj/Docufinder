import { formatPathSegments, formatPathTail } from "../../utils/searchTextUtils";

interface PathBreadcrumbProps {
  /** 파일 전체 경로 — 컨테이너 title + 파일 위치 열기(reveal) 대상 */
  filePath: string;
  /** 폴더 경로 (파일명 제거본) */
  folderPath: string;
  /** 경로/세그먼트 클릭 → 탐색기 열기. 파일 경로를 넘기면 백엔드가 파일 선택(reveal) */
  onOpenFolder?: (path: string) => void;
  /** 컴팩트: 꼬리 우선 축약 한 줄 버튼 (클릭 = 파일 위치 열기) */
  compact?: boolean;
  /** 마지막(파일이 든) 폴더 세그먼트 클릭을 '파일 위치 열기(탐색기에서 파일 선택)'로 */
  revealLast?: boolean;
}

/**
 * 검색 결과 저장 위치 표기 — SearchResultItem·GroupedSearchResultItem·파일명 매치 공용.
 * 기본: 세그먼트별 클릭 가능한 브레드크럼 (중간 경로도 해당 폴더 열기).
 * compact: 말단 폴더가 보이는 꼬리 우선 축약 한 줄.
 */
export function PathBreadcrumb({
  filePath,
  folderPath,
  onOpenFolder,
  compact = false,
  revealLast = false,
}: PathBreadcrumbProps) {
  const cleanFull = filePath.replace(/^\\\\\?\\/, "");

  if (compact) {
    return (
      <button
        onClick={(e) => { e.stopPropagation(); onOpenFolder?.(filePath); }}
        className="text-xs truncate flex-1 min-w-0 text-left px-0.5 py-0.5 rounded transition-colors hover:underline clr-muted hover-accent-text"
        title={`${cleanFull}\n클릭: 파일 위치 열기`}
      >
        {formatPathTail(folderPath)}
      </button>
    );
  }

  return (
    <div className="flex flex-wrap items-center gap-0.5 flex-1 min-w-0" title={cleanFull}>
      {formatPathSegments(folderPath).map((seg, i, arr) => {
        const reveal = revealLast && i === arr.length - 1;
        return (
          <div key={i} className="flex items-center leading-none">
            {seg.fullPath ? (
              <button
                onClick={(e) => { e.stopPropagation(); onOpenFolder?.(reveal ? filePath : seg.fullPath); }}
                className="text-xs px-0.5 py-0.5 rounded transition-colors hover:underline clr-muted hover-accent-text"
                title={reveal ? "파일 위치 열기 (탐색기에서 파일 선택)" : `${seg.fullPath} 열기`}
              >
                {seg.label}
              </button>
            ) : (
              <span className="text-xs px-0.5 py-0.5" style={{ color: "var(--color-text-muted)" }}>
                {seg.label}
              </span>
            )}
            {i < arr.length - 1 && (
              <span className="text-[11px] mx-px" style={{ color: "var(--color-text-tertiary)" }}>/</span>
            )}
          </div>
        );
      })}
    </div>
  );
}
