import { useState, useEffect, useCallback, useRef } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

export interface UpdateInfo {
  version: string;
  body: string | null;
}

export interface UseUpdaterReturn {
  /** 사용 가능한 업데이트 정보 (없으면 null) */
  updateAvailable: UpdateInfo | null;
  /** 다운로드 진행률 (0-100) */
  downloadProgress: number;
  /** 현재 상태 */
  status: "idle" | "checking" | "available" | "downloading" | "installing" | "error";
  /** 에러 메시지 */
  error: string | null;
  /** 업데이트 다운로드 + 설치 시작 */
  startUpdate: () => void;
  /** 업데이트 알림 닫기 (이번 세션에서 숨김) */
  dismiss: () => void;
  /** 수동 업데이트 체크 */
  checkNow: () => void;
}

const CHECK_INTERVAL_MS = 6 * 60 * 60 * 1000; // 6시간

export function useUpdater(): UseUpdaterReturn {
  const [updateAvailable, setUpdateAvailable] = useState<UpdateInfo | null>(null);
  const [downloadProgress, setDownloadProgress] = useState(0);
  const [status, setStatus] = useState<UseUpdaterReturn["status"]>("idle");
  const [error, setError] = useState<string | null>(null);
  const [dismissed, setDismissed] = useState(false);
  const updateRef = useRef<Update | null>(null);

  const doCheck = useCallback(async () => {
    try {
      setStatus("checking");
      setError(null);
      const update = await check();
      if (update) {
        updateRef.current = update;
        setUpdateAvailable({ version: update.version, body: update.body ?? null });
        setStatus("available");
        setDismissed(false);
      } else {
        setStatus("idle");
      }
    } catch (e) {
      // 네트워크 오류 등은 조용히 무시 (오프라인 환경 대응)
      setStatus("idle");
    }
  }, []);

  // 앱 시작 3초 후 첫 체크 + 6시간 주기
  useEffect(() => {
    const initialTimer = setTimeout(() => {
      doCheck();
    }, 3000);

    const interval = setInterval(doCheck, CHECK_INTERVAL_MS);

    return () => {
      clearTimeout(initialTimer);
      clearInterval(interval);
    };
  }, [doCheck]);

  const startUpdate = useCallback(async () => {
    const update = updateRef.current;
    if (!update) return;

    try {
      setStatus("downloading");
      setDownloadProgress(0);

      let contentLength = 0;
      let downloaded = 0;

      await update.downloadAndInstall((event) => {
        if (event.event === "Started" && event.data.contentLength) {
          contentLength = event.data.contentLength;
        } else if (event.event === "Progress") {
          downloaded += event.data.chunkLength;
          if (contentLength > 0) {
            setDownloadProgress(Math.min(100, Math.round((downloaded / contentLength) * 100)));
          }
        } else if (event.event === "Finished") {
          setDownloadProgress(100);
        }
      });

      setStatus("installing");
      await relaunch();
    } catch (e) {
      setError(e instanceof Error ? e.message : "업데이트 실패");
      setStatus("error");
    }
  }, []);

  const dismiss = useCallback(() => {
    setDismissed(true);
  }, []);

  const checkNow = useCallback(() => {
    doCheck();
  }, [doCheck]);

  return {
    updateAvailable: dismissed ? null : updateAvailable,
    downloadProgress,
    status,
    error,
    startUpdate,
    dismiss,
    checkNow,
  };
}
