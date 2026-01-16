import { useEffect, useCallback, RefObject } from "react";

interface ShortcutHandlers {
  onFocusSearch?: () => void;
  onEscape?: () => void;
  onArrowUp?: () => void;
  onArrowDown?: () => void;
  onEnter?: () => void;
  onCopy?: () => void;
  onToggleSidebar?: () => void;
}

/**
 * 키보드 단축키 관리 훅
 */
export function useKeyboardShortcuts(
  handlers: ShortcutHandlers,
  searchInputRef?: RefObject<HTMLInputElement | null>
) {
  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      const isCtrlOrCmd = e.ctrlKey || e.metaKey;
      const isInputFocused =
        document.activeElement?.tagName === "INPUT" ||
        document.activeElement?.tagName === "TEXTAREA";

      // Ctrl+K: 검색창 포커스
      if (isCtrlOrCmd && e.key === "k") {
        e.preventDefault();
        if (searchInputRef?.current) {
          searchInputRef.current.focus();
          searchInputRef.current.select();
        }
        handlers.onFocusSearch?.();
        return;
      }

      // Escape: 검색 초기화 / 모달 닫기
      if (e.key === "Escape") {
        handlers.onEscape?.();
        return;
      }

      // Ctrl+B: 사이드바 토글
      if (isCtrlOrCmd && e.key === "b") {
        e.preventDefault();
        handlers.onToggleSidebar?.();
        return;
      }

      // 입력 중이면 아래 단축키 무시
      if (isInputFocused) return;

      // 화살표 위/아래: 결과 탐색
      if (e.key === "ArrowUp") {
        e.preventDefault();
        handlers.onArrowUp?.();
        return;
      }

      if (e.key === "ArrowDown") {
        e.preventDefault();
        handlers.onArrowDown?.();
        return;
      }

      // Enter: 선택된 파일 열기
      if (e.key === "Enter") {
        handlers.onEnter?.();
        return;
      }

      // Ctrl+C: 경로 복사 (입력 필드 외부에서만)
      if (isCtrlOrCmd && e.key === "c" && !window.getSelection()?.toString()) {
        handlers.onCopy?.();
        return;
      }
    },
    [handlers, searchInputRef]
  );

  useEffect(() => {
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [handleKeyDown]);
}
