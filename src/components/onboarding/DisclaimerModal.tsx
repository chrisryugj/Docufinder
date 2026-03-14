import { useState } from "react";
import { Modal } from "../ui/Modal";
import { Button } from "../ui/Button";

interface DisclaimerModalProps {
  isOpen: boolean;
  onAccept: () => void;
  onExit: () => void;
}

export function DisclaimerModal({ isOpen, onAccept, onExit }: DisclaimerModalProps) {
  const [isChecked, setIsChecked] = useState(false);

  return (
    <Modal
      isOpen={isOpen}
      onClose={() => {}}
      title="Anything 사용 동의"
      size="lg"
      closable={false}
    >
      <div className="space-y-4">
        {/* 앱 소개 */}
        <div className="flex items-center gap-3 mb-4">
          <img src="/anything.png" alt="Anything" className="w-12 h-12 object-contain dark:hidden" />
          <img src="/anything-l.png" alt="Anything" className="w-12 h-12 object-contain hidden dark:block" />
          <div>
            <h3 className="text-base font-bold" style={{ color: "var(--color-text-primary)" }}>
              Anything
            </h3>
            <p className="text-sm" style={{ color: "var(--color-text-muted)" }}>
              로컬 문서 검색 애플리케이션
            </p>
          </div>
        </div>

        {/* 약관 내용 */}
        <div
          className="rounded-lg p-4 max-h-64 overflow-y-auto text-sm space-y-4 leading-relaxed"
          style={{
            backgroundColor: "var(--color-bg-primary)",
            border: "1px solid var(--color-border)",
            color: "var(--color-text-secondary)",
          }}
        >
          <section>
            <h4 className="text-sm font-semibold mb-2" style={{ color: "var(--color-text-primary)" }}>
              1. 서비스 개요
            </h4>
            <p>
              본 소프트웨어(Anything)는 로컬 문서 검색을 위한 데스크톱 애플리케이션입니다.
              HWPX, DOCX, XLSX, PDF, TXT 형식의 문서를 인덱싱하고
              키워드 및 의미 기반 검색을 제공합니다.
            </p>
          </section>

          <section>
            <h4 className="text-sm font-semibold mb-2" style={{ color: "var(--color-text-primary)" }}>
              2. 데이터 처리
            </h4>
            <ul className="list-disc list-inside space-y-1.5">
              <li>모든 문서 인덱싱 및 검색은 사용자의 로컬 컴퓨터에서만 수행됩니다.</li>
              <li>어떠한 문서 내용이나 개인 정보도 외부 서버로 전송되지 않습니다.</li>
              <li>임베딩 모델(KoSimCSE-roberta)은 로컬에서 실행됩니다.</li>
              <li>인덱스 데이터는 앱 데이터 폴더에 저장되며, 앱 삭제 시 함께 제거됩니다.</li>
            </ul>
          </section>

          <section>
            <h4 className="text-sm font-semibold mb-2" style={{ color: "var(--color-text-primary)" }}>
              3. 면책 사항
            </h4>
            <ul className="list-disc list-inside space-y-1.5">
              <li>검색 결과의 정확성이나 완전성을 보장하지 않습니다.</li>
              <li>본 소프트웨어 사용으로 인한 직접적/간접적 손해에 대해 책임지지 않습니다.</li>
              <li>중요한 문서는 원본 파일을 직접 확인하시기 바랍니다.</li>
              <li>파일 형식에 따라 일부 내용이 추출되지 않을 수 있습니다.</li>
            </ul>
          </section>

          <section>
            <h4 className="text-sm font-semibold mb-2" style={{ color: "var(--color-text-primary)" }}>
              4. 저작권
            </h4>
            <p>
              본 소프트웨어의 저작권은 개발팀(AI.Do)에 있으며,
              서울특별시 광진구청 업무 목적으로 자유롭게 사용할 수 있습니다.
            </p>
          </section>
        </div>

        {/* 동의 체크박스 */}
        <label
          className="flex items-center gap-3 cursor-pointer p-3 rounded-lg transition-colors"
          style={{
            backgroundColor: isChecked ? "var(--color-accent-light)" : "transparent",
            border: `1px solid ${isChecked ? "var(--color-accent)" : "var(--color-border)"}`,
          }}
        >
          <input
            type="checkbox"
            checked={isChecked}
            onChange={(e) => setIsChecked(e.target.checked)}
            className="w-5 h-5 rounded accent-current"
            style={{ accentColor: "var(--color-accent)" }}
          />
          <span className="text-sm" style={{ color: "var(--color-text-primary)" }}>
            위 내용을 모두 읽고 이해했으며, 이에 동의합니다.
          </span>
        </label>

        {/* 버튼 */}
        <div className="flex gap-3 pt-2">
          <Button
            variant="ghost"
            onClick={onExit}
            className="flex-1"
          >
            종료
          </Button>
          <Button
            variant="primary"
            onClick={onAccept}
            disabled={!isChecked}
            className="flex-1"
          >
            동의하고 시작
          </Button>
        </div>
      </div>
    </Modal>
  );
}
