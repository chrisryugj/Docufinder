import { useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { logToBackend } from "../utils/errorLogger";

const FOCUS_DEBOUNCE_MS = 500;

/**
 * 윈도우 포커스 복귀 시 검색창 자동 포커스
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
