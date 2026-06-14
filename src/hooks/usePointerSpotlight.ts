import { useEffect, useRef } from "react";

/**
 * 마우스 위치를 CSS 변수(--spot-x / --spot-y)로 요소에 직접 주입한다.
 * - React state 미사용 → 리렌더 0회 (mousemove를 setState로 받으면 리렌더 폭발)
 * - requestAnimationFrame throttle → 프레임당 최대 1회만 DOM 기록
 * - pointermove는 커서가 해당 요소 위에 있을 때만 발화 → 항상 활성 카드 1개뿐
 * - prefers-reduced-motion 시 비활성, 언마운트 시 cleanup
 *
 * `.result-card::after` 등 radial-gradient spotlight 와 함께 사용.
 */
export function usePointerSpotlight<T extends HTMLElement>() {
  const ref = useRef<T>(null);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;

    let raf = 0;
    let x = 0;
    let y = 0;

    const flush = () => {
      raf = 0;
      el.style.setProperty("--spot-x", `${x}px`);
      el.style.setProperty("--spot-y", `${y}px`);
    };

    const onMove = (e: PointerEvent) => {
      const rect = el.getBoundingClientRect();
      x = e.clientX - rect.left;
      y = e.clientY - rect.top;
      if (!raf) raf = requestAnimationFrame(flush);
    };

    el.addEventListener("pointermove", onMove, { passive: true });
    return () => {
      el.removeEventListener("pointermove", onMove);
      if (raf) cancelAnimationFrame(raf);
    };
  }, []);

  return ref;
}
