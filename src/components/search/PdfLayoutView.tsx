import { memo, useRef, useState, useEffect, useCallback } from "react";
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
  const [zoom, setZoom] = useState(1);
  const [fitWidth, setFitWidth] = useState(true);

  // 파일이 바뀌면 첫 페이지로 리셋
  useEffect(() => {
    setPage(1);
    setDataUrl(null);
  }, [filePath]);

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

  // 뷰어(팝업) 모드 — Esc 로 닫기 (capture 로 전역 단축키보다 먼저 소비)
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

  const zoomBy = useCallback((d: number) => {
    setFitWidth(false);
    setZoom((z) => clampZoom((fitWidth ? 1 : z) + d));
  }, [fitWidth]);

  // Ctrl/⌘+휠 = 줌 (문서 뷰어 관례), 일반 휠 = 스크롤
  const onWheel = useCallback((e: React.WheelEvent) => {
    if (!e.ctrlKey && !e.metaKey) return;
    e.preventDefault();
    setFitWidth(false);
    setZoom((z) => clampZoom((fitWidth ? 1 : z) - e.deltaY * 0.0015));
  }, [fitWidth]);

  const widthStyle = fitWidth ? "100%" : `${(zoom * 100).toFixed(0)}%`;

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
        <span className="tabular-nums select-none w-9 text-center">{fitWidth ? "맞춤" : `${Math.round(zoom * 100)}%`}</span>
        <button onClick={() => zoomBy(ZOOM_STEP)} className="p-1 rounded hover:bg-[var(--color-bg-tertiary)]" title="확대" aria-label="확대">
          <ZoomIn size={13} />
        </button>
        <button onClick={() => { setFitWidth(true); setZoom(1); }} className="p-1 rounded hover:bg-[var(--color-bg-tertiary)]" title="너비 맞춤" aria-label="너비 맞춤">
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

      {/* 페이지 이미지 스크롤 영역 */}
      <div
        ref={scrollRef}
        onWheel={onWheel}
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
              style={{ width: widthStyle, maxWidth: fitWidth ? "100%" : "none", height: "auto", backgroundColor: "white" }}
            />
          )
        )}
      </div>
    </div>
  );
});
