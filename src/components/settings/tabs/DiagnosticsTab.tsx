import { useEffect, useState, useCallback } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invokeWithTimeout, IPC_TIMEOUT } from "../../../utils/invokeWithTimeout";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "../../ui/Button";
import { SettingsToggle } from "../SettingsToggle";
import { useUpdater } from "../../../hooks/useUpdater";
import { UpdateModal } from "../../updater/UpdateModal";
import type { TabProps } from "./types";

interface DiagnosticsTabProps extends TabProps {
  setError?: (msg: string | null) => void;
}

interface FormulaModelInfo {
  name: string;
  filename: string;
  sizeMb: number;
  exists: boolean;
  verified: boolean;
  path: string;
  invalidReason?: string | null;
}

interface FormulaModelsStatus {
  modelsDir: string;
  allReady: boolean;
  models: FormulaModelInfo[];
}

export function DiagnosticsTab({ settings, onChange, setError }: DiagnosticsTabProps) {
  // 업데이트 수동 체크 (자동 체크는 App.tsx에서 담당)
  const updater = useUpdater(false);
  const [updateModalOpen, setUpdateModalOpen] = useState(false);
  const handleCheckUpdate = async () => {
    setUpdateModalOpen(true);
    await updater.checkForUpdate();
  };

  // ────────── 수식 OCR 모델 상태 ──────────
  const [formulaStatus, setFormulaStatus] = useState<FormulaModelsStatus | null>(null);
  const [formulaChecking, setFormulaChecking] = useState(false);
  const [formulaDownloading, setFormulaDownloading] = useState(false);
  const [formulaProgress, setFormulaProgress] = useState<string | null>(null);
  const [formulaError, setFormulaError] = useState<string | null>(null);

  const checkFormulaStatus = useCallback(async () => {
    setFormulaChecking(true);
    setFormulaError(null);
    try {
      const s = await invoke<FormulaModelsStatus>("get_formula_models_status");
      setFormulaStatus(s);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setFormulaError(msg);
    } finally {
      setFormulaChecking(false);
    }
  }, []);

  // 토글 on 된 상태에서 처음 열릴 때 상태 조회
  useEffect(() => {
    if (settings.formula_ocr_enabled && !formulaStatus && !formulaChecking) {
      void checkFormulaStatus();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [settings.formula_ocr_enabled]);

  // 다운로드 진행률 이벤트 구독
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    listen<string>("formula-model-progress", (ev) => {
      setFormulaProgress(ev.payload);
    })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => {});
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  const handleDownloadFormula = useCallback(async () => {
    setFormulaDownloading(true);
    setFormulaError(null);
    setFormulaProgress("다운로드 시작…");
    try {
      await invoke("download_formula_models");
      await checkFormulaStatus();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setFormulaError(msg);
    } finally {
      setFormulaDownloading(false);
    }
  }, [checkFormulaStatus]);

  return (
    <div className="space-y-3">
      {/* 업데이트 */}
      <div>
        <h3 className="text-sm font-semibold mb-2" style={{ color: "var(--color-text-primary)" }}>업데이트</h3>
        <div className="flex items-center justify-between">
          <div>
            <label className="text-sm font-medium" style={{ color: "var(--color-text-secondary)" }}>자동 업데이트 확인</label>
            <p className="text-xs" style={{ color: "var(--color-text-muted)" }}>
              앱 시작 시 + 6시간마다 자동 체크 · 새 버전 발견 시 알림
            </p>
          </div>
          <Button
            variant="ghost"
            size="sm"
            onClick={handleCheckUpdate}
            isLoading={updater.state.phase === "checking"}
            disabled={updater.state.phase === "checking" || updater.state.phase === "downloading" || updater.state.phase === "installing"}
          >
            지금 확인
          </Button>
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

      {/* PDF 수식 OCR */}
      <div className="border-t pt-3" style={{ borderColor: "var(--color-border)" }}>
        <h3 className="text-sm font-semibold mb-2" style={{ color: "var(--color-text-primary)" }}>PDF 수식 검색</h3>
        <SettingsToggle
          label="PDF 수식 OCR"
          description="PDF 페이지에서 수식(Pix2Text ONNX)을 LaTeX 로 자동 추출 → 검색/미리보기에 KaTeX 렌더 · 모델 ~155MB (최초 1회 자동 다운로드) · 켜면 PDF 인덱싱 속도 현저히 느려짐"
          checked={settings.formula_ocr_enabled ?? false}
          onChange={(v) => {
            onChange("formula_ocr_enabled", v);
            if (v) {
              // 토글 on 직후 모델 상태 자동 확인
              void checkFormulaStatus();
            }
          }}
        />
        {settings.formula_ocr_enabled && (
          <div
            className="mt-2 p-3 rounded text-xs"
            style={{
              backgroundColor: "var(--color-bg-secondary)",
              color: "var(--color-text-secondary)",
            }}
          >
            <div className="flex items-center justify-between mb-2">
              <span className="font-medium" style={{ color: "var(--color-text-primary)" }}>
                모델 상태
              </span>
              <div className="flex gap-2">
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={checkFormulaStatus}
                  isLoading={formulaChecking}
                  disabled={formulaChecking || formulaDownloading}
                >
                  확인
                </Button>
                {formulaStatus && !formulaStatus.allReady && (
                  <Button
                    variant="primary"
                    size="sm"
                    onClick={handleDownloadFormula}
                    isLoading={formulaDownloading}
                    disabled={formulaDownloading}
                  >
                    다운로드
                  </Button>
                )}
              </div>
            </div>
            {formulaError && (
              <p className="mb-2" style={{ color: "var(--color-error)" }}>
                오류: {formulaError}
              </p>
            )}
            {formulaProgress && formulaDownloading && (
              <p className="mb-2" style={{ color: "var(--color-text-muted)" }}>
                {formulaProgress}
              </p>
            )}
            {formulaStatus ? (
              <ul className="space-y-1">
                {formulaStatus.models.map((m) => (
                  <li key={m.filename} className="flex items-center justify-between">
                    <span>
                      {m.name} ({m.sizeMb}MB)
                    </span>
                    <span
                      style={{
                        color: m.verified
                          ? "var(--color-success)"
                          : m.exists
                            ? "var(--color-warning)"
                            : "var(--color-text-muted)",
                      }}
                    >
                      {m.verified ? "✓ 준비됨" : m.exists ? "⚠ SHA 불일치" : "— 없음"}
                    </span>
                  </li>
                ))}
                <li className="pt-1 mt-1 border-t" style={{ borderColor: "var(--color-border)", color: "var(--color-text-muted)" }}>
                  저장 위치: {formulaStatus.modelsDir}
                </li>
              </ul>
            ) : (
              <p style={{ color: "var(--color-text-muted)" }}>
                상태 확인 필요 — "확인" 버튼을 눌러주세요.
              </p>
            )}
          </div>
        )}
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
                const message = err instanceof Error ? err.message : String(err);
                setError?.(`로그 폴더 열기 실패: ${message}`);
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
