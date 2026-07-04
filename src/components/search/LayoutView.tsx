import { memo, useRef, useState, useEffect, useCallback } from "react";
import { ChevronLeft, ChevronRight, ZoomIn, ZoomOut, Maximize2, ArrowUp, ArrowDown, Expand, X } from "lucide-react";

interface LayoutViewProps {
  /** kordoc render 가 만든 전체 페이지 세로 스택 SVG (신뢰된 소스) */
  svg: string;
  /** 찾기 바 확정어 — 레이아웃 뷰 내 SVG <text> 매치 이동 (없으면 비활성) */
  findTerm?: string;
  /** 있으면 뷰어(팝업) 모드 — 툴바에 닫기(X), Esc 로 닫힘 */
  onClose?: () => void;
  /** 있으면 인라인 모드 — 툴바에 "크게 보기(팝업)" 버튼 노출 */
  onExpand?: () => void;
}

const ZOOM_MIN = 0.3;
const ZOOM_MAX = 4;
const ZOOM_STEP = 0.2;

/**
 * 레이아웃(조판 SVG) 뷰어 — 인라인 SVG 로 삽입해 줌/팬·페이지 네비·찾기 매치 이동을 연다.
 * `<img>` 격리 대신 인라인이지만, SVG 는 kordoc 이 생성한 신뢰 소스이고 스크립트/외부
 * 리소스를 포함하지 않는다(텍스트·도형·이미지 data URI 뿐). DOM 접근이 열려야 페이지
 * 점프·매치 스크롤·텍스트 선택이 가능하다.
 *
 * 팝업 뷰어 모드(onClose 지정): PreviewPanel 이 이 컴포넌트를 전체화면 오버레이에 다시
 * 렌더해 문서를 크게 보게 한다. 휠=스크롤, Ctrl/⌘+휠(트랙패드 핀치)=줌 — 문서 뷰어 관례.
 */
export const LayoutView = memo(function LayoutView({
  svg,
  findTerm,
  onClose,
  onExpand,
}: LayoutViewProps) {
  const scrollRef = useRef<HTMLDivElement>(null); // overflow 스크롤 컨테이너(팬)
  const hostRef = useRef<HTMLDivElement>(null); // dangerouslySetInnerHTML 대상
  const matchesRef = useRef<SVGTextElement[]>([]);

  const [zoom, setZoom] = useState(1); // 1 = fit-width
  const [fitWidth, setFitWidth] = useState(true);
  const [page, setPage] = useState(1);
  const [pageCount, setPageCount] = useState(1);
  const [matchIdx, setMatchIdx] = useState(0);
  const [matchCount, setMatchCount] = useState(0);

  // 페이지 수 (data-page 그룹)
  useEffect(() => {
    setPageCount(Math.max(1, (svg.match(/data-page="/g) ?? []).length));
    setPage(1);
  }, [svg]);

  // 페이지 점프
  const goPage = useCallback((p: number) => {
    const host = hostRef.current;
    if (!host) return;
    const clamped = Math.max(1, Math.min(pageCount, p));
    const g = host.querySelector(`[data-page="${clamped}"]`);
    g?.scrollIntoView({ behavior: "smooth", block: "start" });
    setPage(clamped);
  }, [pageCount]);

  // 스크롤 → 현재 페이지 감지 (뷰포트 중앙에 걸친 페이지)
  const onScroll = useCallback(() => {
    const host = hostRef.current, sc = scrollRef.current;
    if (!host || !sc) return;
    const mid = sc.scrollTop + sc.clientHeight / 2;
    let cur = 1;
    host.querySelectorAll<HTMLElement>("[data-page]").forEach((el) => {
      if (el.offsetTop <= mid) cur = Number(el.getAttribute("data-page")) || cur;
    });
    setPage(cur);
  }, []);

  // 줌 — Ctrl/⌘+휠(트랙패드 핀치도 이 이벤트로 도착). 일반 휠은 스크롤(문서 뷰어 관례).
  const onWheel = useCallback((e: React.WheelEvent) => {
    if (!e.ctrlKey && !e.metaKey) return;
    e.preventDefault();
    setFitWidth(false);
    setZoom((z) => Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, z - e.deltaY * 0.0015)));
  }, []);

  const zoomBy = useCallback((d: number) => {
    setFitWidth(false);
    setZoom((z) => Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, +(z + d).toFixed(2))));
  }, []);

  // 드래그 팬 (줌 상태에서 종이 끌어 이동)
  const panRef = useRef<{ x: number; y: number; l: number; t: number } | null>(null);
  const onMouseDown = useCallback((e: React.MouseEvent) => {
    // 텍스트 선택과 충돌 방지 — 빈 영역(SVG 배경)에서만 팬
    if ((e.target as Element).tagName?.toLowerCase() === "text") return;
    const sc = scrollRef.current;
    if (!sc) return;
    panRef.current = { x: e.clientX, y: e.clientY, l: sc.scrollLeft, t: sc.scrollTop };
  }, []);
  const onMouseMove = useCallback((e: React.MouseEvent) => {
    const p = panRef.current, sc = scrollRef.current;
    if (!p || !sc) return;
    sc.scrollLeft = p.l - (e.clientX - p.x);
    sc.scrollTop = p.t - (e.clientY - p.y);
  }, []);
  const endPan = useCallback(() => { panRef.current = null; }, []);

  // 뷰어(팝업) 모드 — Esc 로 닫기. capture 로 전역 단축키보다 먼저 소비해
  // 뒤 미리보기(찾기/프리뷰 닫힘)까지 번지지 않게 한다.
  useEffect(() => {
    if (!onClose) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        e.preventDefault();
        onClose();
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [onClose]);

  // 찾기 매치 — SVG <text> 내용에서 findTerm 포함 요소 수집·강조
  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    matchesRef.current.forEach((t) => t.removeAttribute("data-find"));
    const term = findTerm?.trim().toLowerCase();
    if (!term) { matchesRef.current = []; setMatchCount(0); setMatchIdx(0); return; }
    const hits = Array.from(host.querySelectorAll<SVGTextElement>("text"))
      .filter((t) => (t.textContent ?? "").toLowerCase().includes(term));
    hits.forEach((t) => t.setAttribute("data-find", "1"));
    matchesRef.current = hits;
    setMatchCount(hits.length);
    setMatchIdx(0);
  }, [findTerm, svg]);

  // 활성 매치 강조 + 스크롤
  useEffect(() => {
    const m = matchesRef.current;
    if (m.length === 0) return;
    m.forEach((t, i) => t.setAttribute("data-find", i === matchIdx ? "active" : "1"));
    m[matchIdx]?.scrollIntoView({ behavior: "smooth", block: "center" });
  }, [matchIdx, matchCount]);

  const navMatch = useCallback((dir: 1 | -1) => {
    setMatchIdx((i) => (matchCount > 0 ? (i + dir + matchCount) % matchCount : 0));
  }, [matchCount]);

  const widthStyle = fitWidth ? "100%" : `${(zoom * 100).toFixed(0)}%`;

  return (
    <div className="flex flex-col h-full">
      {/* 컴포넌트 스코프 SVG 스타일 — 반응형 크기 + 찾기 매치 강조 */}
      <style>{`
        .layout-svg-host svg { width: 100%; height: auto; display: block; }
        .layout-svg-host text[data-find] { fill: #b45309; }
        .layout-svg-host text[data-find="active"] { fill: #dc2626; font-weight: 700; }
      `}</style>

      {/* 툴바 — 페이지 네비 · 줌 · 매치 이동 (에디토리얼 미니멀, hairline) */}
      <div
        className="flex items-center gap-1 px-2 py-1 border-b text-[11px]"
        style={{ borderColor: "var(--color-border)", color: "var(--color-text-muted)" }}
      >
        <button onClick={() => goPage(page - 1)} disabled={page <= 1}
          className="p-1 rounded hover:bg-[var(--color-bg-tertiary)] disabled:opacity-30" title="이전 페이지" aria-label="이전 페이지">
          <ChevronLeft size={13} />
        </button>
        <span className="tabular-nums select-none">{page}/{pageCount}</span>
        <button onClick={() => goPage(page + 1)} disabled={page >= pageCount}
          className="p-1 rounded hover:bg-[var(--color-bg-tertiary)] disabled:opacity-30" title="다음 페이지" aria-label="다음 페이지">
          <ChevronRight size={13} />
        </button>

        <span className="mx-1 opacity-40">·</span>

        <button onClick={() => zoomBy(-ZOOM_STEP)} className="p-1 rounded hover:bg-[var(--color-bg-tertiary)]" title="축소" aria-label="축소">
          <ZoomOut size={13} />
        </button>
        <span className="tabular-nums select-none w-9 text-center">{fitWidth ? "맞춤" : `${Math.round(zoom * 100)}%`}</span>
        <button onClick={() => zoomBy(ZOOM_STEP)} className="p-1 rounded hover:bg-[var(--color-bg-tertiary)]" title="확대" aria-label="확대">
          <ZoomIn size={13} />
        </button>
        <button onClick={() => { setFitWidth(true); setZoom(1); }} className="p-1 rounded hover:bg-[var(--color-bg-tertiary)]" title="너비 맞춤" aria-label="너비 맞춤">
          <Maximize2 size={13} />
        </button>

        {/* 레이아웃 뷰 내 찾기 매치 이동 */}
        {matchCount > 0 && (
          <>
            <span className="mx-1 opacity-40">·</span>
            <span className="tabular-nums select-none">{matchIdx + 1}/{matchCount}</span>
            <button onClick={() => navMatch(-1)} className="p-1 rounded hover:bg-[var(--color-bg-tertiary)]" title="이전 매치" aria-label="이전 매치">
              <ArrowUp size={13} />
            </button>
            <button onClick={() => navMatch(1)} className="p-1 rounded hover:bg-[var(--color-bg-tertiary)]" title="다음 매치" aria-label="다음 매치">
              <ArrowDown size={13} />
            </button>
          </>
        )}

        {/* 인라인: 크게 보기(팝업) 진입 · 뷰어: 닫기 (우측 정렬) */}
        {onExpand && (
          <button onClick={onExpand} className="ml-auto p-1 rounded hover:bg-[var(--color-bg-tertiary)]" title="크게 보기 (팝업)" aria-label="크게 보기">
            <Expand size={13} />
          </button>
        )}
        {onClose && (
          <button onClick={onClose} className="ml-auto p-1 rounded hover:bg-[var(--color-bg-tertiary)]" title="닫기 (Esc)" aria-label="닫기">
            <X size={14} />
          </button>
        )}
      </div>

      {/* SVG 스크롤/팬 영역 — 종이 배경은 SVG 자체 포함 */}
      <div
        ref={scrollRef}
        onScroll={onScroll}
        onWheel={onWheel}
        onMouseDown={onMouseDown}
        onMouseMove={onMouseMove}
        onMouseUp={endPan}
        onMouseLeave={endPan}
        className="flex-1 overflow-auto px-4 py-3"
        style={{ backgroundColor: "var(--color-bg-tertiary)" }}
      >
        <div
          ref={hostRef}
          className="layout-svg-host mx-auto"
          style={{ width: widthStyle, maxWidth: fitWidth ? "100%" : "none" }}
          dangerouslySetInnerHTML={{ __html: svg }}
        />
      </div>
    </div>
  );
});
