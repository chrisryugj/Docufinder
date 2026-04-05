import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { logToBackend } from "../utils/errorLogger";

const STORAGE_KEYS = {
  ONBOARDING_COMPLETED: "docufinder_onboarding_completed",
};

export function useFirstRun() {
  const [showOnboarding, setShowOnboarding] = useState(false);
  const [isInitialized, setIsInitialized] = useState(false);

  useEffect(() => {
    const onboardingCompleted = localStorage.getItem(STORAGE_KEYS.ONBOARDING_COMPLETED);

    // 앱 초기화 (벡터 인덱싱 재개 + Startup Sync)
    invoke("initialize_app").catch((e) => {
      logToBackend("error", "Failed to initialize app", String(e), "useFirstRun");
    });

    if (!onboardingCompleted) {
      setShowOnboarding(true);
    }

    setIsInitialized(true);
  }, []);

  const completeOnboarding = useCallback(() => {
    localStorage.setItem(STORAGE_KEYS.ONBOARDING_COMPLETED, "true");
    setShowOnboarding(false);
  }, []);

  const skipOnboarding = useCallback(() => {
    localStorage.setItem(STORAGE_KEYS.ONBOARDING_COMPLETED, "true");
    setShowOnboarding(false);
  }, []);

  const resetOnboarding = useCallback(() => {
    localStorage.removeItem(STORAGE_KEYS.ONBOARDING_COMPLETED);
    setShowOnboarding(true);
  }, []);

  return {
    showOnboarding,
    isInitialized,
    completeOnboarding,
    skipOnboarding,
    resetOnboarding,
  };
}
