import { useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { logToBackend } from "../utils/errorLogger";

const FOCUS_DEBOUNCE_MS = 500;

/**
 * 윈도우 포커스 복귀 시 검색창 자동 포커스
 *
 * 열려있는 모달(role="dialog")이 있으면 포커스 강탈하지 않음 —
 * 포커스 트랩이 깨지면 스크린리더/키보드 사용자가 모달을 빠져나가게 됨.
 */
export function useWindowFocus(
  searchInputRef: React.RefObject<HTMLInputElement | null>,
  settingsOpen: boolean
): void {
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let lastResetTime = 0;

    const resetSearchFocus = () => {
      if (settingsOpen) return;
      // 모든 모달 체크 (SettingsOpen 외 Help/Stats/Onboarding/Disclaimer 등)
      if (document.querySelector("[role='dialog']")) return;

      const now = Date.now();
      if (now - lastResetTime < FOCUS_DEBOUNCE_MS) return;
      lastResetTime = now;

      const input = searchInputRef.current;
      if (!input || !input.isConnected) return;

      const activeElement = document.activeElement;
      const isEditable =
        activeElement?.tagName === "INPUT" ||
        activeElement?.tagName === "TEXTAREA" ||
        (activeElement instanceof HTMLElement && activeElement.isContentEditable);

      if (isEditable && activeElement !== input) return;
      if (activeElement === input) return;

      requestAnimationFrame(() => {
        if (input.isConnected) {
          input.focus();
        }
      });
    };

    const setup = async () => {
      const window = getCurrentWindow();
      try {
        unlisten = await window.onFocusChanged(({ payload }) => {
          if (payload) {
            resetSearchFocus();
          }
        });
      } catch (err) {
        logToBackend("warn", "Failed to register focus handler", String(err), "App");
      }
    };

    setup();

    return () => {
      if (unlisten) unlisten();
    };
  }, [settingsOpen, searchInputRef]);
}
