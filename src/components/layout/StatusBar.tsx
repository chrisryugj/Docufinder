import { memo } from "react";
import type { IndexStatus, IndexingProgress, VectorIndexingStatus } from "../../types/index";
import { cleanPath } from "../../utils/cleanPath";

interface StatusBarProps {
  status: IndexStatus | null;
  progress: IndexingProgress | null;
  vectorStatus: VectorIndexingStatus | null;
  onCancelIndexing?: () => void;
  onCancelVectorIndexing?: () => void;
  onStartVectorIndexing?: () => void;
  semanticEnabled?: boolean;
}

const phaseLabels: Record<string, string> = {
  preparing: "폴더 분석 준비 중",
  scanning: "파일 검색 중",
  parsing: "파일 분석 중",
  indexing: "인덱싱 중",
  completed: "완료",
  cancelled: "취소됨",
};

export const StatusBar = memo(function StatusBar({ status, progress, vectorStatus, onCancelIndexing, onCancelVectorIndexing, onStartVectorIndexing, semanticEnabled }: StatusBarProps) {
  const isIndexing = progress && progress.phase !== "completed" && progress.phase !== "cancelled";
  const isVectorIndexing = vectorStatus && vectorStatus.is_running && vectorStatus.total_chunks > 0;
  const hasPendingVectors = (vectorStatus?.pending_chunks ?? 0) > 0;
  const isVectorComplete = vectorStatus && !vectorStatus.is_running && !hasPendingVectors;
  const percent = progress && progress.total_files > 0
    ? Math.round((progress.processed_files / progress.total_files) * 100)
    : 0;
  const vectorPercent = vectorStatus && vectorStatus.total_chunks > 0
    ? Math.round((vectorStatus.processed_chunks / vectorStatus.total_chunks) * 100)
    : 0;

  return (
    <footer
      className="px-4 py-2.5 border-t"
      style={{
        backgroundColor: "var(--color-bg-secondary)",
        borderColor: "var(--color-border)",
      }}
    >
      {isIndexing ? (
        <div className="space-y-1.5">
          {/* 진행률 정보 */}
          <div className="flex items-center justify-between text-sm">
            <div className="flex items-center gap-2">
              <span className="animate-pulse" style={{ color: "var(--color-primary)" }}>●</span>
              <span style={{ color: "var(--color-text-secondary)" }}>
                {phaseLabels[progress.phase] || progress.phase}
              </span>
              {progress.phase !== "preparing" && (
                <span style={{ color: "var(--color-text-muted)" }}>
                  {progress.processed_files} / {progress.total_files}
                </span>
              )}
            </div>
            <div className="flex items-center gap-3">
              {progress.phase !== "preparing" && (
                <span style={{ color: "var(--color-text-muted)" }}>{percent}%</span>
              )}
              {onCancelIndexing && (
                <button
                  onClick={onCancelIndexing}
                  className="px-3 py-1 text-xs rounded btn-cancel-hover"
                >
                  취소
                </button>
              )}
            </div>
          </div>

          {/* 진행률 바 */}
          <div
            className="h-1.5 rounded-full overflow-hidden"
            style={{ backgroundColor: "var(--color-bg-tertiary)" }}
          >
            {progress.phase === "preparing" ? (
              <div
                className="h-full w-1/3 rounded-full animate-[indeterminate_1.5s_ease-in-out_infinite]"
                style={{ backgroundColor: "var(--color-primary)" }}
              />
            ) : (
              <div
                className="h-full transition-all duration-300"
                style={{
                  width: `${percent}%`,
                  backgroundColor: "var(--color-primary)",
                }}
              />
            )}
          </div>

          {/* 현재 파일명 */}
          {progress.current_file && (
            <div
              className="text-xs truncate"
              style={{ color: "var(--color-text-muted)" }}
              title={cleanPath(progress.current_file)}
            >
              {cleanPath(progress.current_file)}
            </div>
          )}
        </div>
      ) : isVectorIndexing ? (
        <div className="space-y-1.5">
          {/* 벡터 인덱싱 진행률 */}
          <div className="flex items-center justify-between text-sm">
            <div className="flex items-center gap-2">
              <span className="animate-pulse" style={{ color: "var(--color-accent)" }}>●</span>
              <span style={{ color: "var(--color-text-secondary)" }}>
                시맨틱 인덱싱 중
              </span>
              <span style={{ color: "var(--color-text-muted)" }}>
                {vectorStatus.processed_chunks} / {vectorStatus.total_chunks}
              </span>
            </div>
            <div className="flex items-center gap-3">
              <span style={{ color: "var(--color-text-muted)" }}>{vectorPercent}%</span>
              {onCancelVectorIndexing && (
                <button
                  onClick={onCancelVectorIndexing}
                  className="px-3 py-1 text-xs rounded btn-cancel-hover"
                >
                  취소
                </button>
              )}
            </div>
          </div>

          {/* 진행률 바 */}
          <div
            className="h-1.5 rounded-full overflow-hidden"
            style={{ backgroundColor: "var(--color-bg-tertiary)" }}
          >
            <div
              className="h-full transition-all duration-300"
              style={{
                width: `${vectorPercent}%`,
                backgroundColor: "var(--color-accent)",
              }}
            />
          </div>

          {/* 현재 파일명 */}
          {vectorStatus.current_file && (
            <div
              className="text-xs truncate"
              style={{ color: "var(--color-text-muted)" }}
              title={cleanPath(vectorStatus.current_file)}
            >
              {cleanPath(vectorStatus.current_file)}
            </div>
          )}
        </div>
      ) : (
        <div
          className="flex justify-between text-sm"
          style={{ color: "var(--color-text-muted)" }}
        >
          <div className="flex items-center gap-2">
            <span>
              <span style={{ color: "var(--color-text-secondary)" }}>
                {status?.indexed_files ?? 0}
              </span>
              {" 문서"}
              {status && status.total_files > status.indexed_files && (
                <span style={{ color: "var(--color-text-muted)" }}>
                  {" / "}{status.total_files} 파일
                </span>
              )}
            </span>
            {/* 시맨틱 분석 대기 상태 표시 */}
            {semanticEnabled && hasPendingVectors && !isVectorIndexing && (
              <span style={{ color: "var(--color-text-muted)" }} title="AI가 문서 내용을 분석하여 의미 기반 검색을 준비합니다">
                | 시맨틱 대기:{" "}
                <span style={{ color: "var(--color-accent)" }}>
                  {vectorStatus?.pending_chunks ?? 0}
                </span>
              </span>
            )}
            {/* 시맨틱 완료 상태 */}
            {semanticEnabled && isVectorComplete && (status?.vectors_count ?? 0) > 0 && (
              <span style={{ color: "var(--color-text-muted)" }} title="시맨틱 검색: 키워드가 정확히 일치하지 않아도 의미가 비슷한 문서를 찾아줍니다">
                | 시맨틱:{" "}
                <span style={{ color: "var(--color-success, #22c55e)" }}>✓</span>
              </span>
            )}
          </div>
          <div className="flex items-center gap-3">
            <span>
              {status?.watched_folders.length ? (
                <>
                  폴더:{" "}
                  <span style={{ color: "var(--color-text-secondary)" }}>
                    {status.watched_folders.length}개
                  </span>
                </>
              ) : (
                "폴더를 추가하세요"
              )}
            </span>
            {semanticEnabled && onStartVectorIndexing && !isVectorIndexing && hasPendingVectors && (
              <button
                onClick={onStartVectorIndexing}
                className="px-2 py-0.5 text-xs rounded btn-accent-start-hover"
                title="벡터 인덱싱을 시작합니다. 하이브리드/의미 검색에 필요합니다."
              >
                시맨틱 시작
              </button>
            )}
          </div>
        </div>
      )}
    </footer>
  );
});
