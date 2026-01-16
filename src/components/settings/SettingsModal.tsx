import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Modal } from "../ui/Modal";
import { Button } from "../ui/Button";
import { Dropdown } from "../ui/Dropdown";

interface Settings {
  search_mode: "keyword" | "semantic" | "hybrid";
  max_results: number;
  chunk_size: number;
  chunk_overlap: number;
  theme: "dark" | "light" | "system";
}

interface SettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
}

const SEARCH_MODE_OPTIONS = [
  { value: "keyword", label: "키워드 검색" },
  { value: "semantic", label: "의미 검색" },
  { value: "hybrid", label: "하이브리드 (권장)" },
];

const THEME_OPTIONS = [
  { value: "dark", label: "다크 모드" },
  { value: "light", label: "라이트 모드" },
  { value: "system", label: "시스템 설정" },
];

const MAX_RESULTS_OPTIONS = [
  { value: "20", label: "20개" },
  { value: "50", label: "50개 (기본)" },
  { value: "100", label: "100개" },
  { value: "200", label: "200개" },
];

export function SettingsModal({ isOpen, onClose }: SettingsModalProps) {
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
    }
  };

  if (isLoading) {
    return (
      <Modal isOpen={isOpen} onClose={onClose} title="설정">
        <div className="flex justify-center py-8">
          <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-500"></div>
        </div>
      </Modal>
    );
  }

  return (
    <Modal isOpen={isOpen} onClose={onClose} title="설정">
      {error && (
        <div className="mb-4 p-3 bg-red-500/10 border border-red-500/30 rounded text-red-400 text-sm">
          {error}
        </div>
      )}

      {settings && (
        <div className="space-y-5">
          {/* 검색 모드 */}
          <div>
            <label className="block text-sm font-medium text-gray-300 mb-2">
              기본 검색 모드
            </label>
            <Dropdown
              options={SEARCH_MODE_OPTIONS}
              value={settings.search_mode}
              onChange={(value) => handleChange("search_mode", value as Settings["search_mode"])}
              placeholder="검색 모드 선택"
            />
            <p className="mt-1 text-xs text-gray-500">
              하이브리드: 키워드 + 의미 검색 결합 (모델 필요)
            </p>
          </div>

          {/* 최대 결과 수 */}
          <div>
            <label className="block text-sm font-medium text-gray-300 mb-2">
              최대 검색 결과
            </label>
            <Dropdown
              options={MAX_RESULTS_OPTIONS}
              value={String(settings.max_results)}
              onChange={(value) => handleChange("max_results", parseInt(value))}
              placeholder="결과 수 선택"
            />
          </div>

          {/* 테마 */}
          <div>
            <label className="block text-sm font-medium text-gray-300 mb-2">
              테마
            </label>
            <Dropdown
              options={THEME_OPTIONS}
              value={settings.theme}
              onChange={(value) => handleChange("theme", value as Settings["theme"])}
              placeholder="테마 선택"
            />
          </div>

          {/* 버튼 */}
          <div className="flex justify-end gap-3 pt-4 border-t border-gray-700">
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
