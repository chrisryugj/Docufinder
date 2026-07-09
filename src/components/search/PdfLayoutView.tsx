import { memo, useRef, useState, useEffect, useLayoutEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ChevronLeft, ChevronRight, ZoomIn, ZoomOut, Maximize2, Expand, X, Loader2 } from "lucide-react";

interface PdfPageResponse {
  data_url: string;
  page_count: number;
  width: number;
  height: number;
}

interface PdfLayoutViewProps {
  /** PDF 파일 절대경로 — 페이지를 pdfium 으로 온디맨드 래스터화 */
  filePath: string;
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
 * PDF 원본 레이아웃 뷰어 — pdfium 이 렌더한 페이지 PNG(data URI)를 한 장씩 보여준다.
 * HWPX 의 LayoutView(인라인 SVG) 에 대응하는 PDF 경로. 이미지라 텍스트 선택/검색은 없고,
 * 원본 조판(스캔·벡터 무관)을 100% 보존한다. 메모리 보호를 위해 현재 페이지 1장만 로드하고
 * 페이지 이동 시 백엔드에 다시 요청한다(전 페이지 동시 로드 금지 — 대용량 PDF OOM 방지).
 */
export const PdfLayoutView = memo(function PdfLayoutView({
  filePath,
  onClose,
  onExpand,
}: PdfLayoutViewProps) {
  const scrollRef = useRef<HTMLDivElement>(null);

  const [page, setPage] = useState(1);
  const [pageCount, setPageCount] = useState(1);
  const [dataUrl, setDataUrl] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [zoom, setZoom] = useState(1); // fit=null(수동 줌)일 때 컨테이너 너비 대비 배율
  // 맞춤 모드 — "page"=전체 페이지가 보이게(기본, 중앙정렬), "width"=너비 맞춤, null=수동 줌.
  // 컨테이너 크기를 ResizeObserver 로 추적해 창/패널 리사이즈에 실시간 추종 (LayoutView 와 동일 모델).
  const [fit, setFit] = useState<"width" | "page" | null>("page");
  const [avail, setAvail] = useState<{ w: number; h: number } | null>(null);
  const [pageDims, setPageDims] = useState<{ w: number; h: number } | null>(null);

  // 핸들러 스테일 클로저 방지 미러
  const zoomRef = useRef(zoom);
  const fitRef = useRef(fit);
  const availRef = useRef(avail);
  const pageDimsRef = useRef(pageDims);
  zoomRef.current = zoom;
  fitRef.current = fit;
  availRef.current = avail;
  pageDimsRef.current = pageDims;

  // 현재 실효 줌(컨테이너 너비 대비 배율) — 맞춤 모드에서 ±/휠 줌 시작점
  const currentZoom = useCallback(() => {
    const f = fitRef.current;
    if (f === "width") return 1;
    if (f === "page") {
      const a = availRef.current, dm = pageDimsRef.current;
      if (a && dm && a.w > 0 && dm.h > 0) return Math.min(a.w, a.h * (dm.w / dm.h)) / a.w;
      return 1;
    }
    return zoomRef.current;
  }, []);

  // 컨테이너 크기 추적 — 맞춤 모드의 실시간 리사이즈 추종 근거.
  // useLayoutEffect: 페인트 전에 재야 첫 프레임부터 맞춤 폭이 나온다 (LayoutView 와 동일).
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

  // 파일 전환은 렌더 중에 즉시 재설정 — effect 로 하면 이전 page 값으로 새 파일을 한 번
  // 요청하고(대형 PDF pdfium 렌더 낭비, req 토큰으로 폐기되나 로딩 지연), 첫 응답 전까지
  // 이전 파일의 pageCount 로 범위 밖 네비 → "PDF 렌더 실패" 표시가 가능한 창구간이 생긴다.
  const [lastFile, setLastFile] = useState(filePath);
  if (lastFile !== filePath) {
    setLastFile(filePath);
    setPage(1);
    setPageCount(1);
    setDataUrl(null);
  }

  // 현재 페이지 렌더 요청 — 파일/페이지 변경 시. 늦은 응답은 req 토큰으로 폐기.
  const reqRef = useRef(0);
  useEffect(() => {
    if (!filePath) return;
    const req = ++reqRef.current;
    setLoading(true);
    setError(null);
    invoke<PdfPageResponse>("render_pdf_page", { filePath, page: page - 1 })
      .then((res) => {
        if (reqRef.current !== req) return;
        setDataUrl(res.data_url);
        setPageCount(Math.max(1, res.page_count));
        // 페이지 원본 치수 — 페이지맞춤 폭 계산용 (가로/세로 혼재 문서는 페이지마다 갱신)
        setPageDims(res.width > 0 && res.height > 0 ? { w: res.width, h: res.height } : null);
        scrollRef.current?.scrollTo(0, 0);
      })
      .catch((e) => {
        if (reqRef.current !== req) return;
        setDataUrl(null);
        setError(typeof e === "string" ? e : ((e as { message?: string })?.message ?? "PDF 렌더 실패"));
      })
      .finally(() => {
        if (reqRef.current === req) setLoading(false);
      });
  }, [filePath, page]);

  const goPage = useCallback((p: number) => {
    setPage((cur) => {
      const next = Math.max(1, Math.min(pageCount, p));
      return next === cur ? cur : next;
    });
  }, [pageCount]);

  const zoomBy = useCallback((d: number) => {
    const z = clampZoom(currentZoom() + d);
    setFit(null);
    setZoom(z);
  }, [currentZoom]);

  // 뷰어(팝업) 모드 — Esc 닫기 + Cmd/Ctrl +/-/0 줌 (LayoutView 와 동일 계약).
  // capture 로 전역 단축키·웹뷰 브라우저 줌보다 먼저 소비해 앱 전체 확대로 번지지 않게 한다.
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
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [onClose, zoomBy]);

  // 줌 — Ctrl/⌘+휠(트랙패드 핀치). 네이티브 non-passive 리스너로 붙인다 — React 의
  // onWheel 은 passive 로 등록돼 preventDefault 가 무시되고(defaultPrevented=false 실측,
  // v3.2.2 LayoutView 와 동일 결함), 컴포넌트 줌과 웹뷰 전체 브라우저 줌이 이중으로 걸려
  // 뷰어를 닫아도 앱 UI 가 확대된 채 남는다.
  useEffect(() => {
    const sc = scrollRef.current;
    if (!sc) return;
    const handler = (e: WheelEvent) => {
      if (!e.ctrlKey && !e.metaKey) return; // 일반 휠 = 네이티브 스크롤
      e.preventDefault();
      const z = clampZoom(currentZoom() - e.deltaY * 0.0015);
      setFit(null);
      setZoom(z);
    };
    sc.addEventListener("wheel", handler, { passive: false });
    return () => sc.removeEventListener("wheel", handler);
  }, [currentZoom]);

  // 더블클릭 줌 토글 — 뷰어(팝업) 모드에서만(LayoutView·도움말 계약과 동일). 확대 상태면
  // 페이지맞춤 원복, 아니면 2×. 페이지가 이미지라 텍스트 선택 충돌은 없지만 계약을 맞춘다.
  const onDoubleClick = useCallback(() => {
    if (fitRef.current === null && zoomRef.current > 1.01) {
      setFit("page");
      setZoom(1);
    } else {
      setFit(null);
      setZoom(clampZoom(2));
    }
  }, []);

  // 맞춤 토글 — 페이지 맞춤 ↔ 너비 맞춤 순환 (수동 줌 상태에선 페이지 맞춤 복귀)
  const toggleFit = useCallback(() => {
    setZoom(1);
    setFit((f) => (f === "page" ? "width" : "page"));
  }, []);

  // 맞춤 폭 계산 — page: 페이지가 컨테이너 높이에 들어오는 px 폭(너비 상한), width: 100%,
  // 수동 줌: 컨테이너 너비 대비 %. mx-auto 로 중앙정렬.
  const fitPageW =
    avail && pageDims && pageDims.h > 0
      ? Math.max(120, Math.min(avail.w, avail.h * (pageDims.w / pageDims.h)))
      : null;
  const widthStyle =
    fit === "width" || (fit === "page" && fitPageW === null)
      ? "100%"
      : fit === "page"
        ? `${Math.round(fitPageW!)}px`
        : `${(zoom * 100).toFixed(0)}%`;

  return (
    <div className="flex flex-col h-full">
      {/* 툴바 — 페이지 네비 · 줌 (LayoutView 와 동일 톤) */}
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

        {loading && <Loader2 size={13} className="animate-spin ml-1" />}

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

      {/* 페이지 이미지 스크롤 영역 — 휠 줌은 네이티브 non-passive 리스너(위 effect)로 부착 */}
      <div
        ref={scrollRef}
        onDoubleClick={onClose ? onDoubleClick : undefined}
        className="flex-1 overflow-auto px-4 py-3"
        style={{ backgroundColor: "var(--color-bg-tertiary)" }}
      >
        {error ? (
          <div className="flex items-center justify-center h-full text-[13px] text-center px-6" style={{ color: "var(--color-text-muted)" }}>
            {error}
          </div>
        ) : (
          dataUrl && (
            <img
              src={dataUrl}
              alt={`PDF 페이지 ${page}`}
              draggable={false}
              className="mx-auto block shadow-sm select-none"
              style={{ width: widthStyle, maxWidth: fit !== null ? "100%" : "none", height: "auto", backgroundColor: "white" }}
            />
          )
        )}
      </div>
    </div>
  );
});
