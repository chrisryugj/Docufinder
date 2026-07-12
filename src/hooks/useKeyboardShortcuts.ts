import { useEffect, useRef, type RefObject } from "react";

interface ShortcutHandlers {
  onFocusSearch?: () => void;
  onCommandPalette?: () => void;
  onEscape?: () => void;
  onArrowUp?: () => void;
  onArrowDown?: () => void;
  onEnter?: () => void;
  /**
   * 입력 필드 포커스 중 Enter (ux-audit-1: 타이핑 → ↓ 선택 → Enter로 열기 동선).
   * 소비 여부(어느 입력인지·패러다임)는 호출부가 판정 — true 반환 시 기본 동작 차단.
   */
  onEnterInInput?: () => boolean;
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

      // Ctrl+K: 명령 팔레트 (입력 중에도 동작)
      if (isCtrlOrCmd && e.key === "k") {
        e.preventDefault();
        h.onCommandPalette?.();
        return;
      }

      // "/": 검색창 포커스 (입력 중이 아닐 때만 — 타이핑 방해 방지)
      if (e.key === "/" && !isInputFocused) {
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

      // Enter: 입력 포커스 중에도 선택된 결과 열기 허용 (ux-audit-1)
      // - IME 조합 확정 Enter는 제외
      // - 검색 입력 여부/패러다임 판정은 호출부(App) 책임 — true 반환 시에만 기본 동작 차단
      if (e.key === "Enter" && isInputFocused) {
        if (!e.isComposing && h.onEnterInInput?.()) {
          e.preventDefault();
        }
        return;
      }

      // 입력 중이면 아래 단축키 무시
      if (isInputFocused) return;

      // Enter: 선택된 파일 열기
      // - 포커스된 요소가 이미 Enter를 처리(preventDefault)했으면 중복 열기 금지
      //   (예: 파일명 매치 행에 포커스 + 내용 결과 선택 상태 → 파일 2개 동시 실행 방지)
      if (e.key === "Enter") {
        if (e.defaultPrevented) return;
        h.onEnter?.();
        return;
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);
}
