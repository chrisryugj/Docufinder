import { memo, useMemo } from "react";
import { Check, X, Loader2, Clock, AlertTriangle, Ban } from "lucide-react";
import type { BatchJob, BatchState, BatchJobStatus } from "../../types/index";

interface DriveIndexingPanelProps {
  batch: BatchState;
  onCancel: () => void;
  onDismiss: () => void;
}

const STAGE_LABEL: Record<string, string> = {
  scanning: "파일 검색 중",
  parsing: "문서 분석 중",
  indexing: "인덱스 저장 중",
  fts_commit: "DB 저장 중",
  wal_checkpoint: "캐시 정리 중",
  cache_refresh: "파일 목록 갱신 중",
};

function pathLabel(path: string): string {
  const cleaned = path.replace(/^\\\\\?\\/, "");
  // 드라이브 루트(C:\)는 그대로, 하위 폴더는 마지막 segment
  if (/^[A-Za-z]:\\?$/.test(cleaned)) return cleaned;
  const parts = cleaned.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? cleaned;
}

function statusIcon(status: BatchJobStatus) {
  switch (status) {
    case "pending":
      return <Clock className="w-3.5 h-3.5 shrink-0" style={{ color: "var(--color-text-muted)" }} />;
    case "running":
    case "committing":
      return (
        <Loader2
          className="w-3.5 h-3.5 shrink-0 animate-spin"
          style={{ color: "var(--color-accent)" }}
        />
      );
    case "done":
      return <Check className="w-3.5 h-3.5 shrink-0" style={{ color: "#10b981" }} />;
    case "failed":
      return <AlertTriangle className="w-3.5 h-3.5 shrink-0" style={{ color: "#ef4444" }} />;
    case "cancelled":
      return <Ban className="w-3.5 h-3.5 shrink-0" style={{ color: "var(--color-text-muted)" }} />;
  }
}

function formatDuration(startedAt: number | null, finishedAt: number | null): string {
  if (!startedAt) return "";
  const end = finishedAt ?? Math.floor(Date.now() / 1000);
  const sec = Math.max(0, end - startedAt);
  if (sec < 60) return `${sec}s`;
  const m = Math.floor(sec / 60);
  const s = sec % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}

const JobRow = memo(function JobRow({ job }: { job: BatchJob }) {
  const percent =
    job.total > 0 ? Math.min(100, Math.round((job.processed / job.total) * 100)) : 0;
  const isActive = job.status === "running" || job.status === "committing";
  const stageLabel = job.stage ? STAGE_LABEL[job.stage] ?? job.stage : null;

  return (
    <div className="px-2 py-1.5">
      <div className="flex items-center gap-2 text-[12px] min-w-0">
        {statusIcon(job.status)}
        <span
          className="font-medium shrink-0 tabular-nums"
          style={{ color: "var(--color-text-primary)" }}
        >
          {pathLabel(job.path)}
        </span>
        {job.status === "done" && (
          <span className="text-[11px] tabular-nums" style={{ color: "var(--color-text-muted)" }}>
            {job.indexed_count.toLocaleString()}개
          </span>
        )}
        {isActive && stageLabel && (
          <span
            className="text-[11px] truncate min-w-0"
            style={{ color: "var(--color-text-muted)" }}
          >
            · {stageLabel}
          </span>
        )}
        <div className="flex items-center gap-1.5 ml-auto shrink-0">
          {isActive && job.total > 0 && job.status === "running" && (
            <span
              className="text-[11px] tabular-nums font-semibold"
              style={{ color: "var(--color-accent)" }}
            >
              {percent}%
            </span>
          )}
          {(job.status === "done" ||
            job.status === "failed" ||
            job.status === "cancelled") &&
            job.started_at && (
              <span
                className="text-[10px] tabular-nums"
                style={{ color: "var(--color-text-muted)" }}
              >
                {formatDuration(job.started_at, job.finished_at)}
              </span>
            )}
        </div>
      </div>

      {/* 진행률 바 (running 상태만) */}
      {isActive && (
        <div
          className="mt-1 h-1 rounded-full overflow-hidden"
          style={{ backgroundColor: "var(--color-bg-tertiary)" }}
        >
          {job.status === "committing" || percent === 0 ? (
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
      )}

      {/* 현재 파일명 (running 상태) — 파일 전환 순간 current_file이 비어도 높이 유지 */}
      {job.status === "running" && (
        <div
          className="mt-0.5 text-[10px] truncate leading-[14px]"
          style={{ color: "var(--color-text-muted)", minHeight: "14px" }}
          title={job.current_file ?? undefined}
        >
          {job.current_file ? job.current_file.replace(/^.*[\\/]/, "") : "\u00A0"}
        </div>
      )}

      {/* 에러 메시지 */}
      {job.status === "failed" && job.error && (
        <div
          className="mt-0.5 text-[10px] truncate"
          style={{ color: "#ef4444" }}
          title={job.error}
        >
          {job.error}
        </div>
      )}
    </div>
  );
});

export const DriveIndexingPanel = memo(function DriveIndexingPanel({
  batch,
  onCancel,
  onDismiss,
}: DriveIndexingPanelProps) {
  const summary = useMemo(() => {
    const total = batch.jobs.length;
    const done = batch.jobs.filter(
      (j) => j.status === "done" || j.status === "failed" || j.status === "cancelled",
    ).length;
    const totalIndexed = batch.jobs.reduce((acc, j) => acc + j.indexed_count, 0);

    // 전체 % = (완료 job 수 + 현재 active job의 내부 진행률) / 전체 job 수
    // → 한 폴더가 끝나기 전에도 부드럽게 증가
    const activeJob = batch.jobs.find(
      (j) => j.status === "running" || j.status === "committing",
    );
    const activeFraction =
      activeJob && activeJob.total > 0
        ? Math.min(1, activeJob.processed / activeJob.total)
        : 0;
    const percent =
      total > 0 ? Math.min(100, Math.round(((done + activeFraction) / total) * 100)) : 0;

    return { total, done, totalIndexed, percent };
  }, [batch.jobs]);

  return (
    <div
      className="rounded-lg mb-3 overflow-hidden"
      style={{
        backgroundColor: "var(--color-bg-secondary)",
        border: "1px solid var(--color-border)",
      }}
      data-tour="drive-indexing-panel"
    >
      {/* 헤더 */}
      <div
        className="px-3 py-2 flex items-center justify-between gap-2"
        style={{ borderBottom: "1px solid var(--color-border)" }}
      >
        <div className="flex items-center gap-2 min-w-0">
          {batch.is_running ? (
            <Loader2
              className="w-3.5 h-3.5 animate-spin shrink-0"
              style={{ color: "var(--color-accent)" }}
            />
          ) : (
            <Check className="w-3.5 h-3.5 shrink-0" style={{ color: "#10b981" }} />
          )}
          <span
            className="text-[11px] font-bold tracking-[0.08em] uppercase"
            style={{ color: "var(--color-sidebar-section)" }}
          >
            드라이브 인덱싱
          </span>
          <span
            className="text-[11px] tabular-nums"
            style={{ color: "var(--color-text-muted)" }}
          >
            {summary.done}/{summary.total} · {summary.percent}%
          </span>
        </div>
        {batch.is_running ? (
          <button
            onClick={onCancel}
            className="text-[10px] px-1.5 py-0.5 rounded btn-cancel-hover"
          >
            취소
          </button>
        ) : (
          <button
            onClick={onDismiss}
            className="p-1 rounded btn-icon-hover"
            aria-label="패널 닫기"
          >
            <X className="w-3 h-3" style={{ color: "var(--color-text-muted)" }} />
          </button>
        )}
      </div>

      {/* 전체 진행률 바 */}
      <div className="px-3 pt-2">
        <div
          className="h-1 rounded-full overflow-hidden"
          style={{ backgroundColor: "var(--color-bg-tertiary)" }}
        >
          <div
            className="h-full rounded-full transition-all duration-300"
            style={{
              width: `${summary.percent}%`,
              backgroundColor: "var(--color-accent)",
            }}
          />
        </div>
      </div>

      {/* job 리스트 */}
      <div className="py-1">
        {batch.jobs.map((job) => (
          <JobRow key={job.index} job={job} />
        ))}
      </div>

      {/* 하단 요약 (완료 시) */}
      {!batch.is_running && summary.totalIndexed > 0 && (
        <div
          className="px-3 py-1.5 text-[11px] text-center"
          style={{
            borderTop: "1px solid var(--color-border)",
            color: "var(--color-text-muted)",
          }}
        >
          총 {summary.totalIndexed.toLocaleString()}개 파일 인덱싱 완료
        </div>
      )}
    </div>
  );
});
