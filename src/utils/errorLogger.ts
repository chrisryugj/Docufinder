import { invoke } from "@tauri-apps/api/core";

/**
 * 프론트엔드 에러를 Rust 로그 파일에 전송
 * fire-and-forget: 로깅 실패가 앱 동작에 영향 주지 않음
 */
export function logToBackend(
  level: "error" | "warn" | "info",
  message: string,
  stack?: string,
  source?: string,
) {
  invoke("log_frontend_error", { level, message, stack, source }).catch(
    () => {},
  );
}

/**
 * 글로벌 에러 핸들러 등록
 * - window.onerror: 동기 에러 캡처
 * - unhandledrejection: Promise 에러 캡처
 */
export function setupGlobalErrorHandlers() {
  window.onerror = (_event, source, lineno, colno, error) => {
    const message = error?.message || String(_event);
    const location = source ? `${source}:${lineno}:${colno}` : undefined;
    logToBackend("error", message, error?.stack || location, "window.onerror");
  };

  window.onunhandledrejection = (event: PromiseRejectionEvent) => {
    const reason = event.reason;
    const message =
      reason instanceof Error ? reason.message : String(reason || "Unknown");
    const stack = reason instanceof Error ? reason.stack : undefined;
    logToBackend("error", message, stack, "unhandledrejection");
  };
}
