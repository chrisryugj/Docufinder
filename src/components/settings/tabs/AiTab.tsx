import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Dropdown } from "../../ui/Dropdown";
import { SettingsToggle } from "../SettingsToggle";
import type { TabProps } from "./types";
import { AI_MODEL_OPTIONS, AI_PROVIDER_OPTIONS } from "./types";

export function AiTab({ settings, onChange }: TabProps) {
  const [showApiKey, setShowApiKey] = useState(false);

  const provider = settings.ai_provider ?? "gemini";
  const isOpenAi = provider === "open_ai";

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
          {/* Provider 선택 */}
          <div>
            <label className="block text-sm font-medium mb-1" style={{ color: "var(--color-text-secondary)" }}>
              LLM Provider
            </label>
            <Dropdown
              options={AI_PROVIDER_OPTIONS}
              value={provider}
              onChange={(value) => onChange("ai_provider", value as "gemini" | "open_ai")}
              placeholder="provider 선택"
            />
            <p className="text-[10px] mt-1 leading-snug" style={{ color: "var(--color-text-muted)" }}>
              {isOpenAi
                ? "OpenAI Chat Completions 호환 endpoint — vLLM · Ollama · LiteLLM · 사내 LLM (qwen 등)"
                : "Google Gemini 공식 API — 무료 키 발급 가능"}
            </p>
          </div>

          {/* OpenAI: Base URL */}
          {isOpenAi && (
            <div>
              <label className="block text-sm font-medium mb-1" style={{ color: "var(--color-text-secondary)" }}>
                Base URL
              </label>
              <input
                type="text"
                value={settings.ai_base_url ?? ""}
                onChange={(e) => onChange("ai_base_url", e.target.value || undefined)}
                placeholder="예: http://192.168.1.50:8000 또는 https://api.together.xyz"
                className="w-full px-3 py-1.5 rounded text-sm border focus:outline-none focus:ring-1 focus:ring-[var(--color-accent)]"
                style={{
                  backgroundColor: "var(--color-bg-tertiary)",
                  borderColor: "var(--color-border)",
                  color: "var(--color-text-primary)",
                }}
              />
              <p className="text-[10px] mt-1 leading-snug" style={{ color: "var(--color-text-muted)" }}>
                <code>/v1/chat/completions</code> 가 자동으로 붙습니다. <code>/v1</code> 까지 포함된 URL 도 동작.
              </p>
            </div>
          )}

          {/* API 키 */}
          {(() => {
            const storedKey = settings.ai_api_key || "";
            const isMasked = storedKey.startsWith("***") && storedKey.length <= 7;
            const lastFour = isMasked ? storedKey.slice(3) : "";
            const inputValue = isMasked ? "" : storedKey;
            const keyLabel = isOpenAi ? "API 키" : "Gemini API 키";
            const keyPlaceholder = isOpenAi
              ? "사내 LLM API 키 (없으면 임의 문자열도 OK — 일부 backend는 키 검사 안 함)"
              : "AIza...";

            return (
            <div>
              <label className="flex items-baseline gap-2 text-sm font-medium mb-1" style={{ color: "var(--color-text-secondary)" }}>
                <span>{keyLabel}</span>
                {isMasked && (
                  <span className="text-[10px] font-normal" style={{ color: "var(--color-text-muted)" }}>
                    저장됨 (···{lastFour}) — 바꾸려면 새 키 입력
                  </span>
                )}
              </label>
              <div className="flex items-center gap-2">
                <input
                  type={showApiKey ? "text" : "password"}
                  value={inputValue}
                  onChange={(e) => onChange("ai_api_key", e.target.value || undefined)}
                  placeholder={isMasked ? `●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●${lastFour}` : keyPlaceholder}
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
                  disabled={isMasked && inputValue === ""}
                  className="px-2 py-1.5 text-xs rounded border hover:bg-[var(--color-bg-tertiary)] transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
                  style={{
                    borderColor: "var(--color-border)",
                    color: "var(--color-text-secondary)",
                  }}
                >
                  {showApiKey ? "숨기기" : "보기"}
                </button>
              </div>
              {!isOpenAi && (
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
              )}
            </div>
            );
          })()}

          {/* 모델 선택 / 입력 */}
          <div>
            <label className="block text-sm font-medium mb-1" style={{ color: "var(--color-text-secondary)" }}>
              AI 모델
            </label>
            {isOpenAi ? (
              <input
                type="text"
                value={settings.ai_model}
                onChange={(e) => onChange("ai_model", e.target.value)}
                placeholder="예: qwen3-35b-a3b, llama-3.3-70b-instruct, gpt-4o-mini"
                className="w-full px-3 py-1.5 rounded text-sm border focus:outline-none focus:ring-1 focus:ring-[var(--color-accent)]"
                style={{
                  backgroundColor: "var(--color-bg-tertiary)",
                  borderColor: "var(--color-border)",
                  color: "var(--color-text-primary)",
                }}
              />
            ) : (
              <Dropdown
                options={AI_MODEL_OPTIONS}
                value={settings.ai_model}
                onChange={(value) => onChange("ai_model", value)}
                placeholder="모델 선택"
              />
            )}
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
              <p className="text-[10px] mt-1 leading-snug" style={{ color: "var(--color-text-muted)" }}>
                답변의 다양성 · 문서 QA는 <strong style={{ color: "var(--color-text-secondary)" }}>0.1~0.3</strong> 권장 (환각 감소)
              </p>
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
              <p className="text-[10px] mt-1 leading-snug" style={{ color: "var(--color-text-muted)" }}>
                답변 최대 길이 · 일반 <strong style={{ color: "var(--color-text-secondary)" }}>2048</strong>, 상세 요약 4096
              </p>
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
              {isOpenAi ? (
                <>
                  설정한 base URL 의 LLM 서버로 문서 일부가 전송됩니다. 사내 LLM 사용 시 보안 정책을 확인하세요.
                  <br />
                  사외 endpoint (Together · Groq · OpenAI 등) 를 입력했다면 외부 전송임을 명심하세요.
                </>
              ) : (
                <>
                  AI 기능 사용 시 문서 내용의 일부가 Google Gemini API로 전송됩니다.
                  기밀 문서를 다루는 경우 주의하세요.
                  <br />
                  내부망·방화벽 환경에서는 OpenAI 호환 provider 로 전환해 사내 LLM 을 사용할 수 있습니다.
                </>
              )}
            </span>
          </div>
        </>
      )}
    </div>
  );
}
