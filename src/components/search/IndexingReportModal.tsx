import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Modal } from "../ui/Modal";
import { Button } from "../ui/Button";
import type { AddFolderResult, ConvertHwpResult } from "../../types/index";

interface IndexingReportModalProps {
  isOpen: boolean;
  onClose: () => void;
  results: AddFolderResult[];
  /** 변환 완료 후 재인덱싱 트리거 (변환된 HWPX 경로 전달) */
  onReindex?: (convertedPaths: string[]) => void;
}

export function IndexingReportModal({ isOpen, onClose, results, onReindex }: IndexingReportModalProps) {
  const [showErrors, setShowErrors] = useState(false);
  const [isConverting, setIsConverting] = useState(false);
  const [convertProgress, setConvertProgress] = useState<{ current: number; total: number } | null>(null);
  const [convertResult, setConvertResult] = useState<ConvertHwpResult | null>(null);
  const [convertError, setConvertError] = useState<string | null>(null);

  // results 변경 시 state 초기화 (모달 재오픈 대비)
  useEffect(() => {
    setShowErrors(false);
    setConvertResult(null);
    setConvertError(null);
    setConvertProgress(null);
    setIsConverting(false);
  }, [results]);

  // HWP 변환 진행률 이벤트 리스너
  useEffect(() => {
    if (!isConverting) return;

    const unlisten = listen<{ total: number; current: number; current_file?: string; done?: boolean }>(
      "hwp-convert-progress",
      (event) => {
        setConvertProgress({
          current: event.payload.current,
          total: event.payload.total,
        });
      }
    );

    return () => { unlisten.then((fn) => fn()); };
  }, [isConverting]);

  // 통합 통계
  const totalIndexed = results.reduce((sum, r) => sum + r.indexed_count, 0);
  const totalFailed = results.reduce((sum, r) => sum + r.failed_count, 0);
  const totalOcrImages = results.reduce((sum, r) => sum + (r.ocr_image_count ?? 0), 0);
  const allErrors = results.flatMap((r) => r.errors);
  const allHwpFiles = results.flatMap((r) => r.hwp_files ?? []);

  const handleConvertHwp = async () => {
    if (allHwpFiles.length === 0) return;
    setIsConverting(true);
    setConvertError(null);
    setConvertProgress({ current: 0, total: allHwpFiles.length });

    try {
      const result = await invoke<ConvertHwpResult>("convert_hwp_to_hwpx", {
        paths: allHwpFiles,
      });
      setConvertResult(result);

      // 변환기 미설치 → 설치 안내
      if (result.installer_path) {
        setConvertError(null);
        return;
      }

      // 변환 성공한 파일이 있으면 자동 재인덱싱 트리거
      if (result.success_count > 0 && result.converted_paths.length > 0 && onReindex) {
        onReindex(result.converted_paths);
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setConvertError(msg);
    } finally {
      setIsConverting(false);
      setConvertProgress(null);
    }
  };

  const handleInstallConverter = async () => {
    if (!convertResult?.installer_path) return;
    try {
      await invoke("open_file", { path: convertResult.installer_path, page: null });
    } catch {
      // 실행 실패 시 무시
    }
  };

  return (
    <Modal isOpen={isOpen} onClose={onClose} title="인덱싱 결과" size="lg" closable>
      <div className="space-y-4">
        {/* 요약 */}
        <div className="flex gap-4">
          <StatCard label="성공" value={totalIndexed} color="var(--color-success, #22c55e)" />
          <StatCard label="실패" value={totalFailed} color="var(--color-error, #ef4444)" />
          {totalOcrImages > 0 && (
            <StatCard label="OCR 이미지" value={totalOcrImages} color="#8b5cf6" />
          )}
          {allHwpFiles.length > 0 && (
            <StatCard label="HWP 파일" value={allHwpFiles.length} color="#f59e0b" />
          )}
        </div>

        {/* 에러 목록 */}
        {allErrors.length > 0 && (
          <div>
            <button
              onClick={() => setShowErrors(!showErrors)}
              className="flex items-center gap-1 text-xs font-medium"
              style={{ color: "var(--color-text-muted)" }}
            >
              <svg
                className={`w-3 h-3 transition-transform ${showErrors ? "rotate-90" : ""}`}
                fill="none" viewBox="0 0 24 24" strokeWidth={2} stroke="currentColor"
              >
                <path strokeLinecap="round" strokeLinejoin="round" d="M8.25 4.5l7.5 7.5-7.5 7.5" />
              </svg>
              에러 ({allErrors.length}건)
            </button>
            {showErrors && (
              <div
                className="mt-2 max-h-40 overflow-y-auto rounded p-2 text-xs font-mono"
                style={{ backgroundColor: "var(--color-bg-tertiary)", color: "var(--color-text-secondary)" }}
              >
                {allErrors.slice(0, 50).map((err, i) => (
                  <div key={i} className="py-0.5 truncate">{err}</div>
                ))}
                {allErrors.length > 50 && (
                  <div className="pt-1 text-center" style={{ color: "var(--color-text-muted)" }}>
                    ... 외 {allErrors.length - 50}건
                  </div>
                )}
              </div>
            )}
          </div>
        )}

        {/* HWP 변환 섹션 */}
        {allHwpFiles.length > 0 && !convertResult && (
          <div
            className="rounded-lg p-3 text-sm"
            style={{ backgroundColor: "rgba(245, 158, 11, 0.1)", border: "1px solid rgba(245, 158, 11, 0.2)" }}
          >
            <p style={{ color: "var(--color-text-primary)" }}>
              <strong>{allHwpFiles.length}개</strong>의 구형 HWP 파일이 발견되었습니다.
            </p>
            <p className="mt-1 text-xs" style={{ color: "var(--color-text-secondary)" }}>
              HWPX로 변환하면 내용 검색이 가능해집니다. 원본 파일 옆에 .hwpx 파일이 생성됩니다.
            </p>
            <Button
              variant="primary"
              size="sm"
              className="mt-2"
              onClick={handleConvertHwp}
              disabled={isConverting}
            >
              {isConverting
                ? `변환 중... (${convertProgress?.current ?? 0}/${convertProgress?.total ?? 0})`
                : "HWPX로 변환"}
            </Button>
          </div>
        )}

        {/* 변환 결과 */}
        {convertResult && !convertResult.installer_path && (
          <div
            className="rounded-lg p-3 text-sm"
            style={{
              backgroundColor: convertResult.failed_count > 0
                ? "rgba(245, 158, 11, 0.1)"
                : "rgba(34, 197, 94, 0.1)",
              border: `1px solid ${convertResult.failed_count > 0 ? "rgba(245, 158, 11, 0.2)" : "rgba(34, 197, 94, 0.2)"}`,
            }}
          >
            <p style={{ color: "var(--color-text-primary)" }}>
              변환 완료: <strong>{convertResult.success_count}</strong>개 성공
              {convertResult.failed_count > 0 && `, ${convertResult.failed_count}개 실패`}
            </p>
            {convertResult.success_count > 0 && (
              <p className="mt-1 text-xs" style={{ color: "var(--color-text-secondary)" }}>
                변환된 파일은 자동으로 인덱싱됩니다.
              </p>
            )}
          </div>
        )}

        {/* 변환기 미설치 안내 */}
        {convertResult?.installer_path && (
          <div
            className="rounded-lg p-3 text-sm"
            style={{ backgroundColor: "rgba(245, 158, 11, 0.1)", border: "1px solid rgba(245, 158, 11, 0.2)" }}
          >
            <p style={{ color: "var(--color-text-primary)" }}>
              <strong>HWPX 변환기</strong>가 설치되지 않았습니다.
            </p>
            <p className="mt-1 text-xs" style={{ color: "var(--color-text-secondary)" }}>
              설치 후 다시 변환을 시도해 주세요. 설치가 완료되면 모달을 닫고 다시 폴더를 추가하세요.
            </p>
            <Button
              variant="primary"
              size="sm"
              className="mt-2"
              onClick={handleInstallConverter}
            >
              변환기 설치
            </Button>
          </div>
        )}

        {/* 변환 에러 */}
        {convertError && !convertResult?.installer_path && (
          <div
            className="rounded-lg p-3 text-sm"
            style={{ backgroundColor: "rgba(239, 68, 68, 0.1)", border: "1px solid rgba(239, 68, 68, 0.2)" }}
          >
            <p style={{ color: "var(--color-error, #ef4444)" }}>
              변환 실패: {convertError}
            </p>
            <p className="mt-1 text-xs" style={{ color: "var(--color-text-secondary)" }}>
              한글(HWP) 프로그램이 설치되어 있는지 확인해 주세요.
            </p>
          </div>
        )}

        {/* 닫기 버튼 */}
        <div className="flex justify-end pt-2">
          <Button variant="ghost" onClick={onClose}>닫기</Button>
        </div>
      </div>
    </Modal>
  );
}

function StatCard({ label, value, color }: { label: string; value: number; color: string }) {
  return (
    <div
      className="flex-1 rounded-lg p-3 text-center"
      style={{ backgroundColor: "var(--color-bg-secondary)" }}
    >
      <div className="text-2xl font-bold" style={{ color }}>{value.toLocaleString()}</div>
      <div className="text-xs mt-1" style={{ color: "var(--color-text-muted)" }}>{label}</div>
    </div>
  );
}
