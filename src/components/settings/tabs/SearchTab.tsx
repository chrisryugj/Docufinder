import { Dropdown } from "../../ui/Dropdown";
import { InfoTooltip } from "../../ui/Tooltip";
import { SettingsToggle } from "../SettingsToggle";
import type { Settings } from "../../../types/settings";
import type { TabProps } from "./types";
import { CONFIDENCE_STEP, VECTOR_INDEXING_MODE_OPTIONS, AI_MODEL_OPTIONS } from "./types";

interface SearchTabProps extends TabProps {
  onSemanticToggle: (enabled: boolean) => void;
}

export function SearchTab({ settings, onChange, onSemanticToggle }: SearchTabProps) {
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

      {/* 시맨틱 검색 */}
      <div
        className="rounded-lg p-3"
        style={{
          backgroundColor: (settings.semantic_search_enabled ?? false) ? "rgba(245, 158, 11, 0.08)" : "var(--color-bg-secondary)",
          border: `1px solid ${(settings.semantic_search_enabled ?? false) ? "rgba(245, 158, 11, 0.3)" : "var(--color-border)"}`,
        }}
      >
        <SettingsToggle
          label="시맨틱 검색 (고급)"
          description="AI 의미 검색 · ONNX 모델 500MB 다운로드 필요 (베타)"
          checked={settings.semantic_search_enabled ?? false}
          onChange={onSemanticToggle}
        />
        {(settings.semantic_search_enabled ?? false) && (
          <div className="mt-2 space-y-2">
            <p
              className="text-xs px-2 py-1 rounded"
              style={{ backgroundColor: "rgba(245, 158, 11, 0.1)", color: "var(--color-text-secondary)" }}
            >
              메모리 약 1GB 추가 사용. 저사양 PC(4GB RAM)에서는 성능 저하 가능
            </p>
            <div>
              <label className="block text-sm font-medium mb-1" style={{ color: "var(--color-text-secondary)" }}>벡터 인덱싱 모드</label>
              <Dropdown options={VECTOR_INDEXING_MODE_OPTIONS} value={settings.vector_indexing_mode ?? "manual"} onChange={(value) => onChange("vector_indexing_mode", value as Settings["vector_indexing_mode"])} placeholder="모드 선택" />
            </div>
          </div>
        )}
      </div>

      {/* AI 검색 (Gemini RAG) */}
      <div
        className="rounded-lg p-3"
        style={{
          backgroundColor: (settings.ai_enabled ?? false) ? "rgba(59, 130, 246, 0.08)" : "var(--color-bg-secondary)",
          border: `1px solid ${(settings.ai_enabled ?? false) ? "rgba(59, 130, 246, 0.3)" : "var(--color-border)"}`,
        }}
      >
        <SettingsToggle
          label="AI 답변"
          description="검색 결과를 Gemini AI가 분석하여 답변 생성 (API 키 필요)"
          checked={settings.ai_enabled ?? false}
          onChange={(v) => onChange("ai_enabled", v)}
        />
        {(settings.ai_enabled ?? false) && (
          <div className="mt-2 space-y-2">
            {/* API 키 */}
            <div>
              <label className="block text-sm font-medium mb-1" style={{ color: "var(--color-text-secondary)" }}>
                Gemini API 키
                <InfoTooltip
                  content="Google AI Studio에서 무료 발급 가능"
                  maxWidth={200}
                />
              </label>
              <input
                type="password"
                className="w-full rounded-md px-3 py-1.5 text-xs font-mono"
                style={{
                  backgroundColor: "var(--color-bg-primary)",
                  color: "var(--color-text-primary)",
                  border: "1px solid var(--color-border)",
                }}
                value={settings.ai_api_key ?? ""}
                onChange={(e) => onChange("ai_api_key", e.target.value || undefined)}
                placeholder="AIzaSy..."
              />
            </div>
            {/* 모델 선택 */}
            <div>
              <label className="block text-sm font-medium mb-1" style={{ color: "var(--color-text-secondary)" }}>모델</label>
              <Dropdown
                options={AI_MODEL_OPTIONS}
                value={settings.ai_model ?? "gemini-3.1-flash-lite-preview"}
                onChange={(value) => onChange("ai_model", value)}
                placeholder="모델 선택"
              />
            </div>
            {/* 온도 */}
            <div>
              <label className="flex items-center text-sm font-medium mb-1" style={{ color: "var(--color-text-secondary)" }}>
                창의성 (Temperature)
              </label>
              <div className="flex items-center gap-3">
                <input
                  type="range"
                  min={0}
                  max={2}
                  step={0.1}
                  value={settings.ai_temperature ?? 0.2}
                  onChange={(e) => onChange("ai_temperature", Number(e.target.value))}
                  className="flex-1 accent-[var(--color-accent)]"
                />
                <div className="min-w-[32px] text-sm font-semibold text-right" style={{ color: "var(--color-text-primary)" }}>
                  {(settings.ai_temperature ?? 0.2).toFixed(1)}
                </div>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
