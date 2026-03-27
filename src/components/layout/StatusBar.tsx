import { memo, useState, useEffect } from "react";
import { getVersion } from "@tauri-apps/api/app";
import type { IndexStatus, IndexingProgress, VectorIndexingStatus } from "../../types/index";
import { cleanPath } from "../../utils/cleanPath";

interface StatusBarProps {
  status: IndexStatus | null;
  progress: IndexingProgress | null;
  vectorStatus: VectorIndexingStatus | null;
  onCancelIndexing?: () => void;
  onCancelVectorIndexing?: () => void;
  onStartVectorIndexing?: () => void;
  onResumeIndexing?: () => void;
  semanticEnabled?: boolean;
  hasCancelledFolders?: boolean;
}

const phaseLabels: Record<string, string> = {
  preparing: "폴더 분석 준비 중",
  scanning: "파일 검색 중",
  parsing: "파일 분석 중",
  indexing: "인덱싱 중",
  completed: "완료",
  cancelled: "취소됨",
};

export const StatusBar = memo(function StatusBar({ status, progress, vectorStatus, onCancelIndexing, onCancelVectorIndexing, onStartVectorIndexing, onResumeIndexing, semanticEnabled, hasCancelledFolders }: StatusBarProps) {
  const [appVersion, setAppVersion] = useState("");
  useEffect(() => { getVersion().then(setAppVersion).catch(() => {}); }, []);

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
      className="px-3 py-2 border-t"
      style={{
        backgroundColor: "var(--color-bg-secondary)",
        borderColor: "var(--color-border)",
        height: "50px",
        minHeight: "50px",
        display: "flex",
        flexDirection: "column",
        justifyContent: "center",
      }}
    >
      {isIndexing ? (
        <div className="space-y-1">
          {/* 진행률 정보 */}
          <div className="flex items-center justify-between text-xs">
            <div className="flex items-center gap-1.5">
              <span className="w-1.5 h-1.5 rounded-full animate-pulse" style={{ backgroundColor: "var(--color-accent)" }} />
              <span style={{ color: "var(--color-text-secondary)" }}>
                {phaseLabels[progress.phase] || progress.phase}
              </span>
              {progress.phase !== "preparing" && (
                <span style={{ color: "var(--color-text-muted)" }}>
                  {progress.processed_files}/{progress.total_files}
                </span>
              )}
            </div>
            <div className="flex items-center gap-2">
              {progress.phase !== "preparing" && (
                <span className="font-medium tabular-nums" style={{ color: "var(--color-text-muted)" }}>{percent}%</span>
              )}
              {onCancelIndexing && (
                <button
                  onClick={onCancelIndexing}
                  className="px-2 py-0.5 text-[11px] rounded btn-cancel-hover"
                >
                  취소
                </button>
              )}
            </div>
          </div>

          {/* 진행률 바 */}
          <div
            className="h-1 rounded-full overflow-hidden"
            style={{ backgroundColor: "var(--color-bg-tertiary)" }}
          >
            {progress.phase === "preparing" ? (
              <div
                className="h-full w-1/3 rounded-full animate-[indeterminate_1.5s_ease-in-out_infinite]"
                style={{ backgroundColor: "var(--color-accent)" }}
              />
            ) : (
              <div
                className="h-full rounded-full transition-all duration-300"
                style={{
                  width: `${percent}%`,
                  backgroundColor: "var(--color-accent)",
                }}
              />
            )}
          </div>

          {/* 현재 파일명 */}
          <div
            className="text-[11px] truncate h-4"
            style={{ color: "var(--color-text-muted)" }}
            title={progress.current_file ? cleanPath(progress.current_file) : ""}
          >
            {progress.current_file ? cleanPath(progress.current_file) : ""}
          </div>
        </div>
      ) : isVectorIndexing ? (
        <div className="space-y-1">
          {/* 벡터 인덱싱 진행률 */}
          <div className="flex items-center justify-between text-xs">
            <div className="flex items-center gap-1.5">
              <span className="w-1.5 h-1.5 rounded-full animate-pulse" style={{ backgroundColor: "var(--color-accent)" }} />
              <span style={{ color: "var(--color-text-secondary)" }}>
                시맨틱 인덱싱
              </span>
              <span style={{ color: "var(--color-text-muted)" }}>
                {vectorStatus.processed_chunks}/{vectorStatus.total_chunks}
              </span>
            </div>
            <div className="flex items-center gap-2">
              <span className="font-medium tabular-nums" style={{ color: "var(--color-text-muted)" }}>{vectorPercent}%</span>
              {onCancelVectorIndexing && (
                <button
                  onClick={onCancelVectorIndexing}
                  className="px-2 py-0.5 text-[11px] rounded btn-cancel-hover"
                >
                  취소
                </button>
              )}
            </div>
          </div>

          {/* 진행률 바 */}
          <div
            className="h-1 rounded-full overflow-hidden"
            style={{ backgroundColor: "var(--color-bg-tertiary)" }}
          >
            <div
              className="h-full rounded-full transition-all duration-300"
              style={{
                width: `${vectorPercent}%`,
                backgroundColor: "var(--color-accent)",
              }}
            />
          </div>

          {/* 현재 파일명 */}
          <div
            className="text-[11px] truncate h-4"
            style={{ color: "var(--color-text-muted)" }}
            title={vectorStatus.current_file ? cleanPath(vectorStatus.current_file) : ""}
          >
            {vectorStatus.current_file ? cleanPath(vectorStatus.current_file) : ""}
          </div>
        </div>
      ) : (
        <div
          className="flex justify-between text-xs"
          style={{ color: "var(--color-text-muted)" }}
        >
          <div className="flex items-center gap-1.5">
            <span>
              <span className="font-medium" style={{ color: "var(--color-text-secondary)" }}>
                {status?.indexed_files ?? 0}
              </span>
              {" 문서"}
              {status && status.total_files > status.indexed_files && (
                <span>
                  {" / "}{status.total_files}
                </span>
              )}
            </span>
            {semanticEnabled && hasPendingVectors && !isVectorIndexing && (
              <span title="AI가 문서 내용을 분석하여 의미 기반 검색을 준비합니다">
                · 시맨틱 대기{" "}
                <span style={{ color: "var(--color-accent)" }}>
                  {vectorStatus?.pending_chunks ?? 0}
                </span>
              </span>
            )}
            {semanticEnabled && isVectorComplete && (status?.vectors_count ?? 0) > 0 && (
              <span title="시맨틱 검색 활성화됨">
                · 시맨틱{" "}
                <span style={{ color: "var(--color-success, #22c55e)" }}>✓</span>
              </span>
            )}
            {status?.filename_cache_truncated && (
              <span
                title="파일 수가 캐시 상한(100만개)을 초과했습니다. 일부 파일명 검색 결과가 누락될 수 있습니다."
                style={{ color: "var(--color-warning, #f59e0b)" }}
              >
                · 파일명 캐시 초과
              </span>
            )}
          </div>
          <div className="flex items-center gap-2">
            <span>
              {status?.watched_folders.length ? (
                <>
                  <span className="font-medium" style={{ color: "var(--color-text-secondary)" }}>
                    {status.watched_folders.length}
                  </span>
                  {" 폴더"}
                </>
              ) : (
                "폴더를 추가하세요"
              )}
            </span>
            {hasCancelledFolders && onResumeIndexing && !isIndexing && (
              <button
                onClick={onResumeIndexing}
                className="px-1.5 py-0.5 text-[11px] rounded btn-accent-start-hover font-medium"
                title="취소된 인덱싱을 다시 시작합니다"
              >
                재시작
              </button>
            )}
            {semanticEnabled && onStartVectorIndexing && !isVectorIndexing && hasPendingVectors && (
              <button
                onClick={onStartVectorIndexing}
                className="px-1.5 py-0.5 text-[11px] rounded btn-accent-start-hover font-medium"
                title="벡터 인덱싱을 시작합니다"
              >
                시맨틱 시작
              </button>
            )}
            {appVersion && (
              <span className="text-[10px] opacity-50">v{appVersion}</span>
            )}
          </div>
        </div>
      )}
    </footer>
  );
});
