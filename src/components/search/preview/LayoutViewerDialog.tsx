import { useEffect, useRef } from "react";
import { LayoutView } from "../LayoutView";
import { PdfLayoutView } from "../PdfLayoutView";

interface LayoutViewerDialogProps {
  filePath: string;
  isPdf: boolean;
  layoutSvg: string | null;
  findTerm?: string;
  onClose: () => void;
}

/**
 * 문서 크게 보기: 레이아웃 렌더를 팝업으로 크게. 휠=스크롤, Ctrl/⌘+휠(트랙패드
 * 핀치)=줌, 너비맞춤은 창 크기 따라 스케일. 검증된 LayoutView 를 onClose 로 재사용.
 * role=dialog 라 앱 bare-key 가드에 자동 편입(뒤 뷰 몰래 전환 방지).
 */
export function LayoutViewerDialog({ filePath, isPdf, layoutSvg, findTerm, onClose }: LayoutViewerDialogProps) {
  const viewerRef = useRef<HTMLDivElement>(null); // 팝업 다이얼로그 — 포커스 트랩 대상
  const returnFocusRef = useRef<HTMLElement | null>(null); // 닫힐 때 포커스 복원 대상

  // 접근성: 초기 포커스 + Tab 트랩 + 포커스 복원.
  // (Escape 닫기는 LayoutView 가 capture 단계에서 이미 처리하므로 여기선 Tab 만.)
  // ui/Modal 의 포커스 트랩과 동일 규약: disabled/숨김 요소 제외, 리스너는 document 에
  // 걸어 포커스가 모달 밖으로 새더라도 되돌린다. layoutSvg 재렌더로 내부 노드가 교체되면 재설치.
  useEffect(() => {
    const dialog = viewerRef.current;
    if (!dialog) return;
    // 실제로 포커스를 받는 요소만 — disabled(예: 1페이지에서 이전/다음 버튼)·숨김 제외.
    const FOCUSABLE =
      'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';
    const getFocusable = () =>
      Array.from(dialog.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
        (el) => el.getClientRects().length > 0,
      );
    // 직전 포커스 저장 후 다이얼로그 첫 요소로 이동(렌더 커밋 후).
    returnFocusRef.current = document.activeElement as HTMLElement | null;
    const raf = requestAnimationFrame(() => {
      const [first] = getFocusable();
      (first ?? dialog).focus();
    });
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== "Tab") return;
      const items = getFocusable();
      const firstEl = items[0];
      const lastEl = items[items.length - 1];
      if (!firstEl) {
        // 포커스 가능한 컨트롤이 없으면 컨테이너에 가둔다.
        e.preventDefault();
        dialog.focus();
        return;
      }
      const active = document.activeElement;
      if (!dialog.contains(active)) {
        // 포커스가 다이얼로그 밖으로 샜으면(다른 단축키 등) 안으로 되돌린다.
        e.preventDefault();
        (e.shiftKey ? lastEl : firstEl).focus();
      } else if (e.shiftKey) {
        if (active === firstEl) {
          e.preventDefault();
          lastEl.focus();
        }
      } else if (active === lastEl) {
        e.preventDefault();
        firstEl.focus();
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => {
      cancelAnimationFrame(raf);
      document.removeEventListener("keydown", onKeyDown);
      // 닫힐 때 원래 위치로 포커스 복원(요소가 아직 문서에 있으면).
      const prev = returnFocusRef.current;
      if (prev?.isConnected) prev.focus();
    };
  }, [layoutSvg]);

  return (
    <div
      ref={viewerRef}
      role="dialog"
      aria-modal="true"
      aria-label="문서 크게 보기"
      tabIndex={-1}
      className="fixed inset-0 z-[1200] flex p-4 sm:p-8 outline-none"
      style={{ backgroundColor: "rgba(0,0,0,0.6)" }}
      onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}
    >
      <div
        className="flex-1 min-h-0 rounded-xl overflow-hidden border shadow-2xl"
        style={{ borderColor: "var(--color-border)", backgroundColor: "var(--color-bg-primary)" }}
      >
        {isPdf ? (
          <PdfLayoutView filePath={filePath} onClose={onClose} />
        ) : (
          <LayoutView svg={layoutSvg!} onClose={onClose} findTerm={findTerm} />
        )}
      </div>
    </div>
  );
}
