import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { clearSearchCache } from "./useSearch";
import type { ToastType } from "../components/ui/Toast";
import { getErrorMessage } from "../types/error";

interface UseAppEventsOptions {
  query: string;
  invalidateSearch: () => void;
  refreshStatus: () => Promise<unknown>;
  refreshVectorStatus: () => Promise<unknown>;
  showToast: (message: string, type: ToastType, duration?: number) => string;
  updateToast: (id: string, update: { message: string; type: ToastType }, duration?: number) => void;
}

/**
 * App-level Tauri 이벤트 리스너 관리:
 * - incremental-index-updated: 증분 인덱싱 완료 → 캐시 무효화 + 재검색
 * - model-download-status: 모델 다운로드 상태 → 토스트
 * - indexing-warning: 드라이브 전체·시스템 폴더 인덱싱 안내 → 토스트
 * - db-integrity-warning / probe_kordoc_runtime: 시작 시 진단 → 에러 토스트
 */
export function useAppEvents({
  query,
  invalidateSearch,
  refreshStatus,
  refreshVectorStatus,
  showToast,
  updateToast,
}: UseAppEventsOptions) {
  const backgroundRefreshToastAtRef = useRef(0);
  const cbRef = useRef({ query, invalidateSearch, refreshStatus, refreshVectorStatus, showToast, updateToast });
  useEffect(() => {
    cbRef.current = { query, invalidateSearch, refreshStatus, refreshVectorStatus, showToast, updateToast };
  });

  // 증분 인덱싱 완료 이벤트 — ref 패턴으로 listener를 한 번만 등록 (deps 변경 시 재등록 방지)
  useEffect(() => {
    let unlistenFn: UnlistenFn | null = null;
    let disposed = false; // listen() 이 끝나기 전에 언마운트되면 등록 직후 해제
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
    }).then((fn) => { if (disposed) fn(); else unlistenFn = fn; });

    return () => { disposed = true; unlistenFn?.(); };
  }, []);

  // 모델 다운로드 상태 이벤트 — ref 패턴으로 listener 재등록 방지
  useEffect(() => {
    let semanticToastId: string | null = null;
    let ocrToastId: string | null = null;
    let layoutToastId: string | null = null;
    let unlistenFn: UnlistenFn | null = null;
    let disposed = false; // listen() 이 끝나기 전에 언마운트되면 등록 직후 해제
    listen<string>("model-download-status", (event) => {
      const cb = cbRef.current;
      switch (event.payload) {
        // 시맨틱 모델
        case "downloading":
          semanticToastId = cb.showToast("AI 모델 다운로드 중 (최초 1회)", "loading");
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
          ocrToastId = cb.showToast("OCR 모델 다운로드 중", "loading");
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
        // 레이아웃 분석 모델 (PP-DocLayout, 실험 기능 토글)
        case "downloading-layout":
          layoutToastId = cb.showToast("레이아웃 분석 모델 다운로드 중", "loading");
          break;
        case "completed-layout":
          if (layoutToastId) {
            cb.updateToast(layoutToastId, { message: "레이아웃 분석 모델 다운로드 완료!", type: "success" });
          }
          break;
        case "failed-layout": {
          // 레이아웃 분석은 선택 기능이고 모델이 설치본에 없다(온라인에서만 내려받음).
          // 인터넷이 막힌 환경에서 OCR 을 켜면 여기서 실패하는데, 그때 "실패" 만 뜨면
          // OCR 자체가 안 되는 걸로 읽힌다 — 영향 범위를 문구에 명시한다. (이슈 #35)
          const layoutMsg =
            "레이아웃 분석 모델을 받지 못했습니다. OCR 본체는 정상 동작하며, 이 선택 기능만 꺼집니다.";
          if (layoutToastId) {
            cb.updateToast(layoutToastId, { message: layoutMsg, type: "info" }, 8000);
          } else {
            cb.showToast(layoutMsg, "info", 8000);
          }
          break;
        }
      }
    }).then((fn) => { if (disposed) fn(); else unlistenFn = fn; });

    return () => { disposed = true; unlistenFn?.(); };
  }, []);

  // 인덱싱 안내 (드라이브 전체·시스템 폴더는 AI 검색 준비를 자동으로 시작하지 않음). 종전엔 받는 곳이 없었다.
  useEffect(() => {
    let unlistenFn: UnlistenFn | null = null;
    let disposed = false;
    listen<{ type: string; folder_path: string; message: string }>("indexing-warning", (event) => {
      cbRef.current.showToast(event.payload.message, "info", 10000);
    }).then((fn) => { if (disposed) fn(); else unlistenFn = fn; });

    return () => { disposed = true; unlistenFn?.(); };
  }, []);

  // DB 무결성 경고. 검사가 이 리스너보다 먼저 끝나면 이벤트가 유실되므로 마운트 때 한 번 가져오고,
  // 늦게 끝나면 이벤트로 받는다. 같은 문구는 한 번만 띄운다.
  const shownWarningsRef = useRef(new Set<string>());
  useEffect(() => {
    const show = (message: string) => {
      if (shownWarningsRef.current.has(message)) return;
      shownWarningsRef.current.add(message);
      cbRef.current.showToast(message, "error", 15000);
    };
    let unlistenFn: UnlistenFn | null = null;
    let disposed = false; // listen() 이 끝나기 전에 언마운트되면 등록 직후 해제
    listen<string>("db-integrity-warning", (event) => show(event.payload))
      .then((fn) => { if (disposed) fn(); else unlistenFn = fn; });
    invoke<string[]>("get_startup_warnings")
      .then((warnings) => { if (!disposed) warnings.forEach(show); })
      .catch(() => {});

    return () => { disposed = true; unlistenFn?.(); };
  }, []);

  // 문서 변환기(kordoc 사이드카) 실행 점검 — 시작 시 1회. 백엔드 setup 의 `kordoc-availability`
  // emit 은 React 마운트 전이라 어떤 리스너도 받지 못하므로 push 대신 여기서 pull 한다.
  // 실행통제가 node.exe 를 막는 내부망 PC 에서 "HWP 가 조용히 전부 실패" 를 원인·조치와 함께 띄운다.
  useEffect(() => {
    let cancelled = false;
    invoke<string>("probe_kordoc_runtime").catch((e) => {
      if (!cancelled) cbRef.current.showToast(getErrorMessage(e), "error", 20000);
    });
    return () => { cancelled = true; };
  }, []);
}
