import { useState, useEffect, useCallback } from "react";
import { exit } from "@tauri-apps/plugin-process";

const STORAGE_KEYS = {
  DISCLAIMER_ACCEPTED: "docufinder_disclaimer_accepted",
  DISCLAIMER_VERSION: "docufinder_disclaimer_version",
  ONBOARDING_COMPLETED: "docufinder_onboarding_completed",
};

const CURRENT_DISCLAIMER_VERSION = "1.0";

export function useFirstRun() {
  const [showDisclaimer, setShowDisclaimer] = useState(false);
  const [showOnboarding, setShowOnboarding] = useState(false);
  const [isInitialized, setIsInitialized] = useState(false);

  useEffect(() => {
    const disclaimerAccepted = localStorage.getItem(STORAGE_KEYS.DISCLAIMER_ACCEPTED);
    const disclaimerVersion = localStorage.getItem(STORAGE_KEYS.DISCLAIMER_VERSION);
    const onboardingCompleted = localStorage.getItem(STORAGE_KEYS.ONBOARDING_COMPLETED);

    // 면책 조항 버전이 변경되었으면 재동의 필요
    const needsDisclaimer = !disclaimerAccepted || disclaimerVersion !== CURRENT_DISCLAIMER_VERSION;
    const needsOnboarding = !onboardingCompleted;

    if (needsDisclaimer) {
      setShowDisclaimer(true);
    } else if (needsOnboarding) {
      setShowOnboarding(true);
    }

    setIsInitialized(true);
  }, []);

  const acceptDisclaimer = useCallback(() => {
    localStorage.setItem(STORAGE_KEYS.DISCLAIMER_ACCEPTED, "true");
    localStorage.setItem(STORAGE_KEYS.DISCLAIMER_VERSION, CURRENT_DISCLAIMER_VERSION);
    setShowDisclaimer(false);

    const onboardingCompleted = localStorage.getItem(STORAGE_KEYS.ONBOARDING_COMPLETED);
    if (!onboardingCompleted) {
      setShowOnboarding(true);
    }
  }, []);

  const completeOnboarding = useCallback(() => {
    localStorage.setItem(STORAGE_KEYS.ONBOARDING_COMPLETED, "true");
    setShowOnboarding(false);
  }, []);

  const skipOnboarding = useCallback(() => {
    localStorage.setItem(STORAGE_KEYS.ONBOARDING_COMPLETED, "true");
    setShowOnboarding(false);
  }, []);

  const exitApp = useCallback(async () => {
    await exit(0);
  }, []);

  // 설정에서 온보딩 다시 보기
  const resetOnboarding = useCallback(() => {
    localStorage.removeItem(STORAGE_KEYS.ONBOARDING_COMPLETED);
    setShowOnboarding(true);
  }, []);

  return {
    showDisclaimer,
    showOnboarding,
    isInitialized,
    acceptDisclaimer,
    completeOnboarding,
    skipOnboarding,
    exitApp,
    resetOnboarding,
  };
}
