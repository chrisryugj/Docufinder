import { useState, useEffect } from "react";
import { Modal } from "../ui/Modal";
import { Button } from "../ui/Button";
import type { AddFolderResult } from "../../types/index";

interface IndexingReportModalProps {
  isOpen: boolean;
  onClose: () => void;
  results: AddFolderResult[];
}

/** 에러 문자열 파싱: "경로\t에러사유" → { fileName, filePath, reason } */
function parseError(err: string): { fileName: string; filePath: string; reason: string } {
  const tabIdx = err.indexOf("\t");
  if (tabIdx === -1) {
    // 레거시 포맷 또는 요약 메시지 ("... 외 N건 에러 생략")
    return { fileName: "", filePath: "", reason: err };
  }
  const rawPath = err.slice(0, tabIdx);
  const reason = err.slice(tabIdx + 1);
  // 경로에서 파일명 추출
  const sep = rawPath.includes("/") ? "/" : "\\";
  const parts = rawPath.split(sep);
  const fileName = parts[parts.length - 1] || rawPath;
  return { fileName, filePath: rawPath, reason };
}

export function IndexingReportModal({ isOpen, onClose, results }: IndexingReportModalProps) {
  const [showErrors, setShowErrors] = useState(false);

  useEffect(() => {
    setShowErrors(false);
  }, [results]);

  const totalIndexed = results.reduce((sum, r) => sum + r.indexed_count, 0);
  const totalFailed = results.reduce((sum, r) => sum + r.failed_count, 0);
  const totalOcrImages = results.reduce((sum, r) => sum + (r.ocr_image_count ?? 0), 0);
  const allErrors = results.flatMap((r) => r.errors);

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
                className="mt-2 max-h-60 overflow-y-auto rounded-lg border"
                style={{
                  backgroundColor: "var(--color-bg-tertiary)",
                  borderColor: "var(--color-border)",
                }}
              >
                {allErrors.slice(0, 50).map((err, i) => {
                  const parsed = parseError(err);
                  const isSummary = !parsed.fileName;

                  if (isSummary) {
                    return (
                      <div
                        key={i}
                        className="px-3 py-2 text-xs text-center"
                        style={{ color: "var(--color-text-muted)" }}
                      >
                        {parsed.reason}
                      </div>
                    );
                  }

                  return (
                    <div
                      key={i}
                      className="px-3 py-2 text-xs border-b last:border-b-0"
                      style={{ borderColor: "var(--color-border)" }}
                    >
                      {/* 파일명 + 아이콘 */}
                      <div className="flex items-center gap-1.5 mb-0.5">
                        <svg
                          className="w-3.5 h-3.5 flex-shrink-0"
                          style={{ color: "var(--color-error, #ef4444)" }}
                          fill="none" viewBox="0 0 24 24" strokeWidth={2} stroke="currentColor"
                        >
                          <path strokeLinecap="round" strokeLinejoin="round" d="M12 9v3.75m9-.75a9 9 0 11-18 0 9 9 0 0118 0zm-9 3.75h.008v.008H12v-.008z" />
                        </svg>
                        <span
                          className="font-medium truncate"
                          style={{ color: "var(--color-text-primary)" }}
                          title={parsed.filePath}
                        >
                          {parsed.fileName}
                        </span>
                      </div>
                      {/* 경로 (연하게) */}
                      <div
                        className="ml-5 truncate"
                        style={{ color: "var(--color-text-muted)", fontSize: "10px" }}
                        title={parsed.filePath}
                      >
                        {parsed.filePath}
                      </div>
                      {/* 에러 사유 */}
                      <div
                        className="ml-5 mt-0.5"
                        style={{ color: "var(--color-error, #ef4444)", fontSize: "11px" }}
                      >
                        {parsed.reason}
                      </div>
                    </div>
                  );
                })}
                {allErrors.length > 50 && (
                  <div
                    className="px-3 py-2 text-xs text-center"
                    style={{ color: "var(--color-text-muted)" }}
                  >
                    ... 외 {allErrors.length - 50}건
                  </div>
                )}
              </div>
            )}
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
