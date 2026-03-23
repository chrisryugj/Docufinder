import { useEffect, useRef, useCallback, useId, type ReactNode } from "react";

interface ModalProps {
  isOpen: boolean;
  /** 닫기 콜백 (closable={false}이면 생략 가능) */
  onClose?: () => void;
  title: string;
  children: ReactNode;
  footer?: ReactNode;
  /** 헤더 타이틀 아래 추가 콘텐츠 (탭 바 등) */
  headerExtra?: ReactNode;
  size?: "sm" | "md" | "lg";
  closable?: boolean; // ESC/배경 클릭/X 버튼으로 닫기 허용 여부
}

const sizeClasses = {
  sm: "max-w-md",
  md: "max-w-lg",
  lg: "max-w-xl",
};

// 포커스 가능한 요소 셀렉터
const FOCUSABLE_SELECTOR =
  'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])';

const noop = () => {};

export function Modal({ isOpen, onClose, title, children, footer, headerExtra, size = "md", closable = true }: ModalProps) {
  // closable={false}이면 onClose 불필요 → noop fallback
  const handleClose = onClose ?? noop;
  const titleId = useId();
  const modalRef = useRef<HTMLDivElement>(null);
  const previousActiveElement = useRef<HTMLElement | null>(null);

  // 포커스 트랩: Tab 키가 모달 내부에서만 순환
  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === "Escape" && closable) {
        handleClose();
        return;
      }

      if (e.key === "Tab" && modalRef.current) {
        const focusableElements = modalRef.current.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR);
        const firstElement = focusableElements[0];
        const lastElement = focusableElements[focusableElements.length - 1];

        if (!firstElement) return;

        if (e.shiftKey) {
          // Shift+Tab: 첫 번째 요소에서 마지막으로
          if (document.activeElement === firstElement) {
            e.preventDefault();
            lastElement?.focus();
          }
        } else {
          // Tab: 마지막 요소에서 첫 번째로
          if (document.activeElement === lastElement) {
            e.preventDefault();
            firstElement?.focus();
          }
        }
      }
    },
    [closable, handleClose]
  );

  // Effect 1: 포커스 + body scroll 관리 (open/close 전환 시에만)
  useEffect(() => {
    if (isOpen) {
      previousActiveElement.current = document.activeElement as HTMLElement;
      document.body.style.overflow = "hidden";

      requestAnimationFrame(() => {
        const focusableElements = modalRef.current?.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR);
        focusableElements?.[0]?.focus();
      });
    }

    return () => {
      // isOpen이 true→false로 바뀔 때만 복원 (cleanup은 이전 isOpen 값을 캡처)
      if (isOpen) {
        document.body.style.overflow = "";
        if (previousActiveElement.current?.isConnected) {
          previousActiveElement.current.focus();
        }
      }
    };
  }, [isOpen]);

  // Effect 2: keydown 리스너 (handleKeyDown 변경 시 리스너만 교체, 포커스 영향 없음)
  useEffect(() => {
    if (!isOpen) return;
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [isOpen, handleKeyDown]);

  // 배경 클릭으로 닫기 (closable일 때만)
  const handleBackdropClick = (e: React.MouseEvent) => {
    if (e.target === e.currentTarget && closable) {
      handleClose();
    }
  };

  if (!isOpen) return null;

  return (
    <div
      className="fixed inset-0 flex items-start justify-center z-50 pt-[10vh]"
      style={{ backgroundColor: "var(--color-backdrop)" }}
      onClick={handleBackdropClick}
      role="dialog"
      aria-modal="true"
      aria-labelledby={titleId}
    >
      <div
        ref={modalRef}
        className={`w-full ${sizeClasses[size]} mx-4 animate-modal-enter rounded-lg max-h-[80vh] flex flex-col`}
        style={{
          backgroundColor: "var(--color-bg-secondary)",
          boxShadow: "var(--shadow-xl)",
          border: "1px solid var(--color-border)",
        }}
      >
        {/* Header */}
        <div
          className="shrink-0 flex items-center justify-between px-6 py-3 border-b"
          style={{ borderColor: "var(--color-border)" }}
        >
          <div className="flex items-center gap-4">
            <h2
              id={titleId}
              className="text-base font-bold"
              style={{ color: "var(--color-text-primary)", letterSpacing: "-0.01em" }}
            >
              {title}
            </h2>
            {headerExtra}
          </div>
          {closable && (
            <button
              onClick={handleClose}
              className="p-1.5 rounded-md btn-icon-hover"
              aria-label="닫기"
            >
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          )}
        </div>

        {/* Content */}
        <div className="px-6 py-4 overflow-y-auto flex-1">
          {children}
        </div>

        {/* Footer (고정, 스크롤 밖) */}
        {footer && (
          <div
            className="px-6 py-4 border-t shrink-0"
            style={{ borderColor: "var(--color-border)" }}
          >
            {footer}
          </div>
        )}
      </div>
    </div>
  );
}
