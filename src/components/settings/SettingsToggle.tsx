import { memo, useId } from "react";

interface SettingsToggleProps {
  label: string;
  description: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
}

export const SettingsToggle = memo(function SettingsToggle({
  label,
  description,
  checked,
  onChange,
  disabled = false,
}: SettingsToggleProps) {
  // 스위치가 이름·설명을 갖도록 라벨과 설명을 id 로 잇는다 (종전엔 화면 낭독기에 "스위치"로만 읽혔다)
  const id = useId();
  return (
    <div className="flex items-center justify-between gap-2">
      <div className="min-w-0 flex-1" style={{ wordBreak: "keep-all", overflowWrap: "break-word" }}>
        <label
          id={`${id}-label`}
          htmlFor={`${id}-switch`}
          className="text-sm font-medium"
          style={{ color: "var(--color-text-secondary)" }}
        >
          {label}
        </label>
        <p id={`${id}-desc`} className="mt-0.5 text-xs leading-snug" style={{ color: "var(--color-text-muted)" }}>
          {description}
        </p>
      </div>
      <button
        type="button"
        id={`${id}-switch`}
        role="switch"
        aria-checked={checked}
        aria-labelledby={`${id}-label`}
        aria-describedby={`${id}-desc`}
        disabled={disabled}
        onClick={() => !disabled && onChange(!checked)}
        className={`relative inline-flex h-6 w-11 flex-shrink-0 items-center rounded-full transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:ring-[var(--color-accent)] ${disabled ? "opacity-40 cursor-not-allowed" : ""}`}
        style={{
          backgroundColor: checked ? "var(--color-accent)" : "var(--color-bg-tertiary)",
        }}
      >
        <span
          className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
            checked ? "translate-x-6" : "translate-x-1"
          }`}
        />
      </button>
    </div>
  );
});
