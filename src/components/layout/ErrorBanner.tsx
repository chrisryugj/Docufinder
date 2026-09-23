import { RotateCw } from "lucide-react";

interface ErrorBannerProps {
  message: string;
  onDismiss: () => void;
  onRetry?: () => void;
}

/** 사용자 친화적 에러 메시지 변환. 코드별 문구는 getErrorMessage 가 이미 만든다 —
 *  여기서는 검색 시간 초과와 OS 권한 오류(영문 원문)에만 행동 안내를 붙인다.
 *  배너는 검색·색인·AI 검색 준비 오류를 한 줄로 보여 주므로, 검색어 안내는 검색 오류("검색 실패: …")에만.
 *  (종전엔 "index" 가 들어간 아무 메시지에 "폴더를 다시 추가해 보세요"를 붙여 오진했다) */
function humanizeError(message: string): string {
  if (message.startsWith("검색 실패") && /timeout|타임아웃|오래 걸려/i.test(message)) {
    return "검색이 너무 오래 걸렸습니다. 검색어를 단순화하거나 필터를 적용해 보세요.";
  }
  if (/permission denied|os error (5|13)\b/i.test(message)) {
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
