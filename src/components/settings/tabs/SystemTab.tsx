import { useState, useEffect } from "react";
import { ask, open } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { invokeWithTimeout, IPC_TIMEOUT } from "../../../utils/invokeWithTimeout";
import { Button } from "../../ui/Button";
import { Dropdown } from "../../ui/Dropdown";
import { Modal } from "../../ui/Modal";
import { SettingsToggle } from "../SettingsToggle";
import type { Settings } from "../../../types/settings";
import type { TabProps } from "./types";
import { INDEXING_INTENSITY_OPTIONS, MAX_FILE_SIZE_OPTIONS, DEFAULT_MAX_FILE_SIZE_MB } from "./types";

interface SystemTabProps extends TabProps {
  onClose: () => void;
  onClearData?: () => Promise<void>;
  onAutoIndexAllDrives?: () => Promise<void>;
}

const CLEAR_STEP_LABELS: Record<string, string> = {
  "stopping-watchers": "파일 감시 중지 중...",
  "cancelling-indexing": "인덱싱 취소 중...",
  "clearing-vectors": "벡터 데이터 삭제 중...",
  "clearing-database": "데이터베이스 초기화 중...",
  "completed": "완료!",
};

export function SystemTab({ settings, onChange, setError, onClose, onClearData, onAutoIndexAllDrives }: SystemTabProps) {
  const [isAutoIndexing, setIsAutoIndexing] = useState(false);
  const [isClearing, setIsClearing] = useState(false);
  const [clearStep, setClearStep] = useState<string | null>(null);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [agreed, setAgreed] = useState(false);

  useEffect(() => {
    if (!confirmOpen) setAgreed(false);
  }, [confirmOpen]);

  const runClearData = async () => {
    if (!onClearData) return;
    setConfirmOpen(false);
    setIsClearing(true);
    try {
      await onClearData();
      onClose();
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setError?.(`초기화 실패: ${message}`);
    } finally {
      setIsClearing(false);
    }
  };

  useEffect(() => {
    if (!isClearing) return;
    let unlisten: (() => void) | null = null;
    listen<string>("clear-data-progress", (event) => {
      setClearStep(event.payload);
    }).then((fn) => { unlisten = fn; });
    return () => { unlisten?.(); setClearStep(null); };
  }, [isClearing]);

  return (
    <div className="space-y-3">
      {/* 시작 옵션 (3열) */}
      <div className="grid grid-cols-3 gap-x-4">
        <SettingsToggle
          label="자동 실행"
          description="Windows 시작 시 자동 실행"
          checked={settings.auto_start ?? false}
          onChange={(v) => onChange("auto_start", v)}
        />
        <SettingsToggle
          label="트레이 최소화"
          description="X 버튼 시 종료 대신 트레이"
          checked={settings.close_to_tray ?? false}
          onChange={(v) => {
            onChange("close_to_tray", v);
            if (!v) onChange("start_minimized", false);
          }}
        />
        <SettingsToggle
          label="시작 시 숨김"
          description="시작 시 트레이로 최소화"
          checked={settings.start_minimized ?? false}
          onChange={(v) => onChange("start_minimized", v)}
          disabled={!settings.close_to_tray}
        />
      </div>

      {/* 성능 설정 */}
      <div className="border-t pt-3" style={{ borderColor: "var(--color-border)" }}>
        <h3 className="text-sm font-semibold mb-2" style={{ color: "var(--color-text-primary)" }}>성능</h3>
        <div className="grid grid-cols-2 gap-3">
          <div>
            <label className="block text-sm font-medium mb-1" style={{ color: "var(--color-text-secondary)" }}>
              인덱싱 강도
            </label>
            <Dropdown
              options={INDEXING_INTENSITY_OPTIONS}
              value={settings.indexing_intensity ?? "balanced"}
              onChange={(value) => onChange("indexing_intensity", value as Settings["indexing_intensity"])}
              placeholder="강도 선택"
            />
          </div>
          <div>
            <label className="block text-sm font-medium mb-1" style={{ color: "var(--color-text-secondary)" }}>
              최대 파일 크기
            </label>
            <Dropdown
              options={MAX_FILE_SIZE_OPTIONS}
              value={String(settings.max_file_size_mb ?? DEFAULT_MAX_FILE_SIZE_MB)}
              onChange={(value) => onChange("max_file_size_mb", parseInt(value))}
              placeholder="크기 선택"
            />
            <p className="text-[10px] mt-1 leading-snug" style={{ color: "var(--color-text-muted)" }}>
              초과 파일은 인덱싱 스킵 · 큰 값은 메모리/속도 부담
            </p>
          </div>
        </div>
      </div>

      {/* 데이터 관리 */}
      <div className="border-t pt-3" style={{ borderColor: "var(--color-border)" }}>
        <h3 className="text-sm font-semibold mb-2" style={{ color: "var(--color-text-primary)" }}>데이터 관리</h3>
      </div>

      {/* 데이터 저장 경로 */}
      <div>
        <label className="block text-sm font-medium mb-1" style={{ color: "var(--color-text-secondary)" }}>
          데이터 저장 경로
          <span className="font-normal ml-1" style={{ color: "var(--color-text-muted)" }}>(변경 시 재시작 필요)</span>
        </label>
        <div className="flex items-center gap-2">
          <div
            className="flex-1 px-2.5 py-1.5 rounded-lg text-xs truncate"
            style={{
              backgroundColor: "var(--color-bg-primary)",
              border: "1px solid var(--color-border)",
              color: settings.data_root ? "var(--color-text-primary)" : "var(--color-text-muted)",
            }}
            title={settings.data_root || "기본 위치 (AppData)"}
          >
            {settings.data_root || "기본 위치 (AppData)"}
          </div>
          <Button
            variant="ghost"
            size="sm"
            onClick={async () => {
              const selected = await open({
                directory: true,
                multiple: false,
                title: "데이터 저장 폴더 선택",
              });
              if (selected) {
                onChange("data_root", selected as string);
              }
            }}
          >
            변경
          </Button>
          {settings.data_root && (
            <Button variant="ghost" size="sm" onClick={() => onChange("data_root", undefined)}>
              초기화
            </Button>
          )}
        </div>
      </div>

      {/* 액션 버튼 행 */}
      <div className="flex items-center justify-between">
        <div>
          <label className="text-sm font-medium" style={{ color: "var(--color-text-secondary)" }}>로그 폴더</label>
          <p className="text-xs" style={{ color: "var(--color-text-muted)" }}>오류 로그 (7일 보존)</p>
        </div>
        <Button
          variant="ghost"
          size="sm"
          onClick={async () => {
            try {
              await invokeWithTimeout("open_log_dir", undefined, IPC_TIMEOUT.FILE_ACTION);
            } catch (err) {
              const message = err instanceof Error ? err.message : String(err);
              setError?.(`로그 폴더 열기 실패: ${message}`);
            }
          }}
        >
          폴더 열기
        </Button>
      </div>

      {onAutoIndexAllDrives && (
        <div className="flex items-center justify-between">
          <div>
            <label className="text-sm font-medium" style={{ color: "var(--color-text-secondary)" }}>전체 드라이브 인덱싱</label>
            <p className="text-xs" style={{ color: "var(--color-text-muted)" }}>모든 드라이브 스캔 (시스템 폴더 자동 제외)</p>
          </div>
          <Button
            variant="ghost"
            size="sm"
            isLoading={isAutoIndexing}
            disabled={isAutoIndexing}
            onClick={async () => {
              const confirmed = await ask(
                "모든 드라이브를 스캔하여 문서를 인덱싱합니다.\n시스템 폴더(Windows, Program Files 등)는 자동 제외됩니다.\n\n계속하시겠습니까?",
                { title: "전체 드라이브 인덱싱", kind: "info", okLabel: "시작", cancelLabel: "취소" }
              );
              if (confirmed) {
                setIsAutoIndexing(true);
                try {
                  await onAutoIndexAllDrives();
                  onClose();
                } catch (err) {
                  const message = err instanceof Error ? err.message : String(err);
                  setError?.(`인덱싱 실패: ${message}`);
                } finally {
                  setIsAutoIndexing(false);
                }
              }
            }}
          >
            시작
          </Button>
        </div>
      )}

      <div className="flex items-center justify-between">
        <div>
          <label className="text-sm font-medium" style={{ color: "var(--color-text-secondary)" }}>모든 데이터 초기화</label>
          {isClearing && clearStep ? (
            <p className="text-xs mt-0.5 animate-pulse" style={{ color: "var(--color-accent)" }}>
              {CLEAR_STEP_LABELS[clearStep] ?? clearStep}
            </p>
          ) : (
            <p className="text-xs" style={{ color: "var(--color-text-muted)" }}>문서·벡터·폴더 전체 삭제 (원본 파일 무관)</p>
          )}
        </div>
        <Button
          variant="danger"
          size="sm"
          isLoading={isClearing}
          disabled={isClearing}
          onClick={() => setConfirmOpen(true)}
        >
          초기화
        </Button>
      </div>

      <Modal
        isOpen={confirmOpen}
        onClose={() => setConfirmOpen(false)}
        title="데이터 초기화"
        size="sm"
        footer={
          <div className="flex justify-end gap-2">
            <Button variant="ghost" size="sm" onClick={() => setConfirmOpen(false)}>
              취소
            </Button>
            <Button
              variant="danger"
              size="sm"
              disabled={!agreed}
              onClick={runClearData}
            >
              초기화
            </Button>
          </div>
        }
      >
        <div className="space-y-3">
          <p className="text-sm leading-relaxed" style={{ color: "var(--color-text-primary)" }}>
            모든 인덱싱 데이터와 등록된 폴더가 삭제됩니다.
          </p>
          <p className="text-xs" style={{ color: "var(--color-text-muted)" }}>
            원본 파일은 영향이 없습니다. 이 작업은 되돌릴 수 없습니다.
          </p>
          <label
            className="flex items-start gap-2 p-2.5 rounded-md cursor-pointer select-none"
            style={{
              backgroundColor: "var(--color-bg-primary)",
              border: "1px solid var(--color-border)",
            }}
          >
            <input
              type="checkbox"
              checked={agreed}
              onChange={(e) => setAgreed(e.target.checked)}
              className="mt-0.5"
              data-autofocus
            />
            <span className="text-sm" style={{ color: "var(--color-text-primary)" }}>
              위 내용을 이해했으며 모든 데이터 삭제에 동의합니다.
            </span>
          </label>
        </div>
      </Modal>
    </div>
  );
}
