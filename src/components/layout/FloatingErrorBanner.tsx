import { memo, useEffect, useState } from "react";
import { AlertTriangle, X } from "lucide-react";

interface FloatingErrorBannerProps {
  message: string | null;
  onDismiss: () => void;
  isError: boolean;
}

export const FloatingErrorBanner = memo(function FloatingErrorBanner({
  message,
  onDismiss,
  isError = true,
}: FloatingErrorBannerProps) {
  const [visible, setVisible] = useState(false);
  const [mounted, setMounted] = useState(false);

  // Mount/unmount with CSS transition
  useEffect(() => {
    if (message) {
      setMounted(true);
      // Force reflow before adding visible class
      requestAnimationFrame(() => requestAnimationFrame(() => setVisible(true)));
    } else {
      setVisible(false);
      const timer = setTimeout(() => setMounted(false), 200);
      return () => clearTimeout(timer);
    }
  }, [message]);

  // Auto dismiss after 8s if not critical
  useEffect(() => {
    if (message && !isError) {
      const timer = setTimeout(() => onDismiss(), 8000);
      return () => clearTimeout(timer);
    }
  }, [message, isError, onDismiss]);

  if (!mounted) return null;

  return (
    <div
      className={`toast-banner ${isError ? "toast-banner-error" : ""}`}
      role="alert"
      style={{
        opacity: visible ? 1 : 0,
        transform: visible ? "translateY(0) scale(1)" : "translateY(-20px) scale(0.95)",
        transition: "opacity 0.25s ease-out, transform 0.25s ease-out",
      }}
    >
      <div className="flex items-center gap-2">
        {isError ? (
          <AlertTriangle className="w-5 h-5 flex-shrink-0" />
        ) : (
          <span className="w-2 h-2 rounded-full bg-blue-500 opacity-80 shadow-[0_0_8px_rgba(59,130,246,0.8)]" />
        )}
        <span className="text-sm font-medium pr-4">{message}</span>
      </div>
      <button
        onClick={onDismiss}
        className="p-1 rounded-md opacity-70 hover:opacity-100 transition-opacity ml-auto"
        aria-label="닫기"
      >
        <X className="w-4 h-4" />
      </button>
    </div>
  );
});
