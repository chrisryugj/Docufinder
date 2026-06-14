import { useState, useEffect, useRef } from "react";
import { invokeWithTimeout, IPC_TIMEOUT } from "../../utils/invokeWithTimeout";
import { ask } from "@tauri-apps/plugin-dialog";
import { Modal } from "../ui/Modal";
import { Button } from "../ui/Button";
import type { Settings } from "../../types/settings";
import { getErrorMessage } from "../../types/error";
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
  const [error, setError] = useState<string | null>(null);
  // 자동 저장 성공 시 잠깐 표시되는 조용한 '저장됨' 인디케이터
  const [savedVisible, setSavedVisible] = useState(false);
  const savedIndicatorTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [activeTab, setActiveTab] = useState<SettingsTab>("general");
  const originalDataRootRef = useRef<string | undefined>(undefined);

  useEffect(() => {
    if (!isOpen) return;

    const loadSettings = async () => {
      setIsLoading(true);
      setError(null);
      setSavedVisible(false);
      try {
        const result = await invokeWithTimeout<Settings>("get_settings", undefined, IPC_TIMEOUT.SETTINGS);
        originalDataRootRef.current = result.data_root;
        setSettings(result);
      } catch (err) {
        setError(`설정을 불러올 수 없습니다: ${getErrorMessage(err)}`);
      } finally {
        setIsLoading(false);
      }
    };

    loadSettings();
  }, [isOpen]);

  // 닫기 — 모든 변경은 자동 저장으로 이미 반영됨. 데이터 저장 경로가
  // 바뀐 경우에만 재시작 안내 후 닫는다.
  const handleClose = async () => {
    if (settings && settings.data_root !== originalDataRootRef.current) {
      await ask(
        "데이터 저장 경로가 변경되었습니다.\n변경 사항을 적용하려면 앱을 재시작해주세요.",
        { title: "재시작 필요", kind: "info", okLabel: "확인" }
      );
    }
    onClose();
  };

  // 모든 변경 즉시 적용 모델 — 별도 "저장" 버튼 없이 디바운스 자동 저장 (300ms).
  // close_to_tray 같은 시스템 토글도 앱 종료 전 백엔드에 반영된다.
  const autosaveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const handleChange = <K extends keyof Settings>(key: K, value: Settings[K]) => {
    // functional update: 같은 틱에 연속 호출돼도 stale 상태로 덮어쓰지 않게.
    // (e.g. 트레이 최소화 토글이 start_minimized 까지 동시 변경할 때 꺼지지 않던 버그)
    setSettings((prev) => {
      const next = prev ? { ...prev, [key]: value } : prev;
      if (next) {
        if (autosaveTimerRef.current) clearTimeout(autosaveTimerRef.current);
        autosaveTimerRef.current = setTimeout(() => {
          invokeWithTimeout("update_settings", { settings: next }, IPC_TIMEOUT.SETTINGS)
            .then(() => {
              onSettingsSaved?.(next);
              setError(null);
              // 토스트 대신 모달 내 조용한 '저장됨' 인디케이터
              setSavedVisible(true);
              if (savedIndicatorTimerRef.current) clearTimeout(savedIndicatorTimerRef.current);
              savedIndicatorTimerRef.current = setTimeout(() => setSavedVisible(false), 2000);
            })
            .catch((err) => {
              // 명시 저장 버튼이 없으므로 실패는 모달 에러 배너로 노출
              setError(`설정 저장에 실패했습니다: ${getErrorMessage(err)}`);
            });
        }, 300);
      }
      return next;
    });

    if (key === "theme" && onThemeChange) {
      onThemeChange(value as Settings["theme"]);
    }
  };

  // 모달 unmount 시 디바운스/인디케이터 타이머 정리
  useEffect(() => {
    return () => {
      if (autosaveTimerRef.current) clearTimeout(autosaveTimerRef.current);
      if (savedIndicatorTimerRef.current) clearTimeout(savedIndicatorTimerRef.current);
    };
  }, []);


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
      onClose={handleClose}
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
        <div className="flex items-center justify-between">
          {/* 변경 즉시 자동 저장 — 조용한 '저장됨' 인디케이터 */}
          <div
            aria-live="polite"
            className="flex items-center gap-1.5 text-xs"
            style={{ color: "var(--color-success)" }}
          >
            {savedVisible && (
              <>
                <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
                </svg>
                저장됨
              </>
            )}
          </div>
          <Button variant="secondary" onClick={handleClose}>
            닫기
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
