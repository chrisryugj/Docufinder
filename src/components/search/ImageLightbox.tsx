import { memo, useCallback, useEffect, useRef, useState } from "react";
import { X, ZoomIn, ZoomOut, Maximize2 } from "lucide-react";

interface ImageLightboxProps {
  /** 이미지 소스 — 레이아웃 SVG <image> 의 data URI 또는 URL */
  src: string;
  alt?: string;
  onClose: () => void;
}

const MIN_SCALE = 0.2;
const MAX_SCALE = 8;

/**
 * 이미지 확대 라이트박스 — 미리보기(레이아웃 SVG)의 이미지를 클릭하면 전체화면
 * 오버레이로 띄워 마우스 드래그 팬 · 휠 줌(커서 기준) · 더블클릭 리셋으로 크게 본다.
 * 오픈 시 스테이지에 맞춰(fit) 표시하고 scale 로 확대한다. Esc·X·배경 클릭으로 닫음.
 */
export const ImageLightbox = memo(function ImageLightbox({ src, alt, onClose }: ImageLightboxProps) {
  const [scale, setScale] = useState(1);
  const [tx, setTx] = useState(0);
  const [ty, setTy] = useState(0);
  const stageRef = useRef<HTMLDivElement>(null);
  const dragRef = useRef<{ x: number; y: number; tx: number; ty: number } | null>(null);
  const pannedRef = useRef(false); // 드래그로 움직였는지 — 배경 클릭 닫힘과 구분

  const reset = useCallback(() => { setScale(1); setTx(0); setTy(0); }, []);

  // 키보드 — Esc 닫기, +/- 줌, 0 리셋. capture 로 전역 단축키(뷰 전환 등)보다 먼저 소비.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") { e.stopPropagation(); e.preventDefault(); onClose(); }
      else if (e.key === "+" || e.key === "=") { e.preventDefault(); setScale((s) => Math.min(MAX_SCALE, s * 1.2)); }
      else if (e.key === "-" || e.key === "_") { e.preventDefault(); setScale((s) => Math.max(MIN_SCALE, s / 1.2)); }
      else if (e.key === "0") { e.preventDefault(); reset(); }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [onClose, reset]);

  // 휠 줌 — 커서 아래 지점이 고정되도록 translate 보정
  const onWheel = useCallback((e: React.WheelEvent) => {
    e.preventDefault();
    const rect = stageRef.current?.getBoundingClientRect();
    if (!rect) return;
    const cx = e.clientX - rect.left - rect.width / 2;
    const cy = e.clientY - rect.top - rect.height / 2;
    setScale((s) => {
      const ns = Math.min(MAX_SCALE, Math.max(MIN_SCALE, s * (e.deltaY < 0 ? 1.12 : 1 / 1.12)));
      const ratio = ns / s;
      setTx((x) => cx - (cx - x) * ratio);
      setTy((y) => cy - (cy - y) * ratio);
      return ns;
    });
  }, []);

  const onMouseDown = useCallback((e: React.MouseEvent) => {
    pannedRef.current = false;
    dragRef.current = { x: e.clientX, y: e.clientY, tx, ty };
  }, [tx, ty]);
  const onMouseMove = useCallback((e: React.MouseEvent) => {
    const d = dragRef.current;
    if (!d) return;
    if (Math.abs(e.clientX - d.x) + Math.abs(e.clientY - d.y) > 3) pannedRef.current = true;
    setTx(d.tx + (e.clientX - d.x));
    setTy(d.ty + (e.clientY - d.y));
  }, []);
  const endDrag = useCallback(() => { dragRef.current = null; }, []);

  // 스테이지 배경(이미지 밖) 클릭 — 팬이 아니었으면 닫기
  const onStageClick = useCallback((e: React.MouseEvent) => {
    if (e.target === stageRef.current && !pannedRef.current) onClose();
  }, [onClose]);

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label={alt || "이미지 확대"}
      className="fixed inset-0 flex flex-col"
      style={{ backgroundColor: "rgba(0,0,0,0.82)", zIndex: 1200 }}
    >
      {/* 툴바 */}
      <div className="flex items-center gap-1 px-3 py-2 shrink-0" style={{ color: "rgba(255,255,255,0.92)" }}>
        <button onClick={() => setScale((s) => Math.max(MIN_SCALE, s / 1.2))} className="p-1.5 rounded-lg hover:bg-white/10 transition-colors" title="축소 (−)" aria-label="축소"><ZoomOut size={16} /></button>
        <span className="tabular-nums text-xs w-12 text-center select-none">{Math.round(scale * 100)}%</span>
        <button onClick={() => setScale((s) => Math.min(MAX_SCALE, s * 1.2))} className="p-1.5 rounded-lg hover:bg-white/10 transition-colors" title="확대 (+)" aria-label="확대"><ZoomIn size={16} /></button>
        <button onClick={reset} className="p-1.5 rounded-lg hover:bg-white/10 transition-colors" title="맞춤 (0)" aria-label="맞춤"><Maximize2 size={16} /></button>
        <button onClick={onClose} className="ml-auto p-1.5 rounded-lg hover:bg-white/10 transition-colors" title="닫기 (Esc)" aria-label="닫기"><X size={18} /></button>
      </div>

      {/* 이미지 무대 — 드래그 팬 · 휠 줌 · 더블클릭 리셋 */}
      <div
        ref={stageRef}
        className="flex-1 min-h-0 overflow-hidden flex items-center justify-center"
        style={{ cursor: "grab" }}
        onWheel={onWheel}
        onMouseDown={onMouseDown}
        onMouseMove={onMouseMove}
        onMouseUp={endDrag}
        onMouseLeave={endDrag}
        onClick={onStageClick}
        onDoubleClick={() => (scale === 1 ? setScale(2) : reset())}
      >
        <img
          src={src}
          alt={alt || ""}
          draggable={false}
          className="select-none"
          style={{
            maxWidth: "100%",
            maxHeight: "100%",
            transform: `translate(${tx}px, ${ty}px) scale(${scale})`,
            transformOrigin: "center center",
            willChange: "transform",
          }}
        />
      </div>
    </div>
  );
});
