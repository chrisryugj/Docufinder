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
  { value: "keyword", label: "키워드 검색" },
  { value: "semantic", label: "의미 검색" },
  { value: "hybrid", label: "하이브리드 (권장)" },
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
];

const VIEW_DENSITY_OPTIONS = [
  { value: "normal", label: "기본 (넓게)" },
  { value: "compact", label: "컴팩트 (좁게)" },
];

const CONFIDENCE_STEP = 5;

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
