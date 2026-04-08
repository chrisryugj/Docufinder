import { RotateCw } from "lucide-react";

interface ErrorBannerProps {
  message: string;
  onDismiss: () => void;
  onRetry?: () => void;
}

/** 사용자 친화적 에러 메시지 변환 */
function humanizeError(message: string): string {
  if (message.includes("timeout") || message.includes("Timeout")) {
    return "검색이 너무 오래 걸렸습니다. 검색어를 단순화하거나 필터를 적용해 보세요.";
  }
  if (message.includes("vector") || message.includes("Vector")) {
    return "시맨틱 인덱싱 중 오류가 발생했습니다. 설정에서 시맨틱 검색을 재활성화해 보세요.";
  }
  if (message.includes("index") || message.includes("Index")) {
    return "인덱싱 중 오류가 발생했습니다. 해당 폴더를 다시 추가해 보세요.";
  }
  if (message.includes("permission") || message.includes("Permission") || message.includes("access")) {
    return "파일 접근 권한이 없습니다. 관리자 권한으로 실행하거나 폴더 권한을 확인하세요.";
  }
  return message;
}

export function ErrorBanner({ message, onDismiss, onRetry }: ErrorBannerProps) {
  const displayMessage = humanizeError(message);

  return (
    <div
      className="mt-3 p-3 rounded-lg flex items-center justify-between gap-2"
      style={{
        backgroundColor: "rgba(var(--color-error-rgb), 0.15)",
        border: "1px solid var(--color-error)",
      }}
      role="alert"
    >
      <span className="text-sm flex-1" style={{ color: "var(--color-error)" }}>
        {displayMessage}
      </span>
      <div className="flex items-center gap-1 flex-shrink-0">
        {onRetry && (
          <button
            onClick={onRetry}
            className="flex items-center gap-1 px-2 py-1 text-xs font-medium rounded transition-opacity hover:opacity-80"
            style={{ color: "var(--color-error)" }}
            aria-label="재시도"
          >
            <RotateCw className="w-3 h-3" />
            재시도
          </button>
        )}
        <button
          onClick={onDismiss}
          className="ml-1 hover:opacity-70 transition-opacity"
          style={{ color: "var(--color-error)" }}
          aria-label="에러 닫기"
        >
          ✕
        </button>
      </div>
    </div>
  );
}
