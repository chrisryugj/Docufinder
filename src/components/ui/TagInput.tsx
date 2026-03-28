import { memo, useState, useRef, useEffect, useCallback } from "react";
import { X, Plus } from "lucide-react";

interface TagInputProps {
  tags: string[];
  suggestions?: string[];
  onAdd: (tag: string) => void;
  onRemove: (tag: string) => void;
  maxTags?: number;
}

export const TagInput = memo(function TagInput({
  tags,
  suggestions = [],
  onAdd,
  onRemove,
  maxTags = 10,
}: TagInputProps) {
  const [editing, setEditing] = useState(false);
  const [value, setValue] = useState("");
  const [showSuggestions, setShowSuggestions] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (editing) inputRef.current?.focus();
  }, [editing]);

  // 외부 클릭 닫기
  useEffect(() => {
    if (!editing) return;
    const handler = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setEditing(false);
        setValue("");
        setShowSuggestions(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [editing]);

  const handleSubmit = useCallback(() => {
    const trimmed = value.trim();
    if (trimmed && !tags.includes(trimmed) && tags.length < maxTags) {
      onAdd(trimmed);
      setValue("");
    }
  }, [value, tags, maxTags, onAdd]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      e.preventDefault();
      handleSubmit();
    } else if (e.key === "Escape") {
      setEditing(false);
      setValue("");
      setShowSuggestions(false);
    }
  };

  const filteredSuggestions = suggestions.filter(
    (s) => !tags.includes(s) && s.toLowerCase().includes(value.toLowerCase())
  ).slice(0, 8);

  return (
    <div ref={containerRef} className="flex flex-wrap items-center gap-1">
      {tags.map((tag) => (
        <span
          key={tag}
          className="inline-flex items-center gap-0.5 px-1.5 py-0.5 rounded text-[11px] font-medium"
          style={{
            backgroundColor: "var(--color-accent-bg, color-mix(in srgb, var(--color-accent) 12%, transparent))",
            color: "var(--color-accent)",
          }}
        >
          {tag}
          <button
            onClick={() => onRemove(tag)}
            className="p-0 rounded-full hover:opacity-70 transition-opacity"
            title={`태그 "${tag}" 제거`}
            aria-label={`태그 "${tag}" 제거`}
          >
            <X size={10} />
          </button>
        </span>
      ))}

      {editing ? (
        <div className="relative">
          <input
            ref={inputRef}
            type="text"
            value={value}
            onChange={(e) => { setValue(e.target.value); setShowSuggestions(true); }}
            onKeyDown={handleKeyDown}
            placeholder="태그 입력..."
            maxLength={50}
            className="w-24 px-1.5 py-0.5 text-[11px] rounded border focus:outline-none focus-visible:ring-1 focus-visible:ring-[var(--color-accent)]"
            style={{
              backgroundColor: "var(--color-bg-secondary)",
              borderColor: "var(--color-border)",
              color: "var(--color-text-primary)",
            }}
          />
          {showSuggestions && filteredSuggestions.length > 0 && (
            <div
              className="absolute top-full left-0 mt-0.5 w-32 rounded border shadow-lg z-50 overflow-hidden"
              style={{
                backgroundColor: "var(--color-bg-primary)",
                borderColor: "var(--color-border)",
              }}
            >
              {filteredSuggestions.map((s) => (
                <button
                  key={s}
                  onClick={() => { onAdd(s); setValue(""); setShowSuggestions(false); }}
                  className="w-full px-2 py-1 text-[11px] text-left hover:bg-[var(--color-bg-tertiary)] text-[var(--color-text-primary)]"
                >
                  {s}
                </button>
              ))}
            </div>
          )}
        </div>
      ) : (
        tags.length < maxTags && (
          <button
            onClick={() => setEditing(true)}
            className="inline-flex items-center gap-0.5 px-1 py-0.5 rounded text-[11px] transition-colors hover:bg-[var(--color-bg-tertiary)]"
            style={{ color: "var(--color-text-muted)" }}
            title="태그 추가"
            aria-label="태그 추가"
          >
            <Plus size={10} />
            태그
          </button>
        )
      )}
    </div>
  );
});
