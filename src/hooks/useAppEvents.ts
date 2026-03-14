import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { clearSearchCache } from "./useSearch";
import type { ToastType } from "../components/ui/Toast";

interface UseAppEventsOptions {
  query: string;
  invalidateSearch: () => void;
  refreshStatus: () => Promise<unknown>;
  refreshVectorStatus: () => Promise<unknown>;
  showToast: (message: string, type: ToastType, duration?: number) => string;
  updateToast: (id: string, update: { message: string; type: ToastType }, duration?: number) => void;
  onHwpDetected?: (paths: string[]) => void;
}

/**
 * App-level Tauri 이벤트 리스너 관리:
 * - incremental-index-updated: 증분 인덱싱 완료 → 캐시 무효화 + 재검색
 * - model-download-status: 모델 다운로드 상태 → 토스트
 */
export function useAppEvents({
  query,
  invalidateSearch,
  refreshStatus,
  refreshVectorStatus,
  showToast,
  updateToast,
  onHwpDetected,
}: UseAppEventsOptions) {
  const backgroundRefreshToastAtRef = useRef(0);

  // 증분 인덱싱 완료 이벤트
  useEffect(() => {
    const unlisten = listen<number>("incremental-index-updated", (event) => {
      clearSearchCache();
      void refreshStatus();
      void refreshVectorStatus();

      if (query.trim()) {
        invalidateSearch();

        const now = Date.now();
        if (now - backgroundRefreshToastAtRef.current > 4000) {
          backgroundRefreshToastAtRef.current = now;
          showToast(
            `${event.payload}개 변경 파일을 반영해 현재 검색 결과를 새로고침했습니다.`,
            "info",
            2500
          );
        }
      }
    });

    return () => { unlisten.then((fn) => fn()).catch(() => {}); };
  }, [query, invalidateSearch, refreshStatus, refreshVectorStatus, showToast]);

  // 모델 다운로드 상태 이벤트
  useEffect(() => {
    let toastId: string | null = null;
    const unlisten = listen<string>("model-download-status", (event) => {
      switch (event.payload) {
        case "downloading":
          toastId = showToast("AI 모델 다운로드 중... (최초 1회)", "loading");
          break;
        case "completed":
          if (toastId) {
            updateToast(toastId, { message: "AI 모델 다운로드 완료!", type: "success" });
          }
          break;
        case "failed":
          if (toastId) {
            updateToast(toastId, { message: "AI 모델 다운로드 실패. 재시작하면 다시 시도합니다.", type: "error" }, 5000);
          }
          break;
      }
    });

    return () => { unlisten.then((fn) => fn()).catch(() => {}); };
  }, [showToast, updateToast]);

  // HWP 파일 감지 이벤트 (증분 인덱싱 시)
  useEffect(() => {
    if (!onHwpDetected) return;
    const unlisten = listen<string[]>("hwp-files-detected", (event) => {
      onHwpDetected(event.payload);
    });

    return () => { unlisten.then((fn) => fn()).catch(() => {}); };
  }, [onHwpDetected]);
}
