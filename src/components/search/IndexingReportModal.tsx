import { useState, useEffect } from "react";
import { Modal } from "../ui/Modal";
import { Button } from "../ui/Button";
import type { AddFolderResult } from "../../types/index";

interface IndexingReportModalProps {
  isOpen: boolean;
  onClose: () => void;
  results: AddFolderResult[];
}

export function IndexingReportModal({ isOpen, onClose, results }: IndexingReportModalProps) {
  const [showErrors, setShowErrors] = useState(false);

  // results 변경 시 state 초기화 (모달 재오픈 대비)
  useEffect(() => {
    setShowErrors(false);
  }, [results]);

  // 통합 통계
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
