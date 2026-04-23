import { useState, useEffect, useRef } from "react";
import { invokeWithTimeout, IPC_TIMEOUT } from "../../utils/invokeWithTimeout";
import { ask } from "@tauri-apps/plugin-dialog";
import { Modal } from "../ui/Modal";
import { Button } from "../ui/Button";
import type { Settings } from "../../types/settings";
import { GeneralTab, SearchTab, AiTab, SystemTab, DiagnosticsTab } from "./tabs";

interface SettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
  onThemeChange?: (theme: Settings["theme"]) => void;
  onSettingsSaved?: (settings: Settings) => void;
  onClearData?: () => Promise<void>;
  onAutoIndexAllDrives?: () => Promise<void>;
}

type SettingsTab = "general" | "search" | "ai" | "system" | "diagnostics";

const TABS: { id: SettingsTab; label: string }[] = [
  { id: "general", label: "일반" },
  { id: "search", label: "검색" },
  { id: "ai", label: "AI" },
  { id: "system", label: "시스템" },
  { id: "diagnostics", label: "진단" },
];

export function SettingsModal({ isOpen, onClose, onThemeChange, onSettingsSaved, onClearData, onAutoIndexAllDrives }: SettingsModalProps) {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<SettingsTab>("general");
  const originalDataRootRef = useRef<string | undefined>(undefined);

  useEffect(() => {
    if (!isOpen) return;

    const loadSettings = async () => {
      setIsLoading(true);
      setError(null);
      try {
        const result = await invokeWithTimeout<Settings>("get_settings", undefined, IPC_TIMEOUT.SETTINGS);
        originalDataRootRef.current = result.data_root;
        setSettings(result);
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        setError(`설정을 불러올 수 없습니다: ${message}`);
      } finally {
        setIsLoading(false);
      }
    };

    loadSettings();
  }, [isOpen]);

  const saveSettings = async () => {
    if (!settings) return;

    setIsSaving(true);
    setError(null);
    try {
      await invokeWithTimeout("update_settings", { settings }, IPC_TIMEOUT.SETTINGS);
      onSettingsSaved?.(settings);

      if (settings.data_root !== originalDataRootRef.current) {
        await ask(
          "데이터 저장 경로가 변경되었습니다.\n변경 사항을 적용하려면 앱을 재시작해주세요.",
          { title: "재시작 필요", kind: "info", okLabel: "확인" }
        );
      }

      onClose();
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setError(`설정 저장에 실패했습니다: ${message}`);
    } finally {
      setIsSaving(false);
    }
  };

  const handleChange = <K extends keyof Settings>(key: K, value: Settings[K]) => {
    // functional update: 같은 틱에 연속 호출돼도 stale 상태로 덮어쓰지 않게.
    // (e.g. 트레이 최소화 토글이 start_minimized 까지 동시 변경할 때 꺼지지 않던 버그)
    setSettings((prev) => (prev ? { ...prev, [key]: value } : prev));

    if (key === "theme" && onThemeChange) {
      onThemeChange(value as Settings["theme"]);
    }
  };


  if (isLoading) {
    return (
      <Modal isOpen={isOpen} onClose={onClose} title="설정" size="lg">
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
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title="설정"
      size="lg"
      headerExtra={
        <div className="flex items-center gap-0" role="tablist" aria-label="설정 탭">
          {TABS.map((tab) => (
            <button
              key={tab.id}
              id={`settings-tab-${tab.id}`}
              role="tab"
              aria-selected={activeTab === tab.id}
              aria-controls={`settings-panel-${tab.id}`}
              onClick={() => setActiveTab(tab.id)}
              className={`settings-tab-btn px-2.5 py-1 text-sm rounded-md ${activeTab === tab.id ? "active" : ""}`}
              style={{
                color: activeTab === tab.id ? "var(--color-accent)" : "var(--color-text-muted)",
                fontWeight: activeTab === tab.id ? 600 : 400,
                backgroundColor: activeTab === tab.id ? "var(--color-accent-light)" : "transparent",
              }}
            >
              {tab.label}
            </button>
          ))}
        </div>
      }
      footer={
        <div className="flex justify-end gap-3">
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
      }
    >
      {error && (
        <div
          className="mb-3 p-2.5 rounded-md text-xs"
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
        <div className="space-y-3">
          {activeTab === "general" && (
            <div role="tabpanel" id="settings-panel-general" aria-labelledby="settings-tab-general">
              <GeneralTab settings={settings} onChange={handleChange} />
            </div>
          )}
          {activeTab === "search" && (
            <div role="tabpanel" id="settings-panel-search" aria-labelledby="settings-tab-search">
              <SearchTab settings={settings} onChange={handleChange} />
            </div>
          )}
          {activeTab === "ai" && (
            <div role="tabpanel" id="settings-panel-ai" aria-labelledby="settings-tab-ai">
              <AiTab settings={settings} onChange={handleChange} />
            </div>
          )}
          {activeTab === "system" && (
            <div role="tabpanel" id="settings-panel-system" aria-labelledby="settings-tab-system">
              <SystemTab
                settings={settings}
                onChange={handleChange}
                setError={setError}
                onClose={onClose}
                onClearData={onClearData}
                onAutoIndexAllDrives={onAutoIndexAllDrives}
              />
            </div>
          )}
          {activeTab === "diagnostics" && (
            <div role="tabpanel" id="settings-panel-diagnostics" aria-labelledby="settings-tab-diagnostics">
              <DiagnosticsTab
                settings={settings}
                onChange={handleChange}
                setError={setError}
              />
            </div>
          )}
        </div>
      )}
    </Modal>
  );
}
