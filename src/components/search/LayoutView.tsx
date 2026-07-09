import { memo, useRef, useState, useEffect, useLayoutEffect, useCallback } from "react";
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
const clampZoom = (z: number) => Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, z));

/**
 * 한 페이지의 높이/폭 비를 SVG 소스에서 직접 파싱한다(kordoc 렌더 전용).
 * 페이지는 세로로 쌓이므로 viewBox 의 W 는 곧 표시 폭이고, kordoc 은 구역별 페이지 클립
 * `<clipPath id="pgclip0"><rect … height="841.88"/>` 에 페이지 높이를 굽는다(v3.16+
 * 다구역 렌더부터 pgclip 뒤에 구역 인덱스가 붙는다 — `\d*` 로 신구 형식 모두 수용, 첫
 * 매치 = 1구역). 비율 기준은 DOM 폴백과 동일한 `첫 페이지 높이 / 전체 표시 폭`이다.
 * 형식이 안 맞으면 null 을 반환해 호출부가 getBoundingClientRect 측정으로 폴백하게
 * 한다(rhwp HWP 등).
 */
function pageRatioFromSvg(svg: string): number | null {
  const vb = /viewBox="0 0 ([\d.]+) [\d.]+"/.exec(svg);
  const clip = /<clipPath id="pgclip\d*"><rect[^>]*\bheight="([\d.]+)"/.exec(svg);
  if (!vb || !clip) return null;
  const w = parseFloat(vb[1]);
  const h = parseFloat(clip[1]);
  return w > 0 && h > 0 ? h / w : null;
}

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

  const [zoom, setZoom] = useState(1); // fit=null(수동 줌)일 때 컨테이너 너비 대비 배율
  // 맞춤 모드 — "page"=전체 페이지가 보이게(기본, 중앙정렬), "width"=너비 맞춤, null=수동 줌.
  // 둘 다 컨테이너 크기를 ResizeObserver 로 추적해 창/패널 리사이즈에 실시간 추종한다.
  const [fit, setFit] = useState<"width" | "page" | null>("page");
  // 스크롤 컨테이너 내용 영역 크기 (px-4/py-3 패딩 제외) — 맞춤 계산용
  const [avail, setAvail] = useState<{ w: number; h: number } | null>(null);
  // 첫 페이지 높이/host 너비 비 — SVG 는 비례 스케일이라 이 비율은 줌과 무관(스케일 불변)
  const [pageRatio, setPageRatio] = useState<number | null>(null);
  const [page, setPage] = useState(1);
  const [pageCount, setPageCount] = useState(1);
  const [matchIdx, setMatchIdx] = useState(0);
  const [matchCount, setMatchCount] = useState(0);

  // 커서 기준 줌 보정용 — 핸들러 스테일 클로저 방지(렌더마다 최신값 미러)
  const zoomRef = useRef(zoom);
  const fitRef = useRef(fit);
  const availRef = useRef(avail);
  const pageRatioRef = useRef(pageRatio);
  zoomRef.current = zoom;
  fitRef.current = fit;
  availRef.current = avail;
  pageRatioRef.current = pageRatio;
  const zoomAnchorRef = useRef<{ fx: number; fy: number; cx: number; cy: number } | null>(null);

  // 현재 실효 줌(컨테이너 너비 대비 배율) — 맞춤 모드에서 ±/휠 줌 시작점으로 쓴다
  const currentZoom = useCallback(() => {
    const f = fitRef.current;
    if (f === "width") return 1;
    if (f === "page") {
      const a = availRef.current, r = pageRatioRef.current;
      if (a && r && a.w > 0) return Math.min(a.w, a.h / r) / a.w;
      return 1;
    }
    return zoomRef.current;
  }, []);

  // 컨테이너 크기 추적 — 맞춤 모드가 창/패널 리사이즈에 실시간 추종하는 근거.
  // useLayoutEffect: passive effect 면 avail 이 페인트 뒤에나 세팅돼 첫 프레임이 무조건
  // 100%(너비맞춤)로 그려졌다가 페이지맞춤으로 스냅한다(수십 페이지 SVG 재레이아웃 깜빡임).
  // 페인트 전에 재면 pageRatio(역시 layout effect)와 함께 첫 프레임부터 맞춤 폭이 나온다.
  useLayoutEffect(() => {
    const sc = scrollRef.current;
    if (!sc) return;
    const update = () => {
      const w = sc.clientWidth - 32, h = sc.clientHeight - 24; // px-4/py-3 패딩
      setAvail((prev) => (prev && prev.w === w && prev.h === h ? prev : { w, h }));
    };
    update();
    const ro = new ResizeObserver(update);
    ro.observe(sc);
    return () => ro.disconnect();
  }, []);

  // 페이지 수 (data-page 그룹)
  useEffect(() => {
    setPageCount(Math.max(1, (svg.match(/data-page="/g) ?? []).length));
    setPage(1);
  }, [svg]);

  // 첫 페이지 종횡비 — "맞춤" 초기 표시가 창에 정확히 들어가는지를 좌우하는 핵심 값.
  // 1순위: SVG 소스에서 직접 파싱(결정적) — kordoc 은 표시 폭을 viewBox W, 페이지 높이를
  //   `<clipPath id="pgclip0"><rect height=…>` 로 굽는다. 이 둘로 비율을 바로 얻으면
  //   렌더 타이밍·폰트 로드·컨테이너 리사이즈와 무관하게 첫 프레임부터 정확한 맞춤이 나온다.
  // 2순위: DOM 측정(fallback) — pgclip 이 없는 rhwp(HWP) 등. data-page 는 SVG <g> 라
  //   offsetHeight 가 없어(HTML 전용) getBoundingClientRect 로 잰다. 비율은 스케일 불변.
  useLayoutEffect(() => {
    const parsed = pageRatioFromSvg(svg);
    if (parsed) {
      setPageRatio(parsed);
      return;
    }
    const host = hostRef.current;
    if (!host) return;
    const pg = host.querySelector("[data-page]") ?? host.querySelector("svg");
    const hw = host.getBoundingClientRect().width;
    const ph = pg?.getBoundingClientRect().height ?? 0;
    setPageRatio(hw > 0 && ph > 0 ? ph / hw : null);
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
  // data-page 는 SVG <g> — offsetTop 이 없어(undefined) getBoundingClientRect 로 비교한다.
  const onScroll = useCallback(() => {
    const host = hostRef.current, sc = scrollRef.current;
    if (!host || !sc) return;
    const mid = sc.getBoundingClientRect().top + sc.clientHeight / 2;
    let cur = 1;
    host.querySelectorAll<Element>("[data-page]").forEach((el) => {
      if (el.getBoundingClientRect().top <= mid) cur = Number(el.getAttribute("data-page")) || cur;
    });
    setPage(cur);
  }, []);

  // 커서 기준 줌 — width-% 모델이라 transform:scale 이 아니다. 줌 전 커서 아래 지점의
  // 분율을 캡처했다가, 새 host 치수로 스크롤을 되돌려 그 지점을 고정한다(아래 보정 effect).
  const zoomToPoint = useCallback((nextZoom: number, clientX: number, clientY: number) => {
    const sc = scrollRef.current, host = hostRef.current;
    if (!sc || !host) return;
    const rect = sc.getBoundingClientRect();
    const hostRect = host.getBoundingClientRect();
    // 커서 지점의 host 내 분율 — host 의 실제 위치 기준(px-4/py-3 패딩·mx-auto 여백 포함).
    // cx/cy 는 스크롤 컨테이너 뷰포트 기준 커서 위치(보정 때 이 위치로 되돌린다).
    zoomAnchorRef.current = {
      fx: (clientX - hostRect.left) / host.offsetWidth,
      fy: (clientY - hostRect.top) / host.offsetHeight,
      cx: clientX - rect.left,
      cy: clientY - rect.top,
    };
    setFit(null);
    setZoom(clampZoom(nextZoom));
  }, []);

  // 줌 후 스크롤 보정 — 분율은 스케일 불변. host 의 콘텐츠 좌표계 오프셋(패딩+여백)을
  // 측정해 더한다 — host 가 스크롤 콘텐츠 원점에서 시작하지 않으므로(패딩·mx-auto).
  useLayoutEffect(() => {
    const a = zoomAnchorRef.current, sc = scrollRef.current, host = hostRef.current;
    if (!a || !sc || !host) return;
    const scRect = sc.getBoundingClientRect();
    const hostRect = host.getBoundingClientRect();
    const hostLeft = hostRect.left - scRect.left + sc.scrollLeft; // scrollLeft 무관한 콘텐츠 오프셋
    const hostTop = hostRect.top - scRect.top + sc.scrollTop;
    sc.scrollLeft = hostLeft + a.fx * host.offsetWidth - a.cx;
    sc.scrollTop = hostTop + a.fy * host.offsetHeight - a.cy;
    zoomAnchorRef.current = null;
  }, [zoom, fit]);

  // 줌 — Ctrl/⌘+휠(트랙패드 핀치도 이 이벤트로 도착). 일반 휠은 스크롤(문서 뷰어 관례).
  // 네이티브 non-passive 리스너로 붙인다 — React 의 onWheel 은 passive 로 등록돼
  // preventDefault 가 무시된다(defaultPrevented=false 실측). 그러면 Ctrl/⌘+휠이 컴포넌트
  // 줌과 동시에 웹뷰 전체 브라우저 줌으로 새어나가, 미리보기가 영역에 맞게 표시되지 않고
  // 확대·축소가 어긋난다.
  useEffect(() => {
    const sc = scrollRef.current;
    if (!sc) return;
    const handler = (e: WheelEvent) => {
      if (!e.ctrlKey && !e.metaKey) return; // 일반 휠 = 네이티브 스크롤
      e.preventDefault();
      zoomToPoint(currentZoom() - e.deltaY * 0.0015, e.clientX, e.clientY);
    };
    sc.addEventListener("wheel", handler, { passive: false });
    return () => sc.removeEventListener("wheel", handler);
  }, [zoomToPoint, currentZoom]);

  // ± 버튼 — 컨테이너 중앙 기준 줌
  const zoomBy = useCallback((d: number) => {
    const sc = scrollRef.current;
    if (!sc) return;
    const rect = sc.getBoundingClientRect();
    zoomToPoint(currentZoom() + d, rect.left + rect.width / 2, rect.top + rect.height / 2);
  }, [zoomToPoint, currentZoom]);

  // 더블클릭 줌 토글 — 뷰어(팝업) 모드에서만(onClose 게이트). 확대 상태면 페이지맞춤 원복,
  // 아니면 그 지점 2× 확대. 인라인엔 미부착 — SVG <text> 단어 더블클릭 선택 보존.
  const onDoubleClick = useCallback((e: React.MouseEvent) => {
    if (fitRef.current === null && zoomRef.current > 1.01) {
      setFit("page");
      setZoom(1);
      zoomAnchorRef.current = null;
    } else {
      zoomToPoint(2, e.clientX, e.clientY);
    }
  }, [zoomToPoint]);

  // 맞춤 토글 — 페이지 맞춤 ↔ 너비 맞춤 순환 (수동 줌 상태에선 페이지 맞춤 복귀)
  const toggleFit = useCallback(() => {
    zoomAnchorRef.current = null;
    setZoom(1);
    setFit((f) => (f === "page" ? "width" : "page"));
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

  // 뷰어(팝업) 모드 — Esc 로 닫기 + Cmd/Ctrl +/-/0 줌. capture 로 전역 단축키·웹뷰
  // 브라우저 줌보다 먼저 소비해 뒤 미리보기나 앱 전체 확대로 번지지 않게 한다.
  useEffect(() => {
    if (!onClose) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        e.preventDefault();
        onClose();
        return;
      }
      if (!(e.ctrlKey || e.metaKey)) return;
      if (e.key === "=" || e.key === "+") {
        e.preventDefault();
        zoomBy(ZOOM_STEP);
      } else if (e.key === "-" || e.key === "_") {
        e.preventDefault();
        zoomBy(-ZOOM_STEP);
      } else if (e.key === "0") {
        e.preventDefault();
        setFit("page");
        setZoom(1);
        zoomAnchorRef.current = null;
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [onClose, zoomBy]);

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

  // 활성 매치 강조 + 스크롤 — 먼 매치는 즉시 점프(수십 페이지를 smooth 로 훑는 장시간
  // 스크롤 방지), 가까운 매치만 smooth.
  useEffect(() => {
    const m = matchesRef.current;
    if (m.length === 0) return;
    m.forEach((t, i) => t.setAttribute("data-find", i === matchIdx ? "active" : "1"));
    const target = m[matchIdx], sc = scrollRef.current;
    if (target) {
      const far = sc
        ? Math.abs(target.getBoundingClientRect().top - sc.getBoundingClientRect().top - sc.clientHeight / 2) > sc.clientHeight * 2
        : false;
      target.scrollIntoView({ behavior: far ? "auto" : "smooth", block: "center" });
    }
    // findTerm·svg 도 의존성에 포함: 매치 수가 이전과 같은 새 검색어(예: 3건→3건)는
    // matchIdx·matchCount 가 안 바뀌어 active 강조·스크롤이 갱신되지 않는다. 수집 effect가
    // 먼저(선언순) 실행돼 matchesRef 를 재구성한 뒤 이 effect가 읽으므로 순서 안전.
  }, [matchIdx, matchCount, findTerm, svg]);

  const navMatch = useCallback((dir: 1 | -1) => {
    setMatchIdx((i) => (matchCount > 0 ? (i + dir + matchCount) % matchCount : 0));
  }, [matchCount]);

  // 맞춤 폭 계산 — page: 첫 페이지가 컨테이너 높이에 들어오는 px 폭(너비 상한), width: 100%,
  // 수동 줌: 컨테이너 너비 대비 % (컨테이너가 넓어지면 비례해서 넓어진다). mx-auto 로 중앙정렬.
  const fitPageW =
    avail && pageRatio ? Math.max(120, Math.min(avail.w, avail.h / pageRatio)) : null;
  const widthStyle =
    fit === "width" || (fit === "page" && fitPageW === null)
      ? "100%"
      : fit === "page"
        ? `${Math.round(fitPageW!)}px`
        : `${(zoom * 100).toFixed(0)}%`;

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
        <span className="tabular-nums select-none w-9 text-center">{fit === "page" ? "맞춤" : fit === "width" ? "너비" : `${Math.round(zoom * 100)}%`}</span>
        <button onClick={() => zoomBy(ZOOM_STEP)} className="p-1 rounded hover:bg-[var(--color-bg-tertiary)]" title="확대" aria-label="확대">
          <ZoomIn size={13} />
        </button>
        <button onClick={toggleFit} className="p-1 rounded hover:bg-[var(--color-bg-tertiary)]" title={fit === "page" ? "너비 맞춤" : "페이지 맞춤"} aria-label={fit === "page" ? "너비 맞춤" : "페이지 맞춤"}>
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
        onDoubleClick={onClose ? onDoubleClick : undefined}
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
          style={{ width: widthStyle, maxWidth: fit !== null ? "100%" : "none" }}
          dangerouslySetInnerHTML={{ __html: svg }}
        />
      </div>
    </div>
  );
});
