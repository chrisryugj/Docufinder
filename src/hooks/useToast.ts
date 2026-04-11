import { useState, useCallback, useRef, useEffect } from "react";
import type { ToastData, ToastType } from "../components/ui/Toast";

/**
 * 토스트 알림 관리 훅
 *
 * 사용 예시:
 * ```tsx
 * const { toasts, showToast, updateToast, dismissToast } = useToast();
 *
 * // 일반 토스트
 * showToast("저장 완료", "success");
 *
 * // 로딩 → 성공 패턴
 * const id = showToast("저장 중...", "loading");
 * await saveData();
 * updateToast(id, { message: "저장 완료", type: "success" });
 * ```
 */
const MAX_VISIBLE_TOASTS = 3;

export function useToast() {
  const [toasts, setToasts] = useState<ToastData[]>([]);
  const timersRef = useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map());

  // cleanup on unmount
  useEffect(() => {
    const timers = timersRef.current;
    return () => {
      timers.forEach(clearTimeout);
      timers.clear();
    };
  }, []);

  /**
   * 토스트 표시
   * @param message 메시지
   * @param type 타입 (success/error/loading/info)
   * @param duration 자동 닫힘 시간 (ms). 0이면 자동 닫힘 없음. loading은 기본적으로 자동 닫힘 없음.
   * @returns 토스트 ID (업데이트/닫기용)
   */
  const showToast = useCallback(
    (message: string, type: ToastType = "info", duration = 3000): string => {
      const id = `toast-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;

      setToasts((prev) => {
        const next = [...prev, { id, message, type }];
        // 최대 표시 수 초과 시 오래된 것부터 제거
        if (next.length > MAX_VISIBLE_TOASTS) {
          const removed = next.splice(0, next.length - MAX_VISIBLE_TOASTS);
          for (const t of removed) {
            const timer = timersRef.current.get(t.id);
            if (timer) {
              clearTimeout(timer);
              timersRef.current.delete(t.id);
            }
          }
        }
        return next;
      });

      // loading 타입이 아니고 duration > 0이면 자동 닫기
      if (type !== "loading" && duration > 0) {
        const timer = setTimeout(() => {
          timersRef.current.delete(id);
          setToasts((prev) => prev.filter((toast) => toast.id !== id));
        }, duration);
        timersRef.current.set(id, timer);
      }

      // loading 타입: 30초 안전망 (백엔드 hang 시 영구 표시 방지)
      if (type === "loading") {
        const safetyTimer = setTimeout(() => {
          timersRef.current.delete(id);
          setToasts((prev) => prev.filter((toast) => toast.id !== id));
        }, 30000);
        timersRef.current.set(id, safetyTimer);
      }

      return id;
    },
    []
  );

  /**
   * 토스트 업데이트 (로딩 → 성공/실패 전환용)
   */
  const updateToast = useCallback(
    (id: string, updates: Partial<Omit<ToastData, "id">>, duration = 3000) => {
      // 기존 타이머 취소
      const existingTimer = timersRef.current.get(id);
      if (existingTimer) {
        clearTimeout(existingTimer);
        timersRef.current.delete(id);
      }

      setToasts((prev) =>
        prev.map((toast) =>
          toast.id === id ? { ...toast, ...updates } : toast
        )
      );

      // 업데이트 후 자동 닫기 (loading이 아닌 경우)
      if (updates.type && updates.type !== "loading" && duration > 0) {
        const timer = setTimeout(() => {
          timersRef.current.delete(id);
          setToasts((prev) => prev.filter((toast) => toast.id !== id));
        }, duration);
        timersRef.current.set(id, timer);
      }
    },
    []
  );

  /**
   * 토스트 닫기 (타이머도 정리)
   */
  const dismissToast = useCallback((id: string) => {
    // 예약된 타이머 취소
    const timer = timersRef.current.get(id);
    if (timer) {
      clearTimeout(timer);
      timersRef.current.delete(id);
    }
    setToasts((prev) => prev.filter((toast) => toast.id !== id));
  }, []);

  /**
   * 모든 토스트 닫기
   */
  const dismissAll = useCallback(() => {
    setToasts([]);
  }, []);

  return {
    toasts,
    showToast,
    updateToast,
    dismissToast,
    dismissAll,
  };
}
