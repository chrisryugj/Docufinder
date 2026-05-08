import { useCallback, useEffect, useRef, useState } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
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
  /** macOS 전용 — 새 버전 발견 시 GitHub release 페이지 URL.
   *  set 되어 있으면 모달이 "다운로드 페이지 열기" 버튼을 노출. */
  releaseUrl?: string;
}

interface GithubReleaseInfo {
  tag_name: string;
  html_url: string;
  name?: string | null;
  body?: string | null;
}

const CHECK_INTERVAL_MS = 6 * 60 * 60 * 1000;
const STARTUP_DELAY_MS = 30 * 1000;
const GITHUB_REPO = "chrisryugj/Docufinder";

/** "v2.5.21" / "2.5.21" 등에서 숫자 튜플만 뽑아 비교. semver 라이브러리 도입 회피. */
function isNewerVersion(latest: string, current: string): boolean {
  const parse = (s: string) =>
    s
      .replace(/^v/i, "")
      .split(/[.\-+]/)
      .map((p) => parseInt(p, 10))
      .filter((n) => !Number.isNaN(n));
  const a = parse(latest);
  const b = parse(current);
  const len = Math.max(a.length, b.length);
  for (let i = 0; i < len; i++) {
    const x = a[i] ?? 0;
    const y = b[i] ?? 0;
    if (x !== y) return x > y;
  }
  return false;
}

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
  // openReleasePage 가 stale closure 없이 최신 releaseUrl 을 읽도록 ref 미러.
  const stateRef = useRef(state);
  stateRef.current = state;

  const checkForUpdate = useCallback(async (): Promise<Update | null> => {
    // macOS는 ad-hoc 서명(Apple Developer ID 미보유)으로 plugin-updater 자동 설치는 미지원.
    // 대신 GitHub Releases API 로 최신 태그를 직접 조회 → 새 버전이면 사용자에게 release
    // 페이지 안내(이슈 #22 — 사용자 제안 반영). plugin-updater.check() 는 windows-x86_64 만
    // 등록된 latest.json 에서 darwin platform 을 못 찾고 throw 하므로 호출하지 않는다.
    setState((s) => ({ ...s, phase: "checking", error: undefined, releaseUrl: undefined }));

    if (isMac) {
      try {
        const [info, current] = await Promise.all([
          invoke<GithubReleaseInfo>("check_github_release", { repo: GITHUB_REPO }),
          getVersion(),
        ]);
        if (isNewerVersion(info.tag_name, current)) {
          setState((s) => ({
            ...s,
            phase: "available",
            version: info.tag_name.replace(/^v/i, ""),
            notes: info.body ?? undefined,
            releaseUrl: info.html_url,
            lastCheckedAt: Date.now(),
          }));
        } else {
          setState((s) => ({
            ...s,
            phase: "up-to-date",
            lastCheckedAt: Date.now(),
          }));
        }
      } catch (err) {
        setState((s) => ({
          ...s,
          phase: "error",
          error: getErrorMessage(err),
          lastCheckedAt: Date.now(),
        }));
      }
      return null;
    }

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

  /** macOS 전용 — 새 버전 발견 시 GitHub release 페이지를 시스템 브라우저에서 연다. */
  const openReleasePage = useCallback(async () => {
    const url = stateRef.current.releaseUrl;
    if (!url) return;
    try {
      await invoke("open_url", { url });
    } catch (err) {
      setState((s) => ({ ...s, phase: "error", error: getErrorMessage(err) }));
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
    if (!auto) return;

    // mac 도 자동 체크 — plugin-updater 대신 GitHub Releases API 로 새 버전을 확인하고
    // 발견 시 모달이 release 페이지 다운로드 안내 UI 를 띄운다 (이슈 #22 사용자 제안).
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

  return { state, checkForUpdate, downloadAndInstall, restart, dismiss, cancel, openReleasePage };
}
