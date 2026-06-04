import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { logToBackend } from "../utils/errorLogger";

/**
 * 앱 첫 실행 초기화.
 * 온보딩 안내는 OnboardingTour(자체 storageKey) + AutoIndexPrompt가 담당한다.
 */
export function useFirstRun() {
  const [isInitialized, setIsInitialized] = useState(false);

  useEffect(() => {
    // 앱 초기화 (벡터 인덱싱 재개 + Startup Sync)
    invoke("initialize_app").catch((e) => {
      logToBackend("error", "Failed to initialize app", String(e), "useFirstRun");
    });

    setIsInitialized(true);
  }, []);

  return { isInitialized };
}
