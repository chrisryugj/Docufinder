import { memo } from "react";

interface SuggestedWord {
  word: string;
  distance: number;
  frequency: number;
}

interface TypoSuggestionProps {
  suggestions: SuggestedWord[];
  onAccept: (word: string) => void;
  onDismiss: () => void;
}

export const TypoSuggestion = memo(function TypoSuggestion({
  suggestions,
  onAccept,
  onDismiss,
}: TypoSuggestionProps) {
  if (suggestions.length === 0) return null;

  return (
    <div
      className="flex items-center gap-2 px-3 py-1.5 text-xs rounded-md"
      style={{
        backgroundColor: "color-mix(in srgb, var(--color-accent) 8%, var(--color-bg-primary))",
        color: "var(--color-text-secondary)",
      }}
      role="status"
      aria-live="polite"
    >
      <span style={{ color: "var(--color-text-muted)" }}>혹시 이것을 찾으셨나요?</span>
      {suggestions.map((s) => (
        <button
          key={s.word}
          onClick={() => onAccept(s.word)}
          className="px-2 py-0.5 rounded font-medium transition-colors hover:opacity-80"
          style={{
            backgroundColor: "var(--color-accent)",
            color: "white",
          }}
        >
          {s.word}
        </button>
      ))}
      <button
        onClick={onDismiss}
        className="ml-auto p-0.5 rounded hover:bg-[var(--color-bg-tertiary)] transition-colors"
        style={{ color: "var(--color-text-muted)" }}
        title="닫기"
        aria-label="교정 제안 닫기"
      >
        <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
        </svg>
      </button>
    </div>
  );
});
