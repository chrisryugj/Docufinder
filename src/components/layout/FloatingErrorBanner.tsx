import { memo, useEffect } from "react";
import { AlertTriangle, X } from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";

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
  // auto dismiss after 8s if it's not a critical error
  useEffect(() => {
    if (message && !isError) {
      const timer = setTimeout(() => {
        onDismiss();
      }, 8000);
      return () => clearTimeout(timer);
    }
  }, [message, isError, onDismiss]);

  return (
    <AnimatePresence>
      {message && (
        <motion.div
          initial={{ opacity: 0, y: -20, scale: 0.95 }}
          animate={{ opacity: 1, y: 0, scale: 1 }}
          exit={{ opacity: 0, y: -20, scale: 0.95 }}
          transition={{ type: "spring", stiffness: 400, damping: 25 }}
          className={`toast-banner ${isError ? "toast-banner-error" : ""}`}
          role="alert"
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
        </motion.div>
      )}
    </AnimatePresence>
  );
});
