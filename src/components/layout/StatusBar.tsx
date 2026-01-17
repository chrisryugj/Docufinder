import type { IndexStatus } from "../../types/index";

interface StatusBarProps {
  status: IndexStatus | null;
}

export function StatusBar({ status }: StatusBarProps) {
  return (
    <footer
      className="px-4 py-2.5 border-t"
      style={{
        backgroundColor: "var(--color-bg-secondary)",
        borderColor: "var(--color-border)",
      }}
    >
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
    </footer>
  );
}
