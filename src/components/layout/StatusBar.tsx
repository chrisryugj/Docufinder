import type { IndexStatus, IndexingProgress } from "../../types/index";

interface StatusBarProps {
  status: IndexStatus | null;
  progress: IndexingProgress | null;
  onCancelIndexing?: () => void;
}

const phaseLabels: Record<string, string> = {
  scanning: "파일 검색 중",
  parsing: "파일 분석 중",
  indexing: "인덱싱 중",
  completed: "완료",
  cancelled: "취소됨",
};

export function StatusBar({ status, progress, onCancelIndexing }: StatusBarProps) {
  const isIndexing = progress && progress.phase !== "completed" && progress.phase !== "cancelled";
  const percent = progress && progress.total_files > 0
    ? Math.round((progress.processed_files / progress.total_files) * 100)
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
              <span style={{ color: "var(--color-text-muted)" }}>
                {progress.processed_files} / {progress.total_files}
              </span>
            </div>
            <div className="flex items-center gap-3">
              <span style={{ color: "var(--color-text-muted)" }}>{percent}%</span>
              {onCancelIndexing && (
                <button
                  onClick={onCancelIndexing}
                  className="px-2 py-0.5 text-xs rounded transition-colors"
                  style={{
                    backgroundColor: "var(--color-bg-tertiary)",
                    color: "var(--color-text-secondary)",
                  }}
                  onMouseEnter={(e) => {
                    e.currentTarget.style.backgroundColor = "var(--color-error)";
                    e.currentTarget.style.color = "white";
                  }}
                  onMouseLeave={(e) => {
                    e.currentTarget.style.backgroundColor = "var(--color-bg-tertiary)";
                    e.currentTarget.style.color = "var(--color-text-secondary)";
                  }}
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
              className="h-full transition-all duration-300"
              style={{
                width: `${percent}%`,
                backgroundColor: "var(--color-primary)",
              }}
            />
          </div>

          {/* 현재 파일명 */}
          {progress.current_file && (
            <div
              className="text-xs truncate"
              style={{ color: "var(--color-text-muted)" }}
              title={progress.current_file}
            >
              {progress.current_file}
            </div>
          )}
        </div>
      ) : (
        <div
          className="flex justify-between text-sm"
          style={{ color: "var(--color-text-muted)" }}
        >
          <span>
            인덱싱된 문서:{" "}
            <span style={{ color: "var(--color-text-secondary)" }}>
              {status?.total_files ?? 0}개
            </span>
          </span>
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
        </div>
      )}
    </footer>
  );
}
