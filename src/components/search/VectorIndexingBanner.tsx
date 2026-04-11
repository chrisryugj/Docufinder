interface VectorIndexingBannerProps {
  isVisible: boolean;
  progress: number;
  onCancel: () => void;
}

/** 벡터 인덱싱 진행 중 배너 (검색바 아래 표시) */
export function VectorIndexingBanner({ isVisible, progress, onCancel }: VectorIndexingBannerProps) {
  if (!isVisible) return null;

  return (
    <div
      role="status"
      aria-live="polite"
      className="max-w-4xl mx-auto mt-2 px-3 py-2 rounded-lg flex items-center justify-between text-xs"
      style={{
        backgroundColor: "var(--color-accent-subtle, rgba(59, 130, 246, 0.1))",
        border: "1px solid var(--color-accent-border, rgba(59, 130, 246, 0.2))",
        color: "var(--color-text-secondary)",
      }}
    >
      <div className="flex items-center gap-2">
        <div
          className="animate-spin h-3 w-3 rounded-full"
          style={{ border: "1px solid var(--color-accent)", borderTopColor: "transparent" }}
        />
        <span>벡터 인덱싱 중... ({progress}%) — 키워드 검색만 가능</span>
      </div>
      <button onClick={onCancel} className="font-medium" style={{ color: "var(--color-accent)" }}>
        취소
      </button>
    </div>
  );
}
