import { Modal } from "../ui/Modal";
import { Button } from "../ui/Button";
import type { UpdateState } from "../../hooks/useUpdater";

interface UpdateModalProps {
  isOpen: boolean;
  onClose: () => void;
  state: UpdateState;
  onInstall: () => void;
  onRestart: () => void;
  onCancel?: () => void;
}

function formatBytes(n: number): string {
  if (!n) return "0";
  const units = ["B", "KB", "MB", "GB"];
  let i = 0;
  let v = n;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(i === 0 ? 0 : 1)} ${units[i]}`;
}

export function UpdateModal({ isOpen, onClose, state, onInstall, onRestart, onCancel }: UpdateModalProps) {
  const { phase, version, notes, downloadedBytes, totalBytes, error } = state;
  const progress = totalBytes > 0 ? Math.min(100, (downloadedBytes / totalBytes) * 100) : 0;

  const title =
    phase === "available"
      ? `업데이트 ${version} 사용 가능`
      : phase === "downloading"
      ? "업데이트 다운로드 중"
      : phase === "installing"
      ? "업데이트 설치 중"
      : phase === "ready-to-restart"
      ? "업데이트 준비 완료"
      : phase === "up-to-date"
      ? "최신 버전입니다"
      : phase === "error"
      ? "업데이트 오류"
      : "업데이트";

  return (
    <Modal
      isOpen={isOpen}
      onClose={phase === "installing" ? () => {} : onClose}
      title={title}
      size="md"
      footer={
        <div className="flex justify-end gap-2">
          {phase === "available" && (
            <>
              <Button variant="ghost" size="sm" onClick={onClose}>나중에</Button>
              <Button size="sm" onClick={onInstall}>지금 설치</Button>
            </>
          )}
          {phase === "ready-to-restart" && (
            <>
              <Button variant="ghost" size="sm" onClick={onClose}>나중에</Button>
              <Button size="sm" onClick={onRestart}>재시작</Button>
            </>
          )}
          {(phase === "up-to-date" || phase === "error") && (
            <Button size="sm" onClick={onClose}>닫기</Button>
          )}
          {(phase === "downloading" || phase === "installing") && (
            <Button
              size="sm"
              variant="ghost"
              onClick={() => {
                onCancel?.();
                onClose();
              }}
              disabled={phase === "installing"}
              title={phase === "installing" ? "설치 중에는 취소할 수 없습니다" : "다운로드 취소"}
            >
              취소
            </Button>
          )}
        </div>
      }
    >
      {phase === "available" && (
        <div className="space-y-2">
          <p className="text-sm" style={{ color: "var(--color-text-primary)" }}>
            새 버전 <b>{version}</b>이(가) 배포되었습니다.
          </p>
          {notes && (
            <div
              className="text-xs p-2.5 rounded-md max-h-48 overflow-y-auto whitespace-pre-wrap"
              style={{
                backgroundColor: "var(--color-bg-primary)",
                border: "1px solid var(--color-border)",
                color: "var(--color-text-secondary)",
                wordBreak: "keep-all",
                overflowWrap: "break-word",
              }}
            >
              {notes}
            </div>
          )}
          <p className="text-xs" style={{ color: "var(--color-text-muted)" }}>
            다운로드 후 앱이 자동 재시작됩니다. 인덱스/설정은 보존됩니다.
          </p>
        </div>
      )}

      {(phase === "downloading" || phase === "installing") && (
        <div className="space-y-3">
          <div>
            <div className="flex justify-between text-xs mb-1" style={{ color: "var(--color-text-muted)" }}>
              <span>
                {phase === "downloading" ? "다운로드" : "설치"}
              </span>
              <span>
                {formatBytes(downloadedBytes)}
                {totalBytes > 0 && ` / ${formatBytes(totalBytes)}`}
              </span>
            </div>
            <div className="h-2 rounded-full overflow-hidden" style={{ backgroundColor: "var(--color-border)" }}>
              <div
                className="h-full transition-all"
                style={{
                  width: `${phase === "downloading" ? progress : 100}%`,
                  backgroundColor: "var(--color-accent)",
                }}
              />
            </div>
          </div>
          <p className="text-xs" style={{ color: "var(--color-text-muted)" }}>
            {phase === "installing" ? "설치 중입니다. 잠시만 기다려주세요." : "업데이트를 다운로드하고 있습니다."}
          </p>
        </div>
      )}

      {phase === "ready-to-restart" && (
        <p className="text-sm" style={{ color: "var(--color-text-primary)" }}>
          업데이트가 설치되었습니다. 지금 재시작하시겠습니까?
        </p>
      )}

      {phase === "up-to-date" && (
        <p className="text-sm" style={{ color: "var(--color-text-primary)" }}>
          이미 최신 버전을 사용 중입니다.
        </p>
      )}

      {phase === "error" && (
        <div className="space-y-2">
          <p className="text-sm" style={{ color: "var(--color-error)" }}>
            업데이트 중 오류가 발생했습니다.
          </p>
          {error && (
            <code className="block text-xs p-2 rounded" style={{ backgroundColor: "var(--color-bg-primary)", color: "var(--color-text-muted)" }}>
              {error}
            </code>
          )}
        </div>
      )}
    </Modal>
  );
}
