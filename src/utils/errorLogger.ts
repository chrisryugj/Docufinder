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
 * Telegram 원격 리포트 — 사용자 설정이 켜져있을 때만 호출.
 * invoke 실패도 무시 (네트워크/토큰 미주입 포함)
 */
function reportRemote(
  source: string,
  title: string,
  message: string,
  context: Record<string, string>,
) {
  // 설정 캐시 확인 — window 에 붙여두고 SettingsContext 에서 매번 갱신
  const enabled = (window as unknown as { __errorReportingEnabled?: boolean })
    .__errorReportingEnabled;
  if (enabled === false) return;
  invoke("report_error", {
    payload: { source, title, message, context },
  }).catch(() => {});
}

/**
 * 글로벌 에러 핸들러 등록
 * - window.onerror: 동기 에러 캡처
 * - unhandledrejection: Promise 에러 캡처
 *
 * 로컬 로그 + Telegram 리모트 리포트 병행 (리모트는 설정 토글 체크)
 */
export function setupGlobalErrorHandlers() {
  window.onerror = (_event, source, lineno, colno, error) => {
    const message = error?.message || String(_event);
    const location = source ? `${source}:${lineno}:${colno}` : undefined;
    const stack = error?.stack || location || "";
    logToBackend("error", message, stack, "window.onerror");
    reportRemote("frontend", message.slice(0, 80), stack, {
      location: location || "",
      ua: navigator.userAgent.slice(0, 120),
    });
  };

  window.onunhandledrejection = (event: PromiseRejectionEvent) => {
    const reason = event.reason;
    const message =
      reason instanceof Error ? reason.message : String(reason || "Unknown");
    const stack = reason instanceof Error ? reason.stack || "" : "";
    logToBackend("error", message, stack, "unhandledrejection");
    reportRemote("frontend", message.slice(0, 80), stack || message, {
      kind: "unhandledrejection",
    });
  };
}

/**
 * Settings 로드/저장 시 호출 — 전역 에러 핸들러가 참조할 flag 업데이트
 */
export function setErrorReportingEnabled(enabled: boolean) {
  (window as unknown as { __errorReportingEnabled?: boolean })
    .__errorReportingEnabled = enabled;
}
