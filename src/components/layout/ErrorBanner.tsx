interface ErrorBannerProps {
  message: string;
  onDismiss: () => void;
}

export function ErrorBanner({ message, onDismiss }: ErrorBannerProps) {
  return (
    <div
      className="mt-3 p-3 bg-red-900/30 border border-red-600/50 rounded-lg flex items-center justify-between"
      role="alert"
    >
      <span className="text-red-300 text-sm">{message}</span>
      <button
        onClick={onDismiss}
        className="text-red-400 hover:text-red-300 ml-2"
        aria-label="에러 닫기"
      >
        ✕
      </button>
    </div>
  );
}
