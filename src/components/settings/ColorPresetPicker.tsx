import { memo } from "react";

interface ColorPreset {
  value: string;
  label: string;
  light: string;
  dark: string;
}

interface ColorPresetPickerProps {
  label: string;
  description: string;
  presets: ColorPreset[];
  selectedValue: string | undefined;
  onChange: (value: string | undefined) => void;
}

export const ColorPresetPicker = memo(function ColorPresetPicker({
  label,
  description,
  presets,
  selectedValue,
  onChange,
}: ColorPresetPickerProps) {
  const currentValue = selectedValue || "";

  return (
    <div>
      <label
        className="block text-sm font-medium mb-2"
        style={{ color: "var(--color-text-secondary)" }}
      >
        {label}
      </label>
      <div className="flex flex-wrap gap-2">
        {presets.map((preset) => (
          <button
            key={preset.value || "default"}
            type="button"
            onClick={() => onChange(preset.value || undefined)}
            className={`w-8 h-8 rounded-lg border-2 transition-all ${
              currentValue === preset.value ? "ring-2 ring-offset-2" : ""
            }`}
            style={{
              backgroundColor: preset.light,
              borderColor:
                currentValue === preset.value
                  ? "var(--color-accent)"
                  : "var(--color-border)",
            }}
            title={preset.label}
            aria-label={`${preset.label} 색상 선택`}
          />
        ))}
      </div>
      <p className="mt-1.5 text-xs" style={{ color: "var(--color-text-muted)" }}>
        {description}
      </p>
    </div>
  );
});
