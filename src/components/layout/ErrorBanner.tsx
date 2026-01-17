interface ErrorBannerProps {
  message: string;
  onDismiss: () => void;
}

export function ErrorBanner({ message, onDismiss }: ErrorBannerProps) {
  return (
    <div
      className="mt-3 p-3 rounded-lg flex items-center justify-between"
      style={{
        backgroundColor: "rgba(var(--color-error-rgb), 0.15)",
        border: "1px solid var(--color-error)",
      }}
      role="alert"
    >
      <span className="text-sm" style={{ color: "var(--color-error)" }}>
        {message}
      </span>
      <button
        onClick={onDismiss}
        className="ml-2 hover:opacity-70 transition-opacity"
        style={{ color: "var(--color-error)" }}
        aria-label="에러 닫기"
      >
        ✕
      </button>
    </div>
  );
}
