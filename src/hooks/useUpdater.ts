import { useCallback, useEffect, useRef, useState } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { getErrorMessage } from "../types/error";
import { isMac } from "../utils/platform";

export type UpdatePhase =
  | "idle"
  | "checking"
  | "available"
  | "up-to-date"
  | "downloading"
  | "installing"
  | "ready-to-restart"
  | "error";

export interface UpdateState {
  phase: UpdatePhase;
  version?: string;
  notes?: string;
  downloadedBytes: number;
  totalBytes: number;
  error?: string;
  lastCheckedAt?: number;
}

const CHECK_INTERVAL_MS = 6 * 60 * 60 * 1000;
const STARTUP_DELAY_MS = 30 * 1000;

export function useUpdater(auto: boolean = true) {
  const [state, setState] = useState<UpdateState>({
    phase: "idle",
    downloadedBytes: 0,
    totalBytes: 0,
  });
  const updateRef = useRef<Update | null>(null);
  // 취소 플래그: 사용자가 다운로드 중간에 "취소" 클릭 시 set.
  // plugin-updater 의 downloadAndInstall 은 AbortController 지원 X 이므로,
  // 진행 이벤트 콜백에서 이 플래그를 읽어 UI state 를 idle 로 되돌리고 이후 이벤트 무시.
  const cancelledRef = useRef(false);

  const checkForUpdate = useCallback(async (): Promise<Update | null> => {
    // macOS는 ad-hoc 서명(Apple Developer ID 미보유)으로 자동 업데이트 미지원.
    // tauri-action 의 latest.json 에는 windows-x86_64 항목만 들어가서, plugin-updater 가
    // `darwin-aarch64-app`/`darwin-aarch64` fallback platform 을 못 찾고 오류를 던진다.
    // → mac 에서는 check 호출 자체를 우회하고 "최신" 으로 표시. 신버전은 GitHub Releases 수동 다운로드.
    if (isMac) {
      setState((s) => ({ ...s, phase: "up-to-date", lastCheckedAt: Date.now() }));
      return null;
    }
    setState((s) => ({ ...s, phase: "checking", error: undefined }));
    try {
      const u = await check();
      if (u) {
        updateRef.current = u;
        setState((s) => ({
          ...s,
          phase: "available",
          version: u.version,
          notes: u.body ?? undefined,
          lastCheckedAt: Date.now(),
        }));
        return u;
      }
      setState((s) => ({
        ...s,
        phase: "up-to-date",
        lastCheckedAt: Date.now(),
      }));
      return null;
    } catch (err) {
      const msg = getErrorMessage(err);
      setState((s) => ({ ...s, phase: "error", error: msg, lastCheckedAt: Date.now() }));
      return null;
    }
  }, []);

  const downloadAndInstall = useCallback(async () => {
    const u = updateRef.current;
    if (!u) return;

    cancelledRef.current = false;
    setState((s) => ({ ...s, phase: "downloading", downloadedBytes: 0, totalBytes: 0 }));

    try {
      let total = 0;
      let downloaded = 0;

      await u.downloadAndInstall((event) => {
        // 취소 후 뒤늦게 도착하는 이벤트는 무시 (UI state 덮어쓰기 방지)
        if (cancelledRef.current) return;
        switch (event.event) {
          case "Started":
            total = event.data.contentLength ?? 0;
            setState((s) => ({ ...s, totalBytes: total, downloadedBytes: 0 }));
            break;
          case "Progress":
            downloaded += event.data.chunkLength;
            setState((s) => ({ ...s, downloadedBytes: downloaded }));
            break;
          case "Finished":
            setState((s) => ({ ...s, phase: "installing" }));
            break;
        }
      });

      if (!cancelledRef.current) {
        setState((s) => ({ ...s, phase: "ready-to-restart" }));
      }
    } catch (err) {
      if (cancelledRef.current) return;
      const msg = getErrorMessage(err);
      setState((s) => ({ ...s, phase: "error", error: msg }));
    }
  }, []);

  const cancel = useCallback(() => {
    cancelledRef.current = true;
    setState((s) => ({ ...s, phase: "idle", downloadedBytes: 0, totalBytes: 0 }));
  }, []);

  const restart = useCallback(async () => {
    await relaunch();
  }, []);

  const dismiss = useCallback(() => {
    setState((s) =>
      s.phase === "available" || s.phase === "up-to-date" || s.phase === "error"
        ? { ...s, phase: "idle" }
        : s
    );
  }, []);

  useEffect(() => {
    // mac 은 자동 업데이트 미지원 (위 checkForUpdate 가드 참조). 타이머 자체를 안 건다.
    if (!auto || isMac) return;

    const startTimer = setTimeout(() => {
      void checkForUpdate();
    }, STARTUP_DELAY_MS);

    const interval = setInterval(() => {
      void checkForUpdate();
    }, CHECK_INTERVAL_MS);

    return () => {
      clearTimeout(startTimer);
      clearInterval(interval);
    };
  }, [auto, checkForUpdate]);

  return { state, checkForUpdate, downloadAndInstall, restart, dismiss, cancel };
}
