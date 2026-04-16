import { useState, useCallback } from "react";
import { Modal } from "../ui/Modal";
import { Button } from "../ui/Button";

const STORAGE_KEY = "docufinder_ai_disclaimer_accepted";
const LEGACY_STORAGE_KEY = "ai_disclaimer_accepted";

interface AiDisclaimerModalProps {
  isOpen: boolean;
  onAccept: () => void;
  onDecline: () => void;
}

/** AI 기능 사용 시 문서 유출 경고 모달 */
export function AiDisclaimerModal({ isOpen, onAccept, onDecline }: AiDisclaimerModalProps) {
  const [dontShowAgain, setDontShowAgain] = useState(false);

  const handleAccept = useCallback(() => {
    if (dontShowAgain) {
      localStorage.setItem(STORAGE_KEY, "true");
    }
    onAccept();
  }, [dontShowAgain, onAccept]);

  return (
    <Modal
      isOpen={isOpen}
      onClose={onDecline}
      title="AI 기능 사용 안내"
      size="sm"
      footer={
        <div className="flex items-center justify-between w-full">
          <label className="flex items-center gap-2 cursor-pointer select-none">
            <input
              type="checkbox"
              checked={dontShowAgain}
              onChange={(e) => setDontShowAgain(e.target.checked)}
              className="w-3.5 h-3.5 rounded accent-[var(--color-accent)]"
            />
            <span className="text-xs" style={{ color: "var(--color-text-tertiary)" }}>
              다시 표시하지 않기
            </span>
          </label>
          <div className="flex items-center gap-2">
            <Button variant="ghost" size="sm" onClick={onDecline}>
              취소
            </Button>
            <Button variant="primary" size="sm" onClick={handleAccept}>
              동의하고 사용
            </Button>
          </div>
        </div>
      }
    >
      <div className="space-y-3 py-1">
        <div className="flex items-center justify-center">
          <div
            className="w-12 h-12 rounded-full flex items-center justify-center"
            style={{ backgroundColor: "rgba(234, 179, 8, 0.1)" }}
          >
            <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#eab308" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" />
              <line x1="12" y1="9" x2="12" y2="13" />
              <line x1="12" y1="17" x2="12.01" y2="17" />
            </svg>
          </div>
        </div>

        <p className="text-sm text-center" style={{ color: "var(--color-text-primary)" }}>
          AI 질문 및 요약 기능을 사용하면<br />
          <strong>문서 내용의 일부가 외부 서버</strong>(Google Gemini API)로 전송됩니다.
        </p>

        <div
          className="rounded-lg px-3 py-2 text-xs space-y-1"
          style={{
            backgroundColor: "var(--color-bg-tertiary)",
            color: "var(--color-text-secondary)",
          }}
        >
          <p>- 검색된 문서의 텍스트 청크가 API로 전달됩니다</p>
          <p>- 기밀/민감 문서를 다루는 경우 주의하세요</p>
          <p>- API 키는 로컬에 저장되며 외부에 공유되지 않습니다</p>
        </div>
      </div>
    </Modal>
  );
}

/** AI disclaimer 동의 여부 확인 (레거시 키 자동 마이그레이션) */
export function isAiDisclaimerAccepted(): boolean {
  if (localStorage.getItem(STORAGE_KEY) === "true") return true;
  // 레거시 키 마이그레이션: 이전 버전에서 동의한 사용자 경험 보존
  if (localStorage.getItem(LEGACY_STORAGE_KEY) === "true") {
    localStorage.setItem(STORAGE_KEY, "true");
    localStorage.removeItem(LEGACY_STORAGE_KEY);
    return true;
  }
  return false;
}

/** AI disclaimer 동의 상태 초기화 */
export function resetAiDisclaimer(): void {
  localStorage.removeItem(STORAGE_KEY);
  localStorage.removeItem(LEGACY_STORAGE_KEY);
}
