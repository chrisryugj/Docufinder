import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Dropdown } from "../../ui/Dropdown";
import { SettingsToggle } from "../SettingsToggle";
import type { TabProps } from "./types";
import { AI_MODEL_OPTIONS } from "./types";

export function AiTab({ settings, onChange }: TabProps) {
  const [showApiKey, setShowApiKey] = useState(false);

  return (
    <div className="space-y-4">
      {/* AI 활성화 토글 */}
      <SettingsToggle
        label="AI 기능 활성화"
        description="문서 QA(RAG), AI 요약 등 AI 기반 기능을 사용합니다"
        checked={settings.ai_enabled}
        onChange={(v) => onChange("ai_enabled", v)}
      />

      {/* AI 비활성화 시 나머지 UI 숨김 */}
      {settings.ai_enabled && (
        <>
          {/* API 키 */}
          <div>
            <label className="block text-sm font-medium mb-1" style={{ color: "var(--color-text-secondary)" }}>
              Gemini API 키
            </label>
            <div className="flex items-center gap-2">
              <input
                type={showApiKey ? "text" : "password"}
                value={settings.ai_api_key || ""}
                onChange={(e) => onChange("ai_api_key", e.target.value || undefined)}
                placeholder="AIza..."
                className="flex-1 px-3 py-1.5 rounded text-sm border focus:outline-none focus:ring-1 focus:ring-[var(--color-accent)]"
                style={{
                  backgroundColor: "var(--color-bg-tertiary)",
                  borderColor: "var(--color-border)",
                  color: "var(--color-text-primary)",
                }}
              />
              <button
                type="button"
                onClick={() => setShowApiKey(!showApiKey)}
                className="px-2 py-1.5 text-xs rounded border hover:bg-[var(--color-bg-tertiary)] transition-colors"
                style={{
                  borderColor: "var(--color-border)",
                  color: "var(--color-text-secondary)",
                }}
              >
                {showApiKey ? "숨기기" : "보기"}
              </button>
            </div>
            <p className="text-xs mt-1" style={{ color: "var(--color-text-muted)" }}>
              <a
                href="#"
                onClick={(e) => {
                  e.preventDefault();
                  invoke("open_url", { url: "https://aistudio.google.com/apikey" });
                }}
                className="text-[var(--color-accent)] hover:underline"
              >
                Google AI Studio
              </a>
              에서 무료 API 키를 발급받을 수 있습니다.
            </p>
          </div>

          {/* 모델 선택 */}
          <div>
            <label className="block text-sm font-medium mb-1" style={{ color: "var(--color-text-secondary)" }}>
              AI 모델
            </label>
            <Dropdown
              options={AI_MODEL_OPTIONS}
              value={settings.ai_model}
              onChange={(value) => onChange("ai_model", value)}
              placeholder="모델 선택"
            />
          </div>

          {/* 온도 + 최대 토큰 (2열) */}
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="block text-sm font-medium mb-1" style={{ color: "var(--color-text-secondary)" }}>
                온도 ({settings.ai_temperature.toFixed(1)})
              </label>
              <input
                type="range"
                min="0"
                max="2"
                step="0.1"
                value={settings.ai_temperature}
                onChange={(e) => onChange("ai_temperature", parseFloat(e.target.value))}
                className="w-full accent-[var(--color-accent)]"
              />
              <div className="flex justify-between text-[10px]" style={{ color: "var(--color-text-muted)" }}>
                <span>정확</span>
                <span>창의적</span>
              </div>
            </div>
            <div>
              <label className="block text-sm font-medium mb-1" style={{ color: "var(--color-text-secondary)" }}>
                최대 토큰
              </label>
              <input
                type="number"
                min="256"
                max="8192"
                step="256"
                value={settings.ai_max_tokens}
                onChange={(e) => onChange("ai_max_tokens", parseInt(e.target.value) || 2048)}
                className="w-full px-3 py-1.5 rounded text-sm border focus:outline-none focus:ring-1 focus:ring-[var(--color-accent)]"
                style={{
                  backgroundColor: "var(--color-bg-tertiary)",
                  borderColor: "var(--color-border)",
                  color: "var(--color-text-primary)",
                }}
              />
            </div>
          </div>

          {/* 경고 */}
          <div
            className="flex items-start gap-2 px-3 py-2 rounded text-xs"
            style={{
              backgroundColor: "rgba(234, 179, 8, 0.1)",
              border: "1px solid rgba(234, 179, 8, 0.2)",
              color: "var(--color-text-secondary)",
            }}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#eab308" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="mt-0.5 flex-shrink-0">
              <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" />
              <line x1="12" y1="9" x2="12" y2="13" />
              <line x1="12" y1="17" x2="12.01" y2="17" />
            </svg>
            <span>
              AI 기능 사용 시 문서 내용의 일부가 Google Gemini API로 전송됩니다.
              기밀 문서를 다루는 경우 주의하세요.
            </span>
          </div>
        </>
      )}
    </div>
  );
}
