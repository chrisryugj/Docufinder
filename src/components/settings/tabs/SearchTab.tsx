import { InfoTooltip } from "../../ui/Tooltip";
import { SettingsToggle } from "../SettingsToggle";
import type { TabProps } from "./types";
import { CONFIDENCE_STEP } from "./types";

export function SearchTab({ settings, onChange }: TabProps) {
  return (
    <div className="space-y-3">
      {/* 최소 신뢰도 */}
      <div>
        <label
          className="flex items-center text-sm font-medium mb-1"
          style={{ color: "var(--color-text-secondary)" }}
        >
          최소 신뢰도
          <InfoTooltip
            content={
              <div className="space-y-2 py-1">
                <div>
                  <strong style={{ color: "var(--color-text-primary)" }}>점수 산정</strong>
                  <p className="mt-0.5">RRF 방식으로 키워드·의미 검색 순위를 병합 계산</p>
                </div>
                <div>
                  <strong style={{ color: "var(--color-text-primary)" }}>추천</strong>
                  <ul className="mt-0.5 space-y-0.5">
                    <li>0%: 모든 결과 / 20-30%: 권장 / 50%+: 정확한 결과만</li>
                  </ul>
                </div>
              </div>
            }
            maxWidth={280}
          />
        </label>
        <div className="flex items-center gap-3">
          <input
            type="range"
            min={0}
            max={100}
            step={CONFIDENCE_STEP}
            value={settings.min_confidence}
            onChange={(e) => onChange("min_confidence", Number(e.target.value))}
            className="flex-1 accent-[var(--color-accent)]"
            aria-label="최소 신뢰도 설정"
          />
          <div
            className="min-w-[40px] text-sm font-semibold text-right"
            style={{ color: "var(--color-text-primary)" }}
          >
            {settings.min_confidence}%
          </div>
        </div>
      </div>

      {/* 하위폴더 포함 */}
      <SettingsToggle
        label="하위폴더 포함"
        description="폴더 추가 시 하위폴더도 함께 인덱싱"
        checked={settings.include_subfolders ?? true}
        onChange={(v) => onChange("include_subfolders", v)}
      />

      {/* HWP 자동 감지 */}
      <SettingsToggle
        label="HWP 변환 알림"
        description="새 HWP 파일 감지 시 HWPX 변환 안내 (한글 설치 필요)"
        checked={settings.hwp_auto_detect ?? false}
        onChange={(v) => onChange("hwp_auto_detect", v)}
      />

      {/* 제외 디렉토리 */}
      <div>
        <label className="block text-sm font-medium mb-1" style={{ color: "var(--color-text-secondary)" }}>
          제외 디렉토리
          <span className="font-normal ml-1" style={{ color: "var(--color-text-muted)" }}>
            (줄바꿈 구분, 기본: Windows·Program Files·AppData 등)
          </span>
        </label>
        <textarea
          className="w-full rounded-md px-3 py-1.5 text-xs font-mono resize-y"
          style={{
            backgroundColor: "var(--color-bg-secondary)",
            color: "var(--color-text-primary)",
            border: "1px solid var(--color-border)",
            minHeight: "48px",
          }}
          value={(settings.exclude_dirs ?? []).join("\n")}
          onChange={(e) =>
            onChange(
              "exclude_dirs",
              e.target.value
                .split("\n")
                .map((s) => s.trim())
                .filter((s): s is string => Boolean(s))
            )
          }
          placeholder="추가 제외할 폴더명 입력..."
          rows={2}
        />
      </div>

      {/* OCR 설정 */}
      <div>
        <SettingsToggle
          label="OCR (이미지 텍스트 인식)"
          description="JPG, PNG, BMP, TIFF 이미지에서 텍스트 추출 (PaddleOCR, ~15MB 모델)"
          checked={settings.ocr_enabled ?? false}
          onChange={(v) => onChange("ocr_enabled", v)}
        />
      </div>
    </div>
  );
}
