import { memo, useState, useEffect } from "react";
import { getVersion } from "@tauri-apps/api/app";
import type { IndexStatus, IndexingProgress, BatchState } from "../../types/index";
import { cleanPath } from "../../utils/cleanPath";

interface StatusBarProps {
  status: IndexStatus | null;
  progress: IndexingProgress | null;
  batch?: BatchState | null;
  onCancelIndexing?: () => void;
  onCancelBatch?: () => void;
  onResumeIndexing?: () => void;
  hasCancelledFolders?: boolean;
}

const phaseInfo: Record<string, { label: string; desc: string }> = {
  preparing: { label: "준비 중", desc: "폴더 구조를 분석하고 있습니다" },
  scanning: { label: "파일 검색", desc: "인덱싱할 파일을 찾고 있습니다" },
  parsing: { label: "파일 분석", desc: "문서 내용을 읽고 있습니다" },
  indexing: { label: "인덱싱", desc: "검색 인덱스를 생성하고 있습니다" },
  completed: { label: "완료", desc: "" },
  cancelled: { label: "취소됨", desc: "" },
};

export const StatusBar = memo(function StatusBar({ status, progress, batch, onCancelIndexing, onCancelBatch, onResumeIndexing, hasCancelledFolders }: StatusBarProps) {
  const [appVersion, setAppVersion] = useState("");
  useEffect(() => { getVersion().then(setAppVersion).catch(() => {}); }, []);

  const isBatchRunning = batch?.is_running ?? false;
  const isIndexing = !isBatchRunning && progress && progress.phase !== "completed" && progress.phase !== "cancelled";
  const percent = progress && progress.total_files > 0
    ? Math.round((progress.processed_files / progress.total_files) * 100)
    : 0;

  // 배치 요약 계산
  const batchSummary = batch ? (() => {
    const total = batch.jobs.length;
    const done = batch.jobs.filter(j => j.status === "done" || j.status === "failed" || j.status === "cancelled").length;
    const p = total > 0 ? Math.round((done / total) * 100) : 0;
    const current = batch.jobs[batch.current_index];
    return { total, done, percent: p, current };
  })() : null;

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
      {isBatchRunning && batchSummary ? (
        <div className="space-y-1.5">
          <div className="flex items-center gap-2 text-sm min-w-0">
            <span className="w-2 h-2 shrink-0 rounded-full animate-pulse" style={{ backgroundColor: "var(--color-accent)" }} />
            <span className="shrink-0 font-medium" style={{ color: "var(--color-text-primary)" }}>
              드라이브 인덱싱
            </span>
            <span className="shrink-0 tabular-nums text-xs" style={{ color: "var(--color-text-muted)" }}>
              {batchSummary.done}/{batchSummary.total} 드라이브
            </span>
            {batchSummary.current && batchSummary.current.status === "running" && (
              <span
                className="truncate text-xs min-w-0"
                style={{ color: "var(--color-text-muted)" }}
                title={batchSummary.current.path}
              >
                · {batchSummary.current.path.replace(/^\\\\\?\\/, "")}
              </span>
            )}
            <div className="flex items-center gap-2 ml-auto shrink-0">
              <span className="font-semibold tabular-nums" style={{ color: "var(--color-accent)" }}>{batchSummary.percent}%</span>
              {onCancelBatch && (
                <button onClick={onCancelBatch} className="px-2 py-0.5 text-[11px] rounded btn-cancel-hover">
                  취소
                </button>
              )}
            </div>
          </div>
          <div className="h-1.5 rounded-full overflow-hidden" style={{ backgroundColor: "var(--color-bg-tertiary)" }}>
            <div
              className="h-full rounded-full transition-all duration-300"
              style={{ width: `${batchSummary.percent}%`, backgroundColor: "var(--color-accent)" }}
            />
          </div>
        </div>
      ) : isIndexing ? (
        <div className="space-y-1.5">
          {/* 진행률 정보 */}
          <div className="flex items-center gap-2 text-sm min-w-0">
            <span className="w-2 h-2 shrink-0 rounded-full animate-pulse" style={{ backgroundColor: "var(--color-accent)" }} />
            <span className="shrink-0 font-medium" style={{ color: "var(--color-text-primary)" }}>
              {(phaseInfo[progress.phase] || { label: progress.phase }).label}
            </span>
            {progress.phase !== "preparing" && (
              <span className="shrink-0 tabular-nums text-xs" style={{ color: "var(--color-text-muted)" }}>
                {progress.processed_files}/{progress.total_files}
              </span>
            )}
            {progress.current_file && (
              <span
                className="truncate text-xs min-w-0"
                style={{ color: "var(--color-text-muted)" }}
                title={cleanPath(progress.current_file)}
              >
                · {progress.current_file.replace(/^.*[\\/]/, "")}
              </span>
            )}
            <div className="flex items-center gap-2 ml-auto shrink-0">
              {progress.phase !== "preparing" && (
                <span className="font-semibold tabular-nums" style={{ color: "var(--color-accent)" }}>{percent}%</span>
              )}
              {onCancelIndexing && (
                <button onClick={onCancelIndexing} className="px-2 py-0.5 text-[11px] rounded btn-cancel-hover">
                  취소
                </button>
              )}
            </div>
          </div>

          {/* 진행률 바 */}
          <div className="h-1.5 rounded-full overflow-hidden" style={{ backgroundColor: "var(--color-bg-tertiary)" }}>
            {progress.phase === "preparing" ? (
              <div
                className="h-full w-1/3 rounded-full animate-[indeterminate_1.5s_ease-in-out_infinite]"
                style={{ backgroundColor: "var(--color-accent)" }}
              />
            ) : (
              <div
                className="h-full rounded-full transition-all duration-300"
                style={{ width: `${percent}%`, backgroundColor: "var(--color-accent)" }}
              />
            )}
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
            {appVersion && (
              <span className="text-[10px] opacity-50">v{appVersion}</span>
            )}
          </div>
        </div>
      )}
    </footer>
  );
});
