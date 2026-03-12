import type { UseUpdaterReturn } from "../../hooks/useUpdater";

interface UpdateBannerProps {
  updater: UseUpdaterReturn;
}

export function UpdateBanner({ updater }: UpdateBannerProps) {
  const { updateAvailable, downloadProgress, status, error, startUpdate, dismiss } = updater;

  // 표시 조건: 업데이트 있거나 다운로드/설치 중이거나 에러
  if (!updateAvailable && status !== "downloading" && status !== "installing" && status !== "error") {
    return null;
  }

  return (
    <div
      className="flex items-center justify-between px-3 py-2 text-xs"
      style={{
        backgroundColor: error
          ? "var(--color-error-subtle, rgba(239, 68, 68, 0.1))"
          : "var(--color-accent-subtle, rgba(59, 130, 246, 0.1))",
        borderBottom: `1px solid ${error
          ? "var(--color-error-border, rgba(239, 68, 68, 0.2))"
          : "var(--color-accent-border, rgba(59, 130, 246, 0.2))"
        }`,
        color: "var(--color-text-primary)",
      }}
    >
      <div className="flex items-center gap-2 min-w-0">
        {status === "downloading" && (
          <>
            <div
              className="animate-spin h-3 w-3 rounded-full shrink-0"
              style={{ border: "1px solid var(--color-accent)", borderTopColor: "transparent" }}
            />
            <span className="truncate">
              업데이트 다운로드 중... {downloadProgress}%
            </span>
            <div
              className="h-1 rounded-full overflow-hidden shrink-0"
              style={{ width: 80, backgroundColor: "var(--color-border)" }}
            >
              <div
                className="h-full rounded-full transition-all duration-300"
                style={{
                  width: `${downloadProgress}%`,
                  backgroundColor: "var(--color-accent)",
                }}
              />
            </div>
          </>
        )}

        {status === "installing" && (
          <>
            <div
              className="animate-spin h-3 w-3 rounded-full shrink-0"
              style={{ border: "1px solid var(--color-accent)", borderTopColor: "transparent" }}
            />
            <span>설치 중... 앱이 곧 재시작됩니다.</span>
          </>
        )}

        {status === "error" && (
          <span style={{ color: "var(--color-error, #ef4444)" }}>
            업데이트 실패: {error}
          </span>
        )}

        {status === "available" && updateAvailable && (
          <span className="truncate">
            새 버전 <strong>v{updateAvailable.version}</strong> 사용 가능
            {updateAvailable.body && ` — ${updateAvailable.body.split("\n")[0]}`}
          </span>
        )}
      </div>

      <div className="flex items-center gap-2 shrink-0 ml-2">
        {status === "available" && (
          <>
            <button
              onClick={startUpdate}
              className="px-2 py-0.5 rounded font-medium text-white"
              style={{ backgroundColor: "var(--color-accent)" }}
            >
              지금 설치
            </button>
            <button
              onClick={dismiss}
              className="px-2 py-0.5 rounded"
              style={{ color: "var(--color-text-tertiary)" }}
            >
              나중에
            </button>
          </>
        )}

        {status === "error" && (
          <button
            onClick={dismiss}
            className="px-2 py-0.5 rounded"
            style={{ color: "var(--color-text-tertiary)" }}
          >
            닫기
          </button>
        )}
      </div>
    </div>
  );
}
