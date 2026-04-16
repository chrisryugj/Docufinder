import { useEffect, useRef, type RefObject } from "react";

interface ShortcutHandlers {
  onFocusSearch?: () => void;
  onEscape?: () => void;
  onArrowUp?: () => void;
  onArrowDown?: () => void;
  onEnter?: () => void;
  onCopy?: () => void; // Ctrl+Shift+C로 전용 바인딩 (일반 Ctrl+C 복사와 충돌 방지)
  onToggleSidebar?: () => void;
}

/**
 * 키보드 단축키 관리 훅
 * ref 패턴: 이벤트 리스너를 1회만 등록하고, 최신 핸들러는 ref로 참조
 */
export function useKeyboardShortcuts(
  handlers: ShortcutHandlers,
  _searchInputRef?: RefObject<HTMLInputElement | null>
) {
  const handlersRef = useRef(handlers);
  handlersRef.current = handlers;

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const h = handlersRef.current;
      const isCtrlOrCmd = e.ctrlKey || e.metaKey;
      const isInputFocused =
        document.activeElement?.tagName === "INPUT" ||
        document.activeElement?.tagName === "TEXTAREA";

      // Ctrl+K: 검색창 포커스 (타겟 선택은 핸들러에서 처리)
      if (isCtrlOrCmd && e.key === "k") {
        e.preventDefault();
        h.onFocusSearch?.();
        return;
      }

      // Escape: 검색 초기화 (모달이 열려있으면 모달이 자체 처리하므로 스킵)
      if (e.key === "Escape") {
        const modalOpen = document.querySelector("[role='dialog']");
        if (!modalOpen) {
          h.onEscape?.();
        }
        return;
      }

      // Ctrl+B: 사이드바 토글
      if (isCtrlOrCmd && e.key === "b") {
        e.preventDefault();
        h.onToggleSidebar?.();
        return;
      }

      // 화살표 위/아래: 결과 탐색
      // - 모달 열려있으면 모달 내 입력에 양보
      // - 검색창 외 입력 필드(설정, QA 질문 등)에서는 양보
      if (e.key === "ArrowUp" || e.key === "ArrowDown") {
        const modalOpen = document.querySelector("[role='dialog']");
        if (modalOpen) return;
        if (isInputFocused && document.activeElement?.tagName === "TEXTAREA") return;

        e.preventDefault();
        if (e.key === "ArrowUp") h.onArrowUp?.();
        else h.onArrowDown?.();
        return;
      }

      // Ctrl+Shift+C: 선택된 결과 경로 복사 (일반 복사와 충돌 방지)
      // 입력 필드 포커스 상태라도 명시적 조합이므로 허용.
      if (isCtrlOrCmd && e.shiftKey && (e.key === "C" || e.key === "c")) {
        e.preventDefault();
        h.onCopy?.();
        return;
      }

      // 입력 중이면 아래 단축키 무시
      if (isInputFocused) return;

      // Enter: 선택된 파일 열기
      if (e.key === "Enter") {
        h.onEnter?.();
        return;
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);
}
