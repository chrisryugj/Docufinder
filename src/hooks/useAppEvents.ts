import { useEffect, useRef } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
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
  const cbRef = useRef({ query, invalidateSearch, refreshStatus, refreshVectorStatus, showToast });
  useEffect(() => {
    cbRef.current = { query, invalidateSearch, refreshStatus, refreshVectorStatus, showToast };
  });

  // 증분 인덱싱 완료 이벤트 — ref 패턴으로 listener를 한 번만 등록 (deps 변경 시 재등록 방지)
  useEffect(() => {
    let unlistenFn: UnlistenFn | null = null;
    listen<number>("incremental-index-updated", (event) => {
      const cb = cbRef.current;
      clearSearchCache();
      void cb.refreshStatus();
      void cb.refreshVectorStatus();

      if (cb.query.trim()) {
        cb.invalidateSearch();

        const now = Date.now();
        if (now - backgroundRefreshToastAtRef.current > 4000) {
          backgroundRefreshToastAtRef.current = now;
          cb.showToast(
            `${event.payload}개 변경 파일을 반영해 현재 검색 결과를 새로고침했습니다.`,
            "info",
            2500
          );
        }
      }
    }).then((fn) => { unlistenFn = fn; });

    return () => { unlistenFn?.(); };
  }, []);

  // 모델 다운로드 상태 이벤트
  useEffect(() => {
    let toastId: string | null = null;
    let unlistenFn: UnlistenFn | null = null;
    listen<string>("model-download-status", (event) => {
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
    }).then((fn) => { unlistenFn = fn; });

    return () => { unlistenFn?.(); };
  }, [showToast, updateToast]);

  // HWP 파일 감지 이벤트 (증분 인덱싱 시)
  useEffect(() => {
    if (!onHwpDetected) return;
    let unlistenFn: UnlistenFn | null = null;
    listen<string[]>("hwp-files-detected", (event) => {
      onHwpDetected(event.payload);
    }).then((fn) => { unlistenFn = fn; });

    return () => { unlistenFn?.(); };
  }, [onHwpDetected]);
}
