import { useEffect, useState } from "react";

export type ToastType = "success" | "error" | "loading" | "info";

export interface ToastData {
  id: string;
  message: string;
  type: ToastType;
}

interface ToastProps {
  toast: ToastData;
  onDismiss: (id: string) => void;
}

/**
 * 개별 토스트 컴포넌트
 */
export function Toast({ toast, onDismiss }: ToastProps) {
  const [isVisible, setIsVisible] = useState(false);

  useEffect(() => {
    // 마운트 시 애니메이션
    requestAnimationFrame(() => setIsVisible(true));
  }, []);

  const handleDismiss = () => {
    setIsVisible(false);
    setTimeout(() => onDismiss(toast.id), 200);
  };

  const getStyles = () => {
    switch (toast.type) {
      case "success":
        return {
          bg: "var(--color-success)",
          icon: (
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
            </svg>
          ),
        };
      case "error":
        return {
          bg: "var(--color-error)",
          icon: (
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          ),
        };
      case "loading":
        return {
          bg: "var(--color-accent)",
          icon: (
            <svg className="w-4 h-4 animate-spin" fill="none" viewBox="0 0 24 24">
              <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
              <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
            </svg>
          ),
        };
      case "info":
      default:
        return {
          bg: "var(--color-accent)",
          icon: (
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
          ),
        };
    }
  };

  const styles = getStyles();

  return (
    <div
      className={`flex items-center gap-3 px-4 py-3 rounded-lg text-white text-sm shadow-lg backdrop-blur-sm transition-all duration-200 ${
        isVisible ? "opacity-100 translate-x-0" : "opacity-0 translate-x-4"
      }`}
      style={{ backgroundColor: styles.bg }}
      role="alert"
    >
      {styles.icon}
      <span className="flex-1">{toast.message}</span>
      {toast.type !== "loading" && (
        <button
          onClick={handleDismiss}
          className="p-1 rounded hover:bg-white/20 transition-colors"
          aria-label="닫기"
        >
          <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      )}
    </div>
  );
}

interface ToastContainerProps {
  toasts: ToastData[];
  onDismiss: (id: string) => void;
}

/**
 * 토스트 컨테이너 - 여러 토스트를 스택으로 표시
 */
export function ToastContainer({ toasts, onDismiss }: ToastContainerProps) {
  if (toasts.length === 0) return null;

  return (
    <div className="fixed bottom-6 right-6 z-[100] flex flex-col gap-2">
      {toasts.map((toast) => (
        <Toast key={toast.id} toast={toast} onDismiss={onDismiss} />
      ))}
    </div>
  );
}
