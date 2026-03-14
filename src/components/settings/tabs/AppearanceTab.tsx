import { ColorPresetPicker } from "../ColorPresetPicker";
import type { TabProps } from "./types";
import { HIGHLIGHT_COLOR_PRESETS } from "./types";

export function AppearanceTab({ settings, onChange }: TabProps) {
  return (
    <div className="space-y-5">
      <ColorPresetPicker
        label="파일명 하이라이트"
        description="파일명 검색 결과에서 매칭된 글자 강조 색상"
        presets={HIGHLIGHT_COLOR_PRESETS}
        selectedValue={settings.highlight_filename_color}
        onChange={(v) => onChange("highlight_filename_color", v)}
      />

      <ColorPresetPicker
        label="문서 내용 하이라이트"
        description="문서 검색 결과에서 매칭된 키워드 강조 색상"
        presets={HIGHLIGHT_COLOR_PRESETS}
        selectedValue={settings.highlight_content_color}
        onChange={(v) => onChange("highlight_content_color", v)}
      />
    </div>
  );
}
