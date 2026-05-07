import { useState } from "react";
import { invokeWithTimeout, IPC_TIMEOUT } from "../../../utils/invokeWithTimeout";
import { Button } from "../../ui/Button";
import { SettingsToggle } from "../SettingsToggle";
import { useUpdater } from "../../../hooks/useUpdater";
import { UpdateModal } from "../../updater/UpdateModal";
import { getErrorMessage } from "../../../types/error";
import { isMac } from "../../../utils/platform";
import type { TabProps } from "./types";

interface DiagnosticsTabProps extends TabProps {
  setError?: (msg: string | null) => void;
}

export function DiagnosticsTab({ settings, onChange, setError }: DiagnosticsTabProps) {
  // 업데이트 수동 체크 (자동 체크는 App.tsx에서 담당)
  const updater = useUpdater(false);
  const [updateModalOpen, setUpdateModalOpen] = useState(false);
  const handleCheckUpdate = async () => {
    setUpdateModalOpen(true);
    await updater.checkForUpdate();
  };

  return (
    <div className="space-y-3">
      {/* 업데이트 */}
      <div>
        <h3 className="text-sm font-semibold mb-2" style={{ color: "var(--color-text-primary)" }}>업데이트</h3>
        <div className="flex items-center justify-between">
          <div>
            <label className="text-sm font-medium" style={{ color: "var(--color-text-secondary)" }}>
              {isMac ? "자동 업데이트 (macOS 미지원)" : "자동 업데이트 확인"}
            </label>
            <p className="text-xs" style={{ color: "var(--color-text-muted)" }}>
              {isMac
                ? "Apple Developer ID 미보유로 자동 업데이트 비활성. 신버전은 github.com/chrisryugj/docufinder/releases 에서 수동 다운로드"
                : "앱 시작 시 + 6시간마다 자동 체크 · 새 버전 발견 시 알림"}
            </p>
          </div>
          {!isMac && (
            <Button
              variant="ghost"
              size="sm"
              onClick={handleCheckUpdate}
              isLoading={updater.state.phase === "checking"}
              disabled={updater.state.phase === "checking" || updater.state.phase === "downloading" || updater.state.phase === "installing"}
            >
              지금 확인
            </Button>
          )}
        </div>
      </div>

      {/* 오류 리포트 */}
      <div className="border-t pt-3" style={{ borderColor: "var(--color-border)" }}>
        <h3 className="text-sm font-semibold mb-2" style={{ color: "var(--color-text-primary)" }}>오류 리포트</h3>
        <SettingsToggle
          label="오류 자동 전송"
          description="앱에서 오류 발생 시 개발자에게 자동 리포트 · 파일 경로 익명화, 문서 내용/검색어 전송 안 함"
          checked={settings.error_reporting_enabled ?? true}
          onChange={(v) => onChange("error_reporting_enabled", v)}
        />
      </div>

      {/* 로그 */}
      <div className="border-t pt-3" style={{ borderColor: "var(--color-border)" }}>
        <h3 className="text-sm font-semibold mb-2" style={{ color: "var(--color-text-primary)" }}>로그</h3>
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
                setError?.(`로그 폴더 열기 실패: ${getErrorMessage(err)}`);
              }
            }}
          >
            폴더 열기
          </Button>
        </div>
      </div>

      <UpdateModal
        isOpen={updateModalOpen}
        onClose={() => {
          setUpdateModalOpen(false);
          updater.dismiss();
        }}
        state={updater.state}
        onInstall={updater.downloadAndInstall}
        onRestart={updater.restart}
        onCancel={updater.cancel}
      />
    </div>
  );
}
