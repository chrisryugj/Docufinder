import { Modal } from "../ui/Modal";
import { Button } from "../ui/Button";

interface AutoIndexPromptProps {
  isOpen: boolean;
  onClose: () => void;
  onAutoIndex: () => Promise<unknown>;
  onSelectFolder: () => Promise<unknown>;
}

/** 앱 시작 시 등록 폴더 0개일 때 표시되는 인덱싱 안내 다이얼로그 */
export function AutoIndexPrompt({ isOpen, onClose, onAutoIndex, onSelectFolder }: AutoIndexPromptProps) {
  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title="문서 검색을 시작하세요"
      size="sm"
    >
      <div className="space-y-4">
        <p className="text-sm" style={{ color: "var(--color-text-secondary)" }}>
          등록된 폴더가 없습니다. 어떻게 시작할까요?
        </p>
        <div className="space-y-2">
          <Button
            className="w-full justify-center"
            onClick={async () => {
              onClose();
              await onAutoIndex();
            }}
          >
            전체 드라이브 인덱싱
          </Button>
          <p className="text-xs text-center" style={{ color: "var(--color-text-muted)" }}>
            모든 드라이브를 스캔합니다 (시스템 폴더 자동 제외)
          </p>
          <Button
            variant="ghost"
            className="w-full justify-center"
            onClick={async () => {
              onClose();
              await onSelectFolder();
            }}
          >
            폴더 직접 선택
          </Button>
          <div className="pt-2 border-t" style={{ borderColor: "var(--color-border)" }}>
            <button
              className="w-full text-center text-xs py-1.5"
              style={{ color: "var(--color-text-muted)" }}
              onClick={onClose}
            >
              나중에 할게요
            </button>
          </div>
        </div>
      </div>
    </Modal>
  );
}
