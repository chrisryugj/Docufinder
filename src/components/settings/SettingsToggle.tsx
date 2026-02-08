import { memo } from "react";

interface SettingsToggleProps {
  label: string;
  description: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
  /** 활성 시 색상 (기본: bg-blue-500) */
  activeColor?: string;
}

export const SettingsToggle = memo(function SettingsToggle({
  label,
  description,
  checked,
  onChange,
  activeColor = "bg-blue-500",
}: SettingsToggleProps) {
  return (
    <div className="flex items-center justify-between">
      <div>
        <label
          className="text-sm font-medium"
          style={{ color: "var(--color-text-secondary)" }}
        >
          {label}
        </label>
        <p className="mt-0.5 text-xs" style={{ color: "var(--color-text-muted)" }}>
          {description}
        </p>
      </div>
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        onClick={() => onChange(!checked)}
        className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500 focus-visible:ring-offset-2 ${
          checked ? activeColor : "bg-[var(--color-bg-tertiary)]"
        }`}
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
