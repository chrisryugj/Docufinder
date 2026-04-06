import { useState, useEffect, useRef } from "react";
import { invokeWithTimeout, IPC_TIMEOUT } from "../../utils/invokeWithTimeout";
import { ask } from "@tauri-apps/plugin-dialog";
import { Modal } from "../ui/Modal";
import { Button } from "../ui/Button";
import type { Settings } from "../../types/settings";
import { GeneralTab, SearchTab, AiTab, SystemTab } from "./tabs";

interface SettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
  onThemeChange?: (theme: Settings["theme"]) => void;
  onSettingsSaved?: (settings: Settings) => void;
  onClearData?: () => Promise<void>;
  onAutoIndexAllDrives?: () => Promise<void>;
}

type SettingsTab = "general" | "search" | "ai" | "system";

const TABS: { id: SettingsTab; label: string }[] = [
  { id: "general", label: "일반" },
  { id: "search", label: "검색" },
  { id: "ai", label: "AI" },
  { id: "system", label: "시스템" },
];

export function SettingsModal({ isOpen, onClose, onThemeChange, onSettingsSaved, onClearData, onAutoIndexAllDrives }: SettingsModalProps) {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<SettingsTab>("general");
  const originalDataRootRef = useRef<string | undefined>(undefined);
  const [adminCodeInput, setAdminCodeInput] = useState("");
  const [showAdminCodePrompt, setShowAdminCodePrompt] = useState(false);
  const [adminCodeError, setAdminCodeError] = useState<string | null>(null);
  const pendingConfirmRef = useRef<(() => void) | null>(null);

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
    if (!settings) return;

    if (
      key === "search_mode" &&
      (value === "hybrid" || value === "semantic") &&
      !(settings.semantic_search_enabled ?? false)
    ) {
      enableSemanticWithConfirm(() => {
        setSettings((prev) => prev ? { ...prev, [key]: value, semantic_search_enabled: true } : prev);
      });
      return;
    }

    setSettings({ ...settings, [key]: value });

    if (key === "theme" && onThemeChange) {
      onThemeChange(value as Settings["theme"]);
    }
  };

  const enableSemanticWithConfirm = (onConfirm: () => void) => {
    pendingConfirmRef.current = onConfirm;
    setAdminCodeInput("");
    setAdminCodeError(null);
    setShowAdminCodePrompt(true);
  };

  const handleAdminCodeSubmit = async () => {
    if (!adminCodeInput.trim()) return;
    try {
      const isValid = await invokeWithTimeout<boolean>("verify_admin_code", { code: adminCodeInput }, IPC_TIMEOUT.SETTINGS);
      if (!isValid) {
        setAdminCodeError("관리자 코드가 올바르지 않습니다.");
        return;
      }
      setShowAdminCodePrompt(false);
      const confirmed = await ask(
        "시맨틱 검색은 ONNX 모델 다운로드가 필요하며, 추가 디스크 공간과 메모리를 사용합니다.\n활성화하시겠습니까?",
        { title: "시맨틱 검색 활성화", kind: "info", okLabel: "활성화", cancelLabel: "취소" }
      );
      if (confirmed) {
        pendingConfirmRef.current?.();
      }
    } catch {
      setAdminCodeError("검증 중 오류가 발생했습니다.");
    }
  };

  const handleSemanticToggle = (enabled: boolean) => {
    if (!settings) return;
    if (enabled) {
      enableSemanticWithConfirm(() => {
        setSettings((prev) => prev ? { ...prev, semantic_search_enabled: true } : prev);
      });
    } else {
      setSettings({ ...settings, semantic_search_enabled: false });
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
              className="px-2.5 py-1 text-sm transition-colors rounded-md"
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

      {showAdminCodePrompt && (
        <div
          className="mb-3 p-3 rounded-lg border"
          style={{ backgroundColor: "var(--color-bg-secondary)", borderColor: "var(--color-border)" }}
        >
          <label className="block text-xs font-medium mb-1.5" style={{ color: "var(--color-text-primary)" }}>
            시맨틱 검색을 활성화하려면 관리자 코드를 입력하세요.
          </label>
          <div className="flex gap-2">
            <input
              type="password"
              autoFocus
              value={adminCodeInput}
              onChange={(e) => { setAdminCodeInput(e.target.value); setAdminCodeError(null); }}
              onKeyDown={(e) => { if (e.key === "Enter") handleAdminCodeSubmit(); if (e.key === "Escape") setShowAdminCodePrompt(false); }}
              className="flex-1 px-2.5 py-1.5 text-sm rounded-md border outline-none focus:ring-1"
              style={{
                backgroundColor: "var(--color-bg-primary)",
                borderColor: adminCodeError ? "var(--color-error)" : "var(--color-border)",
                color: "var(--color-text-primary)",
              }}
              placeholder="관리자 코드"
            />
            <Button size="sm" onClick={handleAdminCodeSubmit}>확인</Button>
            <Button size="sm" variant="ghost" onClick={() => setShowAdminCodePrompt(false)}>취소</Button>
          </div>
          {adminCodeError && (
            <p className="mt-1 text-xs" style={{ color: "var(--color-error)" }}>{adminCodeError}</p>
          )}
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
              <SearchTab settings={settings} onChange={handleChange} onSemanticToggle={handleSemanticToggle} />
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
        </div>
      )}
    </Modal>
  );
}
