import { useEffect, useLayoutEffect, useMemo, useRef, useState, useCallback, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { X, ArrowRight, Check } from "lucide-react";
import clsx from "clsx";

export interface TourStep {
  /** 대상 요소 CSS selector. null이면 화면 중앙 안내(스포트라이트 없음) */
  selector: string | null;
  title: string;
  body: ReactNode;
  /** 툴팁 배치 선호 방향. 화면 경계 넘으면 자동 flip */
  placement?: "top" | "bottom" | "left" | "right" | "auto";
  /** 스포트라이트 여백(px) */
  padding?: number;
}

interface OnboardingTourProps {
  steps: TourStep[];
  storageKey: string;
  /** 첫 방문 시 자동 시작 여부 */
  autoStart?: boolean;
  /** 외부에서 강제로 시작할 때 사용. 0보다 큰 값으로 증가시킬 것 */
  runKey?: number;
  onComplete?: () => void;
}

interface Rect {
  top: number;
  left: number;
  width: number;
  height: number;
}

const FALLBACK_RECT: Rect = { top: -9999, left: -9999, width: 0, height: 0 };

function rectsEqual(a: Rect, b: Rect) {
  return a.top === b.top && a.left === b.left && a.width === b.width && a.height === b.height;
}

export function OnboardingTour({
  steps,
  storageKey,
  autoStart = true,
  runKey = 0,
  onComplete,
}: OnboardingTourProps) {
  const [mounted, setMounted] = useState(false);
  const [open, setOpen] = useState(false);
  const [index, setIndex] = useState(0);
  const [rect, setRect] = useState<Rect>(FALLBACK_RECT);
  const [hasTarget, setHasTarget] = useState(false);
  const [tipSize, setTipSize] = useState<{ w: number; h: number }>({ w: 380, h: 220 });
  const tipRef = useRef<HTMLDivElement | null>(null);
  const finishedRef = useRef(false);

  useEffect(() => {
    setMounted(true);
  }, []);

  // 첫 방문 자동 시작
  useEffect(() => {
    if (!mounted || !autoStart) return;
    try {
      if (localStorage.getItem(storageKey) === "done") return;
    } catch {
      /* ignore */
    }
    const t = setTimeout(() => {
      finishedRef.current = false;
      setIndex(0);
      setOpen(true);
    }, 1200);
    return () => clearTimeout(t);
  }, [mounted, autoStart, storageKey]);

  // 외부 강제 재시작
  useEffect(() => {
    if (!mounted || runKey <= 0) return;
    finishedRef.current = false;
    setIndex(0);
    setOpen(true);
  }, [runKey, mounted]);

  const current = steps[index];
  const currentSelector = current?.selector ?? null;
  const currentPadding = current?.padding ?? 8;

  const measureTarget = useCallback(() => {
    if (!currentSelector) {
      setHasTarget((prev) => (prev ? false : prev));
      setRect((prev) => (rectsEqual(prev, FALLBACK_RECT) ? prev : FALLBACK_RECT));
      return;
    }
    const el = document.querySelector(currentSelector) as HTMLElement | null;
    if (!el) {
      setHasTarget((prev) => (prev ? false : prev));
      setRect((prev) => (rectsEqual(prev, FALLBACK_RECT) ? prev : FALLBACK_RECT));
      return;
    }
    const r = el.getBoundingClientRect();
    const next: Rect = { top: r.top, left: r.left, width: r.width, height: r.height };
    setHasTarget((prev) => (prev ? prev : true));
    setRect((prev) => (rectsEqual(prev, next) ? prev : next));
  }, [currentSelector]);

  useLayoutEffect(() => {
    if (!open) return;
    if (currentSelector) {
      const el = document.querySelector(currentSelector) as HTMLElement | null;
      if (el) el.scrollIntoView({ block: "center", behavior: "auto" });
    }
    measureTarget();
    const t = setTimeout(measureTarget, 120);
    return () => clearTimeout(t);
  }, [open, index, currentSelector, measureTarget]);

  useEffect(() => {
    if (!open) return;
    let raf = 0;
    const onUpdate = () => {
      if (raf) return;
      raf = requestAnimationFrame(() => {
        raf = 0;
        measureTarget();
      });
    };
    window.addEventListener("resize", onUpdate, { passive: true });
    window.addEventListener("scroll", onUpdate, { passive: true });
    return () => {
      if (raf) cancelAnimationFrame(raf);
      window.removeEventListener("resize", onUpdate);
      window.removeEventListener("scroll", onUpdate);
    };
  }, [open, measureTarget]);

  const finish = useCallback(
    (completed: boolean) => {
      if (finishedRef.current) return;
      finishedRef.current = true;
      setOpen(false);
      setIndex(0);
      try {
        localStorage.setItem(storageKey, "done");
      } catch {
        /* ignore */
      }
      if (completed) onComplete?.();
    },
    [storageKey, onComplete],
  );

  const goNext = useCallback(() => {
    setIndex((i) => {
      if (i < steps.length - 1) return i + 1;
      finish(true);
      return i;
    });
  }, [steps.length, finish]);

  const goPrev = useCallback(() => {
    setIndex((i) => (i > 0 ? i - 1 : i));
  }, []);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") finish(false);
      else if (e.key === "ArrowRight") goNext();
      else if (e.key === "ArrowLeft") goPrev();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, finish, goNext, goPrev]);

  // 툴팁 크기 측정
  useEffect(() => {
    if (!open || !tipRef.current) return;
    const el = tipRef.current;
    const update = () => {
      const r = el.getBoundingClientRect();
      setTipSize((prev) =>
        Math.abs(prev.w - r.width) > 1 || Math.abs(prev.h - r.height) > 1
          ? { w: r.width, h: r.height }
          : prev,
      );
    };
    update();
    const ro = new ResizeObserver(update);
    ro.observe(el);
    return () => ro.disconnect();
  }, [open, index]);

  const spot = useMemo(() => {
    if (!hasTarget) return FALLBACK_RECT;
    return {
      top: Math.max(0, rect.top - currentPadding),
      left: Math.max(0, rect.left - currentPadding),
      width: rect.width + currentPadding * 2,
      height: rect.height + currentPadding * 2,
    };
  }, [hasTarget, rect, currentPadding]);

  const tooltipPos = useMemo(() => {
    if (typeof window === "undefined") return { top: 0, left: 0 };
    const vw = window.innerWidth;
    const vh = window.innerHeight;
    const margin = 16;
    const tipW = tipSize.w;
    const tipH = tipSize.h;

    if (!hasTarget) {
      return {
        top: Math.max(margin, (vh - tipH) / 2),
        left: Math.max(margin, (vw - tipW) / 2),
      };
    }

    const placement = current?.placement ?? "auto";
    const below = spot.top + spot.height + margin + tipH <= vh - margin;
    const above = spot.top - margin - tipH >= margin;
    const rightOK = spot.left + spot.width + margin + tipW <= vw - margin;
    const leftOK = spot.left - margin - tipW >= margin;

    // 가로 배치 우선 (sidebar 같은 경우)
    if (placement === "right" && rightOK) {
      const centered = spot.top + spot.height / 2 - tipH / 2;
      return {
        top: Math.min(Math.max(margin, centered), vh - tipH - margin),
        left: spot.left + spot.width + margin,
      };
    }
    if (placement === "left" && leftOK) {
      const centered = spot.top + spot.height / 2 - tipH / 2;
      return {
        top: Math.min(Math.max(margin, centered), vh - tipH - margin),
        left: Math.max(margin, spot.left - margin - tipW),
      };
    }

    let vertical: "top" | "bottom" = "bottom";
    if (placement === "top" && above) vertical = "top";
    else if (placement === "bottom" && below) vertical = "bottom";
    else if (placement === "auto") vertical = below ? "bottom" : above ? "top" : "bottom";
    else vertical = below ? "bottom" : "top";

    const top =
      vertical === "bottom"
        ? spot.top + spot.height + margin
        : Math.max(margin, spot.top - margin - tipH);

    const centered = spot.left + spot.width / 2 - tipW / 2;
    const left = Math.min(Math.max(margin, centered), vw - tipW - margin);

    return { top, left };
  }, [hasTarget, spot, current, tipSize]);

  if (!mounted || !open || !current) return null;

  // 단일 div + box-shadow 스포트라이트 (GPU 가벼움)
  const overlays = hasTarget ? (
    <>
      <div className="fixed inset-0" onClick={(e) => e.stopPropagation()} />
      <div
        className="fixed pointer-events-none rounded-md transition-all duration-200 ease-out"
        style={{
          top: spot.top,
          left: spot.left,
          width: spot.width,
          height: spot.height,
          boxShadow:
            "0 0 0 9999px rgba(15,23,42,0.62), 0 0 0 1.5px var(--color-accent, #3b82f6)",
        }}
      />
    </>
  ) : (
    <div className="fixed inset-0" style={{ backgroundColor: "rgba(15,23,42,0.7)" }} />
  );

  const isLast = index === steps.length - 1;
  const isFirst = index === 0;
  const total = steps.length;

  return createPortal(
    <div
      className="fixed inset-0 z-[100]"
      aria-live="polite"
      role="dialog"
      aria-modal="true"
      aria-label="기능 투어"
    >
      {overlays}

      {/* 툴팁 카드 */}
      <div
        ref={tipRef}
        className={clsx(
          "fixed z-[101] w-[calc(100vw-32px)] sm:w-[380px] max-w-[380px]",
          "rounded-xl overflow-hidden animate-scale-in",
          "shadow-[0_20px_60px_-15px_rgba(15,23,42,0.4),0_8px_20px_-8px_rgba(15,23,42,0.2)]",
        )}
        style={{
          top: tooltipPos.top,
          left: tooltipPos.left,
          backgroundColor: "var(--color-bg-secondary)",
          border: "1px solid var(--color-border)",
        }}
      >
        {/* 상단 악센트 라인 */}
        <div
          className="h-[2px]"
          style={{
            background: "linear-gradient(to right, transparent, var(--color-accent), transparent)",
          }}
        />

        {/* 헤더 */}
        <div className="px-5 pt-4 pb-1 flex items-start justify-between gap-3">
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2 mb-1.5">
              <span
                className="text-[10px] font-bold tracking-[0.15em] uppercase"
                style={{ color: "var(--color-accent)" }}
              >
                Step {index + 1} / {total}
              </span>
            </div>
            <h3
              className="text-[15px] font-semibold leading-tight tracking-tight break-keep"
              style={{ color: "var(--color-text-primary)" }}
            >
              {current.title}
            </h3>
          </div>
          <button
            onClick={() => finish(false)}
            className="p-1.5 -m-1 rounded-md btn-icon-hover shrink-0"
            aria-label="투어 닫기"
          >
            <X className="w-4 h-4" style={{ color: "var(--color-text-muted)" }} />
          </button>
        </div>

        {/* 본문 */}
        <div
          className="px-5 pb-4 pt-2 text-[13px] leading-relaxed break-keep"
          style={{ color: "var(--color-text-secondary)" }}
        >
          {current.body}
        </div>

        {/* 진행 바 */}
        <div className="px-5">
          <div
            className="h-[3px] rounded-full overflow-hidden"
            style={{ backgroundColor: "var(--color-bg-tertiary)" }}
          >
            <div
              className="h-full transition-[width] duration-400 ease-out rounded-full"
              style={{
                width: `${((index + 1) / total) * 100}%`,
                backgroundColor: "var(--color-accent)",
              }}
            />
          </div>
        </div>

        {/* 액션 */}
        <div
          className="px-5 py-3.5 mt-2 flex items-center justify-between gap-2"
          style={{
            borderTop: "1px solid var(--color-border)",
            backgroundColor: "var(--color-bg-tertiary)",
          }}
        >
          <button
            onClick={() => finish(false)}
            className="text-[12px] font-medium px-1 transition-colors"
            style={{ color: "var(--color-text-muted)" }}
          >
            건너뛰기
          </button>

          <div className="flex items-center gap-1.5">
            {!isFirst && (
              <button
                onClick={goPrev}
                className="h-8 px-3.5 text-[12px] font-medium rounded-md btn-icon-hover"
                style={{ color: "var(--color-text-secondary)" }}
              >
                이전
              </button>
            )}
            <button
              onClick={goNext}
              className={clsx(
                "h-8 px-4 text-[12px] font-semibold rounded-md transition-all",
                "shadow-sm hover:shadow-md flex items-center gap-1.5",
              )}
              style={{
                backgroundColor: "var(--color-accent)",
                color: "#fff",
              }}
            >
              {isLast ? (
                <>
                  시작하기
                  <Check className="w-3 h-3" />
                </>
              ) : (
                <>
                  다음
                  <ArrowRight className="w-3 h-3" />
                </>
              )}
            </button>
          </div>
        </div>
      </div>
    </div>,
    document.body,
  );
}

/** 투어 완료 상태 초기화 */
export function resetOnboardingTour(storageKey: string) {
  try {
    localStorage.removeItem(storageKey);
  } catch {
    /* ignore */
  }
}
