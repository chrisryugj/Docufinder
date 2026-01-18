import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Modal } from "../ui/Modal";
import { Button } from "../ui/Button";
import { Dropdown } from "../ui/Dropdown";
import type { Settings } from "../../types/settings";

interface SettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
  onThemeChange?: (theme: Settings["theme"]) => void;
  onSettingsSaved?: (settings: Settings) => void;
}

const SEARCH_MODE_OPTIONS = [
  { value: "hybrid", label: "하이브리드 (권장)" },
  { value: "keyword", label: "키워드 검색" },
  { value: "semantic", label: "의미 검색" },
  { value: "filename", label: "파일명 검색" },
];

const THEME_OPTIONS = [
  { value: "light", label: "라이트 모드" },
  { value: "dark", label: "다크 모드" },
  { value: "system", label: "시스템 설정" },
];

const MAX_RESULTS_OPTIONS = [
  { value: "20", label: "20개" },
  { value: "50", label: "50개 (기본)" },
  { value: "100", label: "100개" },
  { value: "200", label: "200개" },
  { value: "500", label: "500개" },
  { value: "1000", label: "1000개" },
];

const VIEW_DENSITY_OPTIONS = [
  { value: "normal", label: "기본 (넓게)" },
  { value: "compact", label: "컴팩트 (좁게)" },
];

const CONFIDENCE_STEP = 5;

// 하이라이트 색상 프리셋 (라이트/다크 모드 각각)
const HIGHLIGHT_COLOR_PRESETS = [
  { value: "", label: "기본", light: "#fde047", dark: "#854d0e" },
  { value: "#fbbf24", label: "앰버", light: "#fbbf24", dark: "#b45309" },
  { value: "#fb923c", label: "오렌지", light: "#fb923c", dark: "#c2410c" },
  { value: "#f87171", label: "레드", light: "#f87171", dark: "#b91c1c" },
  { value: "#c084fc", label: "퍼플", light: "#c084fc", dark: "#7c3aed" },
  { value: "#60a5fa", label: "블루", light: "#60a5fa", dark: "#1d4ed8" },
  { value: "#34d399", label: "그린", light: "#34d399", dark: "#059669" },
  { value: "#2dd4bf", label: "틸", light: "#2dd4bf", dark: "#0d9488" },
];

export function SettingsModal({ isOpen, onClose, onThemeChange, onSettingsSaved }: SettingsModalProps) {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // 설정 로드
  useEffect(() => {
    if (isOpen) {
      loadSettings();
    }
  }, [isOpen]);

  const loadSettings = async () => {
    setIsLoading(true);
    setError(null);
    try {
      const result = await invoke<Settings>("get_settings");
      setSettings(result);
    } catch (err) {
      setError(`설정 로드 실패: ${err}`);
    } finally {
      setIsLoading(false);
    }
  };

  const saveSettings = async () => {
    if (!settings) return;

    setIsSaving(true);
    setError(null);
    try {
      await invoke("update_settings", { settings });
      onSettingsSaved?.(settings);
      onClose();
    } catch (err) {
      setError(`설정 저장 실패: ${err}`);
    } finally {
      setIsSaving(false);
    }
  };

  const handleChange = <K extends keyof Settings>(key: K, value: Settings[K]) => {
    if (settings) {
      setSettings({ ...settings, [key]: value });

      // 테마 변경 시 즉시 적용
      if (key === "theme" && onThemeChange) {
        onThemeChange(value as Settings["theme"]);
      }
    }
  };

  if (isLoading) {
    return (
      <Modal isOpen={isOpen} onClose={onClose} title="설정">
        <div className="flex justify-center py-8">
          <div
            className="animate-spin rounded-full h-8 w-8 border-2"
            style={{
              borderColor: "var(--color-border)",
              borderTopColor: "var(--color-accent)",
            }}
          />
        </div>
      </Modal>
    );
  }

  return (
    <Modal isOpen={isOpen} onClose={onClose} title="설정">
      {error && (
        <div
          className="mb-4 p-3 rounded-lg text-sm"
          style={{
            backgroundColor: "rgba(239, 68, 68, 0.1)",
            border: "1px solid rgba(239, 68, 68, 0.3)",
            color: "var(--color-error)",
          }}
        >
          {error}
        </div>
      )}

      {settings && (
        <div className="space-y-5">
          {/* 검색 모드 */}
          <div>
            <label
              className="block text-sm font-medium mb-2"
              style={{ color: "var(--color-text-secondary)" }}
            >
              기본 검색 모드
            </label>
            <Dropdown
              options={SEARCH_MODE_OPTIONS}
              value={settings.search_mode}
              onChange={(value) => handleChange("search_mode", value as Settings["search_mode"])}
              placeholder="검색 모드 선택"
            />
            <p className="mt-1.5 text-xs" style={{ color: "var(--color-text-muted)" }}>
              하이브리드: 키워드 + 의미 검색 결합 (모델 필요)
            </p>
          </div>

          {/* 최대 결과 수 */}
          <div>
            <label
              className="block text-sm font-medium mb-2"
              style={{ color: "var(--color-text-secondary)" }}
            >
              최대 검색 결과
            </label>
            <Dropdown
              options={MAX_RESULTS_OPTIONS}
              value={String(settings.max_results)}
              onChange={(value) => handleChange("max_results", parseInt(value))}
              placeholder="결과 수 선택"
            />
          </div>

          {/* 최소 신뢰도 */}
          <div>
            <label
              className="block text-sm font-medium mb-2"
              style={{ color: "var(--color-text-secondary)" }}
            >
              최소 신뢰도
            </label>
            <div className="flex items-center gap-4">
              <input
                type="range"
                min={0}
                max={100}
                step={CONFIDENCE_STEP}
                value={settings.min_confidence}
                onChange={(e) => handleChange("min_confidence", Number(e.target.value))}
                className="flex-1 accent-blue-500"
                aria-label="최소 신뢰도 설정"
              />
              <div
                className="min-w-[48px] text-sm font-semibold text-right"
                style={{ color: "var(--color-text-primary)" }}
              >
                {settings.min_confidence}%
              </div>
            </div>
            <p className="mt-1.5 text-xs" style={{ color: "var(--color-text-muted)" }}>
              설정 값 미만의 결과는 표시하지 않습니다
            </p>
          </div>

          {/* 테마 */}
          <div>
            <label
              className="block text-sm font-medium mb-2"
              style={{ color: "var(--color-text-secondary)" }}
            >
              테마
            </label>
            <Dropdown
              options={THEME_OPTIONS}
              value={settings.theme}
              onChange={(value) => handleChange("theme", value as Settings["theme"])}
              placeholder="테마 선택"
            />
          </div>

          {/* 결과 보기 밀도 */}
          <div>
            <label
              className="block text-sm font-medium mb-2"
              style={{ color: "var(--color-text-secondary)" }}
            >
              검색 결과 보기
            </label>
            <Dropdown
              options={VIEW_DENSITY_OPTIONS}
              value={settings.view_density ?? "normal"}
              onChange={(value) => handleChange("view_density", value as Settings["view_density"])}
              placeholder="보기 모드 선택"
            />
            <p className="mt-1.5 text-xs" style={{ color: "var(--color-text-muted)" }}>
              컴팩트: 더 많은 결과를 한 화면에 표시
            </p>
          </div>

          {/* 하위폴더 포함 */}
          <div className="flex items-center justify-between">
            <div>
              <label
                className="text-sm font-medium"
                style={{ color: "var(--color-text-secondary)" }}
              >
                하위폴더 포함
              </label>
              <p className="mt-0.5 text-xs" style={{ color: "var(--color-text-muted)" }}>
                폴더 추가 시 하위폴더도 함께 인덱싱
              </p>
            </div>
            <button
              type="button"
              role="switch"
              aria-checked={settings.include_subfolders ?? true}
              onClick={() => handleChange("include_subfolders", !(settings.include_subfolders ?? true))}
              className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
                (settings.include_subfolders ?? true)
                  ? "bg-blue-500"
                  : "bg-gray-600"
              }`}
            >
              <span
                className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                  (settings.include_subfolders ?? true) ? "translate-x-6" : "translate-x-1"
                }`}
              />
            </button>
          </div>

          {/* 시스템 트레이 섹션 구분선 */}
          <div
            className="border-t pt-4"
            style={{ borderColor: "var(--color-border)" }}
          >
            <h3
              className="text-sm font-medium mb-3"
              style={{ color: "var(--color-text-primary)" }}
            >
              시스템 트레이
            </h3>
          </div>

          {/* 윈도우 시작 시 자동 실행 */}
          <div className="flex items-center justify-between">
            <div>
              <label
                className="text-sm font-medium"
                style={{ color: "var(--color-text-secondary)" }}
              >
                Windows 시작 시 자동 실행
              </label>
              <p className="mt-0.5 text-xs" style={{ color: "var(--color-text-muted)" }}>
                컴퓨터 부팅 시 자동으로 시작
              </p>
            </div>
            <button
              type="button"
              role="switch"
              aria-checked={settings.auto_start ?? false}
              onClick={() => handleChange("auto_start", !(settings.auto_start ?? false))}
              className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
                (settings.auto_start ?? false)
                  ? "bg-blue-500"
                  : "bg-gray-600"
              }`}
            >
              <span
                className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                  (settings.auto_start ?? false) ? "translate-x-6" : "translate-x-1"
                }`}
              />
            </button>
          </div>

          {/* 시작 시 최소화 */}
          <div className="flex items-center justify-between">
            <div>
              <label
                className="text-sm font-medium"
                style={{ color: "var(--color-text-secondary)" }}
              >
                시작 시 트레이로 최소화
              </label>
              <p className="mt-0.5 text-xs" style={{ color: "var(--color-text-muted)" }}>
                앱 시작 시 트레이 아이콘으로 숨김
              </p>
            </div>
            <button
              type="button"
              role="switch"
              aria-checked={settings.start_minimized ?? false}
              onClick={() => handleChange("start_minimized", !(settings.start_minimized ?? false))}
              className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
                (settings.start_minimized ?? false)
                  ? "bg-blue-500"
                  : "bg-gray-600"
              }`}
            >
              <span
                className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                  (settings.start_minimized ?? false) ? "translate-x-6" : "translate-x-1"
                }`}
              />
            </button>
          </div>

          {/* 하이라이트 색상 섹션 */}
          <div
            className="border-t pt-4"
            style={{ borderColor: "var(--color-border)" }}
          >
            <h3
              className="text-sm font-medium mb-3"
              style={{ color: "var(--color-text-primary)" }}
            >
              하이라이트 색상
            </h3>
          </div>

          {/* 파일명 하이라이트 색상 */}
          <div>
            <label
              className="block text-sm font-medium mb-2"
              style={{ color: "var(--color-text-secondary)" }}
            >
              파일명 하이라이트
            </label>
            <div className="flex flex-wrap gap-2">
              {HIGHLIGHT_COLOR_PRESETS.map((preset) => (
                <button
                  key={preset.value || "default"}
                  type="button"
                  onClick={() => handleChange("highlight_filename_color", preset.value || undefined)}
                  className={`w-8 h-8 rounded-lg border-2 transition-all ${
                    (settings.highlight_filename_color || "") === preset.value
                      ? "ring-2 ring-offset-2"
                      : ""
                  }`}
                  style={{
                    backgroundColor: preset.light,
                    borderColor: (settings.highlight_filename_color || "") === preset.value
                      ? "var(--color-accent)"
                      : "var(--color-border)",
                    ringColor: "var(--color-accent)",
                  }}
                  title={preset.label}
                  aria-label={`${preset.label} 색상 선택`}
                />
              ))}
            </div>
            <p className="mt-1.5 text-xs" style={{ color: "var(--color-text-muted)" }}>
              파일명 검색 결과에서 매칭된 글자 강조 색상
            </p>
          </div>

          {/* 문서 내용 하이라이트 색상 */}
          <div>
            <label
              className="block text-sm font-medium mb-2"
              style={{ color: "var(--color-text-secondary)" }}
            >
              문서 내용 하이라이트
            </label>
            <div className="flex flex-wrap gap-2">
              {HIGHLIGHT_COLOR_PRESETS.map((preset) => (
                <button
                  key={preset.value || "default"}
                  type="button"
                  onClick={() => handleChange("highlight_content_color", preset.value || undefined)}
                  className={`w-8 h-8 rounded-lg border-2 transition-all ${
                    (settings.highlight_content_color || "") === preset.value
                      ? "ring-2 ring-offset-2"
                      : ""
                  }`}
                  style={{
                    backgroundColor: preset.light,
                    borderColor: (settings.highlight_content_color || "") === preset.value
                      ? "var(--color-accent)"
                      : "var(--color-border)",
                    ringColor: "var(--color-accent)",
                  }}
                  title={preset.label}
                  aria-label={`${preset.label} 색상 선택`}
                />
              ))}
            </div>
            <p className="mt-1.5 text-xs" style={{ color: "var(--color-text-muted)" }}>
              문서 검색 결과에서 매칭된 키워드 강조 색상
            </p>
          </div>

          {/* 버튼 */}
          <div
            className="flex justify-end gap-3 pt-4 border-t"
            style={{ borderColor: "var(--color-border)" }}
          >
            <Button variant="ghost" onClick={onClose}>
              취소
            </Button>
            <Button
              onClick={saveSettings}
              isLoading={isSaving}
              disabled={isSaving}
            >
              저장
            </Button>
          </div>
        </div>
      )}
    </Modal>
  );
}
