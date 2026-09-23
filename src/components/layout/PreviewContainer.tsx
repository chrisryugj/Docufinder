import { Suspense, type ReactNode } from "react";
import { PanelLoading } from "../ui/PanelLoading";

interface PreviewContainerProps {
  /** true: 결과 영역 위에 겹쳐 띄움(좁은 창), false: 옆에 밀어 붙임(넓은 창) */
  overlay: boolean;
  width: number;
  minWidth: number;
  /** 겹침 모드 최대 폭 (결과 영역 폭의 85%) */
  overlayMaxWidth: number;
  onResizeStart: (e: React.MouseEvent) => void;
  onClose: () => void;
  /** PreviewPanel 요소. 두 모드가 같은 자리에 두어 전환 때 다시 마운트되지 않는다 */
  children: ReactNode;
}

/**
 * 미리보기 패널 틀 — 밀어 붙이기(push)와 겹쳐 띄우기(overlay)를 감싸는 요소만 바꾼다.
 * 종전엔 두 모드가 PreviewPanel 을 따로 렌더해, 창 크기나 경계 드래그로 모드가 바뀌면
 * 문서를 다시 읽고 스크롤·AI 요약·찾기 막대·레이아웃 보기가 전부 날아갔다.
 */
export function PreviewContainer({
  overlay,
  width,
  minWidth,
  overlayMaxWidth,
  onResizeStart,
  onClose,
  children,
}: PreviewContainerProps) {
  const resizeHandle = (className: string) => (
    <div
      onMouseDown={onResizeStart}
      className={`w-1 cursor-col-resize hover:bg-[var(--color-accent)] transition-colors ${className}`}
      style={{ backgroundColor: "var(--color-border)" }}
      title="드래그하여 너비 조절"
      role="separator"
      aria-orientation="vertical"
      aria-label="미리보기 너비 조절"
    >
      <div className="absolute inset-y-0 -left-1 -right-1" />
    </div>
  );

  return (
    <>
      {overlay ? (
        <div className="absolute inset-0 z-40 bg-black/15 animate-fade-in" onClick={onClose} />
      ) : (
        resizeHandle("shrink-0 group relative")
      )}
      <div
        className={overlay ? "absolute right-0 top-0 bottom-0 z-50 shadow-2xl preview-slide-in" : "shrink-0"}
        style={overlay
          ? { width: Math.max(Math.min(width, overlayMaxWidth), minWidth), minWidth }
          : { width: Math.max(width, minWidth), minWidth, maxWidth: "50%" }}
      >
        {/* overlay 모드에도 리사이즈 핸들 — push 에서 넓히다 overlay 로 전환되면
            핸들이 사라져 다시 줄일 수 없던 버그 수정. 좁히면 push 모드로 자동 복귀. */}
        {overlay && resizeHandle("absolute inset-y-0 left-0 z-10")}
        <Suspense fallback={<PanelLoading label="미리보기를 여는 중" />}>
          {children}
        </Suspense>
      </div>
    </>
  );
}
