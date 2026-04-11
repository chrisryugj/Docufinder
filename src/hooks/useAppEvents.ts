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
  const cbRef = useRef({ query, invalidateSearch, refreshStatus, refreshVectorStatus, showToast, updateToast });
  useEffect(() => {
    cbRef.current = { query, invalidateSearch, refreshStatus, refreshVectorStatus, showToast, updateToast };
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

  // 모델 다운로드 상태 이벤트 — ref 패턴으로 listener 재등록 방지
  useEffect(() => {
    let semanticToastId: string | null = null;
    let ocrToastId: string | null = null;
    let unlistenFn: UnlistenFn | null = null;
    listen<string>("model-download-status", (event) => {
      const cb = cbRef.current;
      switch (event.payload) {
        // 시맨틱 모델
        case "downloading":
          semanticToastId = cb.showToast("AI 모델 다운로드 중... (최초 1회)", "loading");
          break;
        case "completed":
          if (semanticToastId) {
            cb.updateToast(semanticToastId, { message: "AI 모델 다운로드 완료!", type: "success" });
          }
          break;
        case "failed":
          if (semanticToastId) {
            cb.updateToast(semanticToastId, { message: "AI 모델 다운로드 실패. 재시작하면 다시 시도합니다.", type: "error" }, 8000);
          } else {
            cb.showToast("AI 모델 다운로드 실패. 설정에서 시맨틱 검색을 확인하세요.", "error", 8000);
          }
          break;
        // OCR 모델
        case "downloading-ocr":
          ocrToastId = cb.showToast("OCR 모델 다운로드 중...", "loading");
          break;
        case "completed-ocr":
          if (ocrToastId) {
            cb.updateToast(ocrToastId, { message: "OCR 모델 다운로드 완료!", type: "success" });
          }
          break;
        case "failed-ocr":
          if (ocrToastId) {
            cb.updateToast(ocrToastId, { message: "OCR 모델 다운로드 실패. 재시작하면 다시 시도합니다.", type: "error" }, 8000);
          } else {
            cb.showToast("OCR 모델 다운로드 실패. 설정에서 OCR을 확인하세요.", "error", 8000);
          }
          break;
      }
    }).then((fn) => { unlistenFn = fn; });

    return () => { unlistenFn?.(); };
  }, []);

  // HWP 파일 감지 이벤트 (증분 인덱싱 시) — ref 패턴으로 재등록 방지
  const onHwpDetectedRef = useRef(onHwpDetected);
  useEffect(() => { onHwpDetectedRef.current = onHwpDetected; });
  useEffect(() => {
    let unlistenFn: UnlistenFn | null = null;
    listen<string[]>("hwp-files-detected", (event) => {
      onHwpDetectedRef.current?.(event.payload);
    }).then((fn) => { unlistenFn = fn; });

    return () => { unlistenFn?.(); };
  }, []);

  // DB 무결성 경고 이벤트
  useEffect(() => {
    let unlistenFn: UnlistenFn | null = null;
    listen<string>("db-integrity-warning", (event) => {
      cbRef.current.showToast(event.payload, "error", 15000);
    }).then((fn) => { unlistenFn = fn; });

    return () => { unlistenFn?.(); };
  }, []);
}
