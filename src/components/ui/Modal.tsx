import { useEffect, useRef, useCallback, type ReactNode } from "react";

interface ModalProps {
  isOpen: boolean;
  onClose: () => void;
  title: string;
  children: ReactNode;
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

export function Modal({ isOpen, onClose, title, children, size = "md", closable = true }: ModalProps) {
  const modalRef = useRef<HTMLDivElement>(null);
  const previousActiveElement = useRef<HTMLElement | null>(null);

  // 포커스 트랩: Tab 키가 모달 내부에서만 순환
  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === "Escape" && closable) {
        onClose();
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
    [closable, onClose]
  );

  useEffect(() => {
    if (isOpen) {
      // 현재 포커스된 요소 저장
      previousActiveElement.current = document.activeElement as HTMLElement;

      document.addEventListener("keydown", handleKeyDown);
      document.body.style.overflow = "hidden";

      // 모달 열릴 때 첫 번째 포커스 가능 요소로 포커스
      requestAnimationFrame(() => {
        const focusableElements = modalRef.current?.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR);
        focusableElements?.[0]?.focus();
      });
    }

    return () => {
      document.removeEventListener("keydown", handleKeyDown);
      document.body.style.overflow = "";

      // 모달 닫힐 때 이전 포커스 복원
      if (previousActiveElement.current) {
        previousActiveElement.current.focus();
      }
    };
  }, [isOpen, handleKeyDown]);

  // 배경 클릭으로 닫기 (closable일 때만)
  const handleBackdropClick = (e: React.MouseEvent) => {
    if (e.target === e.currentTarget && closable) {
      onClose();
    }
  };

  if (!isOpen) return null;

  return (
    <div
      className="fixed inset-0 flex items-center justify-center z-50"
      style={{ backgroundColor: "rgba(0, 0, 0, 0.6)" }}
      onClick={handleBackdropClick}
      role="dialog"
      aria-modal="true"
      aria-labelledby="modal-title"
    >
      <div
        ref={modalRef}
        className={`w-full ${sizeClasses[size]} mx-4 animate-modal-enter rounded-lg max-h-[85vh] flex flex-col`}
        style={{
          backgroundColor: "var(--color-bg-secondary)",
          boxShadow: "var(--shadow-lg)",
          border: "1px solid var(--color-border)",
        }}
      >
        {/* Header */}
        <div
          className="flex items-center justify-between px-6 py-4 border-b"
          style={{ borderColor: "var(--color-border)" }}
        >
          <h2
            id="modal-title"
            className="text-lg font-semibold"
            style={{ color: "var(--color-text-primary)" }}
          >
            {title}
          </h2>
          {closable && (
            <button
              onClick={onClose}
              className="p-1.5 rounded-md transition-all duration-200"
              style={{ color: "var(--color-text-muted)" }}
              onMouseEnter={(e) => {
                e.currentTarget.style.backgroundColor = "var(--color-bg-tertiary)";
                e.currentTarget.style.color = "var(--color-text-primary)";
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.backgroundColor = "transparent";
                e.currentTarget.style.color = "var(--color-text-muted)";
              }}
              aria-label="닫기"
            >
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          )}
        </div>

        {/* Content */}
        <div className="px-6 py-5 overflow-y-auto flex-1">
          {children}
        </div>
      </div>
    </div>
  );
}
