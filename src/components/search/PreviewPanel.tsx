import { memo, useEffect, useState, useRef, useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { X, FileText, Copy, ExternalLink, FolderOpen, Bookmark, Sparkles, MessageSquare, ClipboardCopy, Save, Search, MoreHorizontal, Tag, AlertTriangle, RotateCw, ScanText } from "lucide-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import rehypeKatex from "rehype-katex";
import rehypeRaw from "rehype-raw";
import rehypeSanitize from "rehype-sanitize";
import "katex/dist/katex.min.css";
import { save } from "@tauri-apps/plugin-dialog";
import { HighlightedFilename } from "./HighlightedFilename";
import { FileIcon } from "../ui/FileIcon";
import { LayoutView } from "./LayoutView";
import { PdfLayoutView } from "./PdfLayoutView";
import { Badge, getFileTypeBadgeVariant } from "../ui/Badge";
import { Tooltip } from "../ui/Tooltip";
import { TagInput } from "../ui/TagInput";
import { cleanPath } from "../../utils/cleanPath";
import { MOD_KEY, REVEAL_LABEL } from "../../utils/platform";
import { IS_LITE } from "../../utils/buildFlavor";
import { getErrorMessage } from "../../types/error";
import { useUIActions } from "../../contexts/UIContext";
import { PREVIEW_SANITIZE_SCHEMA, createMarkdownComponents, findJumpTarget } from "./preview/markdown";
import { usePreviewFind } from "./preview/usePreviewFind";
import { FindBar } from "./preview/FindBar";
import { PasswordGate } from "./preview/PasswordGate";
import { AiSection, SummaryMenu, useAiSummary } from "./preview/AiSummary";
import { LayoutViewerDialog } from "./preview/LayoutViewerDialog";
import { OCR_ELIGIBLE_RE } from "./ResultContextMenu";

// ─── Types ─────────────────────────────────────────────

interface MarkdownPreviewResponse {
  file_path: string;
  file_name: string;
  markdown: string;
  /** 복사 시 글자가 깨지는 문서(CID/PUA 매핑 손상)로 감지됨 */
  garbled: boolean;
  /** 열기 암호가 필요한 문서 — 비밀번호 입력을 띄운다.
   *  암호를 넣어 재요청했는데도 true 면 입력한 암호가 틀린 것. */
  needs_password?: boolean;
}

/** 인용 점프 타깃 — AI 답변 [출처N] 클릭 시 부모가 내려보냄.
 *  token은 nonce: 같은 파일/같은 앵커라도 token이 바뀌면 점프 재실행. */
export interface PreviewJumpTarget {
  anchors: string[];
  page: number | null;
  token: number;
}

interface PreviewPanelProps {
  filePath: string | null;
  highlightQuery?: string;
  /** AI 인용 점프 타깃 (없으면 일반 미리보기) */
  jumpTarget?: PreviewJumpTarget;
  onClose: () => void;
  onOpenFile?: (filePath: string, page?: number | null) => void;
  onCopyPath?: (path: string) => void;
  onOpenFolder?: (path: string) => void;
  onBookmark?: (filePath: string, contentPreview: string, pageNumber?: number | null, locationHint?: string | null) => void;
  /** 본문이 비었을 때 "OCR로 다시 읽기" (스캔 문서) */
  /** OCR 로 다시 읽기 — 끝날 때까지 기다린 뒤 미리보기를 다시 불러온다 */
  onOcrReindex?: (filePath: string) => void | Promise<void>;
  isBookmarked?: boolean;
  tags?: string[];
  tagSuggestions?: string[];
  onAddTag?: (filePath: string, tag: string) => void;
  onRemoveTag?: (filePath: string, tag: string) => void;
}

// 렌더마다 새 배열을 만들면 ReactMarkdown 이 매번 다시 파싱한다.
// rehypePlugins 순서: raw(원시 HTML 표 파싱) → sanitize(XSS 차단) → katex(수식). katex 출력은
// sanitize 뒤라 그대로 통과한다. kordoc의 병합·중첩 HTML 표는 여기서 네이티브로 렌더된다.
const BODY_REMARK = [[remarkGfm, { singleTilde: false }], remarkMath] as const;
const BODY_REHYPE = [rehypeRaw, [rehypeSanitize, PREVIEW_SANITIZE_SCHEMA], rehypeKatex] as const;
// HTML 이 없는 문서는 rehype-raw 의 트리 재파싱을 건너뛴다 (sanitize·katex 는 유지).
const BODY_REHYPE_NO_RAW = [[rehypeSanitize, PREVIEW_SANITIZE_SCHEMA], rehypeKatex] as const;
const HAS_HTML_TAG = /<\/?[a-z][^>]*>/i;

// ─── 미리보기 뷰 선호 (localStorage 전역 지속) ──
// 마지막으로 고른 뷰를 기억해 다음 HWPX 파일에도 적용 (파일 전환마다 markdown 리셋 대신).
type PreviewView = "markdown" | "layout";
const PREF_VIEW_KEY = "docufinder:preview-view";
const readPreferredView = (): PreviewView => {
  try {
    return localStorage.getItem(PREF_VIEW_KEY) === "layout" ? "layout" : "markdown";
  } catch {
    return "markdown";
  }
};
const writePreferredView = (mode: PreviewView) => {
  try { localStorage.setItem(PREF_VIEW_KEY, mode); } catch { /* private mode 등 무시 */ }
};

/** 본문 불러오는 동안의 자리 표시 (스피너 대신 문서 윤곽) */
function PreviewSkeleton() {
  const widths = ["62%", "94%", "88%", "97%", "71%", "0", "90%", "84%", "56%"];
  return (
    <div className="px-6 py-5 space-y-2.5" aria-busy="true" aria-label="미리보기 불러오는 중">
      {widths.map((w, i) =>
        w === "0" ? <div key={i} className="h-2" /> : (
          <div key={i} className="h-3 rounded skeleton-shimmer" style={{ width: w }} />
        ),
      )}
    </div>
  );
}

// ─── PreviewPanel ──────────────────────────────────────

export const PreviewPanel = memo(function PreviewPanel({
  filePath,
  highlightQuery,
  jumpTarget,
  onClose,
  onOpenFile,
  onCopyPath,
  onOpenFolder,
  onBookmark,
  onOcrReindex,
  isBookmarked,
  tags = [],
  tagSuggestions = [],
  onAddTag,
  onRemoveTag,
}: PreviewPanelProps) {
  const [markdown, setMarkdown] = useState<string | null>(null);
  // 복사 시 글자 깨짐 표식 — 최종 반환 markdown 기준 판정값 (백엔드 계산)
  const [garbled, setGarbled] = useState(false);
  /** 열기 암호가 필요한 문서 — 입력 폼을 띄운다 */
  const [needsPassword, setNeedsPassword] = useState(false);
  const [passwordInput, setPasswordInput] = useState("");
  /** 암호 재시도 진행 중 */
  const [unlocking, setUnlocking] = useState(false);
  /** 입력한 암호가 틀렸다 (한 번이라도 시도한 뒤) */
  const [passwordWrong, setPasswordWrong] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  /** "다시 시도" 로 같은 파일을 다시 불러올 때 올린다 */
  const [reloadKey, setReloadKey] = useState(0);
  const contentRef = useRef<HTMLDivElement>(null);
  // 비동기 응답이 도착했을 때 아직 같은 파일인지 확인용 (암호 해제 도중 파일 전환 대비)
  const filePathRef = useRef(filePath);
  filePathRef.current = filePath;
  // 찾기(Ctrl+F) 대상은 문서 본문 한정 — AI 요약·질문답변 영역 제외용 전용 ref
  const previewBodyRef = useRef<HTMLDivElement>(null);

  const summary = useAiSummary(filePath);

  // 파일 질문 토글
  const [showFileQa, setShowFileQa] = useState(false);

  // 더보기(⋯) 메뉴 토글 — 파일 위치·복사/내보내기·태그 추가를 담는 오버플로 팝오버
  const [showMoreMenu, setShowMoreMenu] = useState(false);
  const moreMenuRef = useRef<HTMLDivElement>(null);
  // 태그 패널 — 태그가 있거나 ⋯>태그 추가로 열었을 때만 노출 (빈 태그 바 상시 노출 제거)
  const [tagPanelOpen, setTagPanelOpen] = useState(false);

  const panelRef = useRef<HTMLDivElement>(null);

  // 레이아웃 뷰 (kordoc render SVG) — HWPX 전용, 파일당 1회 렌더 후 캐시.
  const [viewMode, setViewMode] = useState<PreviewView>("markdown");
  const [layoutSvg, setLayoutSvg] = useState<string | null>(null);
  const [layoutLoading, setLayoutLoading] = useState(false);
  const [viewerOpen, setViewerOpen] = useState(false); // 문서 크게 보기(팝업) 오버레이
  const layoutReqRef = useRef(0); // 파일 전환 시 in-flight 렌더 응답 무효화
  const prefAppliedRef = useRef<string | null>(null); // 선호 뷰 자동 진입: 파일당 1회
  const desiredViewRef = useRef<PreviewView>("markdown"); // 의도한 뷰 — in-flight 렌더가 사용자 전환/점프를 덮지 않게

  const { showToast, updateToast } = useUIActions();

  // 파싱된 텍스트 복사
  const handleCopyText = useCallback(async () => {
    if (!markdown) return;
    setShowMoreMenu(false);
    try {
      await navigator.clipboard.writeText(markdown);
      showToast(`텍스트 복사 완료 (${markdown.length.toLocaleString()}자)`, "success");
    } catch {
      showToast("텍스트 복사 실패", "error");
    }
  }, [markdown, showToast]);

  // Markdown 파일로 저장
  const handleExportMarkdown = useCallback(async () => {
    if (!markdown || !filePath) return;
    setShowMoreMenu(false);
    const baseName = cleanPath(filePath).split(/[\\/]/).pop() || "preview";
    const stem = baseName.replace(/\.[^.]+$/, "") || "preview";
    const safeName = stem.replace(/[<>:"/\\|?*]+/g, "_");
    let outputPath: string | null = null;
    try {
      outputPath = await save({
        defaultPath: `${safeName}.md`,
        filters: [{ name: "Markdown", extensions: ["md"] }],
      });
    } catch {
      showToast("파일 저장 창 열기 실패", "error");
      return;
    }
    if (!outputPath) return; // 사용자 취소
    const toastId = showToast("Markdown 저장 중", "loading");
    try {
      await invoke("export_markdown", { content: markdown, outputPath });
      updateToast(toastId, { message: "Markdown 파일로 저장했습니다", type: "success" });
    } catch (e) {
      updateToast(toastId, { message: `저장 실패: ${getErrorMessage(e)}`, type: "error" });
    }
  }, [markdown, filePath, showToast, updateToast]);

  // 파일 변경 시 본문 로드 + 패널 상태 초기화
  useEffect(() => {
    if (!filePath) {
      setMarkdown(null);
      setGarbled(false);
      return;
    }
    setShowFileQa(false);
    setShowMoreMenu(false);
    setTagPanelOpen(false);
    layoutReqRef.current++;
    setViewMode("markdown");
    desiredViewRef.current = "markdown";
    setLayoutSvg(null);
    setLayoutLoading(false);
    setViewerOpen(false);
    // 파일이 (재)열릴 때마다 선호 뷰를 다시 적용 — 같은 파일을 닫았다 다시 열어도 원본
    // 레이아웃 선호가 유지되도록. (이 리셋이 없으면 prefAppliedRef 가 남아 재적용을 건너뜀)
    prefAppliedRef.current = null;

    let cancelled = false;
    setLoading(true);
    setError(null);
    setGarbled(false);
    setNeedsPassword(false);
    setPasswordInput("");
    setPasswordWrong(false);

    // 빠른 탐색 시 불필요한 파싱 방지를 위해 300ms debounce (화살표 키 고속 이동 대응)
    const timer = setTimeout(() => {
      invoke<MarkdownPreviewResponse>("load_markdown_preview", { filePath, password: null })
        .then((res) => {
          if (!cancelled) {
            setMarkdown(res.markdown);
            setGarbled(res.garbled ?? false);
            setNeedsPassword(res.needs_password ?? false);
            setLoading(false);
            contentRef.current?.scrollTo(0, 0);
          }
        })
        .catch((e) => {
          if (!cancelled) {
            setError(getErrorMessage(e));
            setLoading(false);
          }
        });
    }, 300);

    return () => { cancelled = true; clearTimeout(timer); };
  }, [filePath, reloadKey]);

  // 인용 점프 — AI 답변 [출처N] 클릭 시 앵커 위치로 스크롤 + 플래시 하이라이트.
  // markdown 렌더 완료 후 실행해야 DOM 검색 가능 (300ms debounce + async fetch 고려).
  const jumpDoneRef = useRef(0);
  useEffect(() => {
    if (!jumpTarget || loading || !markdown) return;
    if (jumpDoneRef.current === jumpTarget.token) return; // 같은 점프 중복 방지
    desiredViewRef.current = "markdown"; // 인용 점프는 항상 마크다운 — in-flight 레이아웃 렌더가 덮지 못하게
    if (viewMode === "layout") {
      // 레이아웃 뷰에선 본문 DOM 이 없어 점프 불가 — 마크다운 뷰로 복귀만 하고
      // done 마킹 없이 반환, viewMode 가 바뀐 재실행에서 점프한다.
      setViewMode("markdown");
      return;
    }
    const root = contentRef.current;
    if (!root) return;

    const raf = requestAnimationFrame(() => {
      const el = findJumpTarget(root, jumpTarget.anchors, jumpTarget.page);
      jumpDoneRef.current = jumpTarget.token;
      if (el) {
        // 먼 타깃은 즉시 점프 — 장문(hwp/hwpx 수십 페이지) 문서에서 smooth 가 문서
        // 전체를 몇 초간 훑어 내려가는 "혼자 스크롤되는" 증상을 막는다. 가까울 때만 smooth.
        const dist = Math.abs(
          el.getBoundingClientRect().top - root.getBoundingClientRect().top - root.clientHeight / 2,
        );
        el.scrollIntoView({ block: "center", behavior: dist > root.clientHeight * 1.5 ? "auto" : "smooth" });
        el.classList.add("cite-flash");
        window.setTimeout(() => el.classList.remove("cite-flash"), 2200);
      } else {
        root.scrollTo({ top: 0, behavior: root.scrollTop > root.clientHeight * 1.5 ? "auto" : "smooth" });
      }
    });
    return () => cancelAnimationFrame(raf);
  }, [jumpTarget, loading, markdown, viewMode]);

  // URL 열기
  const handleOpenUrl = useCallback((url: string) => {
    invoke("open_url", { url }).catch(() => {});
  }, []);

  // 검색어 정규식
  const searchRegex = useMemo(() => {
    if (!highlightQuery?.trim()) return null;
    const keywords = highlightQuery.trim().split(/\s+/).filter(Boolean);
    if (keywords.length === 0) return null;
    const pattern = keywords.map(k => k.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("|");
    return new RegExp(pattern, "gi");
  }, [highlightQuery]);

  // 본문·AI 요약 공용 — 찾기(Ctrl+F) 하이라이트는 렌더 후 CSS Custom Highlight
  // 로 적용되므로 (T3-6) components 는 찾기 상태와 무관하다
  const markdownComponents = useMemo(
    () => createMarkdownComponents(searchRegex, handleOpenUrl),
    [searchRegex, handleOpenUrl],
  );

  // 본문 파싱 캐시 — react-markdown은 memo가 아니라 부모 리렌더마다 remark/katex
  // 전체를 재파싱한다. 매치 이동·메뉴 토글 등 무관한 상태 변화에 수만 줄 문서를
  // 재파싱하지 않도록 내용과 검색어 확정값(searchRegex)이 바뀔 때만 재구성한다.
  // 찾기(Ctrl+F)는 DOM Range 하이라이트라 여기 관여하지 않는다 (T3-6).
  const previewMarkdownNode = useMemo(() => {
    if (!markdown) return null;
    return (
      <ReactMarkdown
        remarkPlugins={BODY_REMARK as never}
        rehypePlugins={(HAS_HTML_TAG.test(markdown) ? BODY_REHYPE : BODY_REHYPE_NO_RAW) as never}
        components={markdownComponents}
      >
        {markdown}
      </ReactMarkdown>
    );
  }, [markdown, markdownComponents]);

  const find = usePreviewFind({
    filePath,
    markdown,
    loading,
    bodyRef: previewBodyRef,
    bodyNode: previewMarkdownNode,
  });
  const { findOpen, findTerm, closeFind, handleFindToggle, handlePanelKeyDown } = find;

  // ── 레이아웃 뷰 (kordoc render SVG) ──

  // 레이아웃 렌더 요청 — 성공 시 레이아웃 뷰 전환. 검색어(highlightQuery)는 SVG 안에
  // 형광펜 rect 로 구워져 온다. silent 는 검색어 변경 자동 재렌더용 (실패 토스트 억제).
  const requestLayoutRender = useCallback((silent = false) => {
    if (!filePath) return;
    const req = ++layoutReqRef.current;
    setLayoutLoading(true);
    invoke<string>("render_layout_svg", { filePath, highlightQuery: highlightQuery?.trim() || null })
      .then((svg) => {
        if (layoutReqRef.current !== req) return; // 파일 전환됨 — 늦은 응답 폐기
        setLayoutSvg(svg); // 캐시는 항상 저장 (다음 레이아웃 전환 즉시)
        // 렌더 도중 사용자가 마크다운으로 전환/인용 점프했으면 뷰를 덮지 않는다.
        if (desiredViewRef.current === "layout") setViewMode("layout");
      })
      .catch((e) => {
        if (layoutReqRef.current !== req) return;
        setViewMode("markdown");
        if (!silent) {
          showToast(`원본 레이아웃을 그리지 못했어요. 문서 텍스트로 보여 드릴게요. (${getErrorMessage(e)})`, "error");
        }
      })
      .finally(() => {
        if (layoutReqRef.current === req) setLayoutLoading(false);
      });
  }, [filePath, highlightQuery, showToast]);

  // 뷰 명시 선택 (세그먼트·단축키 공용) — 고른 뷰를 localStorage 에 선호로 기억.
  // 레이아웃: 캐시 있으면 즉시, 없으면 렌더 후 전환. 실패(HWP·조판 캐시 없음 등) 시
  // requestLayoutRender 가 에러 토스트만 띄우고 마크다운 뷰 유지 — 세그먼트는 남아 재시도 가능.
  const selectView = useCallback((mode: PreviewView) => {
    desiredViewRef.current = mode;
    writePreferredView(mode);
    if (mode === "markdown") {
      setViewMode("markdown");
      return;
    }
    if (viewMode === "layout") return;
    closeFind();
    // PDF 는 SVG 렌더(render_layout_svg) 없이 PdfLayoutView 가 페이지 이미지를 자체 로드한다
    if (filePath?.split(".").pop()?.toLowerCase() === "pdf") {
      setViewMode("layout");
      return;
    }
    if (layoutSvg) {
      setViewMode("layout");
      return;
    }
    if (layoutLoading) return;
    requestLayoutRender();
  }, [viewMode, layoutSvg, layoutLoading, closeFind, requestLayoutRender, filePath]);

  // 검색어가 바뀌면 캐시 무효화 (형광펜이 SVG 에 박제돼 있음) — 레이아웃 뷰 열람 중이면 재렌더
  const prevHlRef = useRef(highlightQuery);
  useEffect(() => {
    if (prevHlRef.current === highlightQuery) return;
    prevHlRef.current = highlightQuery;
    setLayoutSvg(null);
    if (viewMode === "layout") requestLayoutRender(true);
  }, [highlightQuery, viewMode, requestLayoutRender]);

  // 레이아웃 뷰 진입 시 스크롤 초기화 (이전 뷰의 스크롤 위치 잔상 방지)
  useEffect(() => {
    if (viewMode === "layout") contentRef.current?.scrollTo(0, 0);
  }, [viewMode]);

  // 선호 뷰 자동 진입 — 선호가 레이아웃이고 HWPX면 파일 열 때 자동으로 레이아웃 렌더(파일당 1회).
  // 대기 중인 인용 점프가 있으면 마크다운(인용 위치)을 우선한다. silent 렌더라 실패해도 조용히 유지.
  useEffect(() => {
    if (!filePath || loading || markdown === null) return;
    if (prefAppliedRef.current === filePath) return;
    prefAppliedRef.current = filePath;
    if (jumpTarget && jumpDoneRef.current !== jumpTarget.token) return;
    const pvExt = filePath.split(".").pop()?.toLowerCase();
    if (pvExt !== "hwpx" && pvExt !== "hwp") return;
    if (readPreferredView() !== "layout") return;
    desiredViewRef.current = "layout";
    requestLayoutRender(true);
  }, [filePath, loading, markdown, jumpTarget, requestLayoutRender]);

  // 뷰 전환 단축키 (1=문서 텍스트, 2=원본 레이아웃) — HWPX·HWP·PDF·입력창 밖에서만.
  // 앱 기존 bare-key 전역 단축키(`/`)와 같은 패턴. 입력/텍스트영역 포커스 시엔 비활성.
  useEffect(() => {
    const extLower = filePath?.split(".").pop()?.toLowerCase();
    if (!filePath || (extLower !== "hwpx" && extLower !== "pdf" && extLower !== "hwp")) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey || e.metaKey || e.altKey || e.shiftKey) return;
      // 모달(설정·도움말·문서비교 등)이 떠 있으면 가려진 미리보기를 몰래 전환하지 않는다
      // (앱 기존 bare-key 단축키 관례 — useKeyboardShortcuts 와 동일).
      if (document.querySelector("[role='dialog']")) return;
      const t = e.target as HTMLElement | null;
      if (t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.isContentEditable)) return;
      if (e.key === "1") { e.preventDefault(); selectView("markdown"); }
      else if (e.key === "2") { e.preventDefault(); selectView("layout"); }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [filePath, selectView]);

  // 복사/내보내기 드롭다운 — 바깥 클릭 시 닫기
  useEffect(() => {
    if (!showMoreMenu) return;
    const onPointerDown = (e: MouseEvent) => {
      if (moreMenuRef.current && !moreMenuRef.current.contains(e.target as Node)) {
        setShowMoreMenu(false);
      }
    };
    document.addEventListener("mousedown", onPointerDown);
    return () => document.removeEventListener("mousedown", onPointerDown);
  }, [showMoreMenu]);

  if (!filePath) return null;

  const ext = filePath.split(".").pop()?.toLowerCase() || "";
  const fileName = filePath.split(/[/\\]/).pop() || filePath;
  const hasAiContent = summary.aiSummary || summary.summaryError || summary.summaryLoading || showFileQa;
  // 원본 레이아웃 세그먼트: HWPX=kordoc render SVG, HWP=rhwp 네이티브 SVG (둘 다 LayoutView 소비)
  const isHwpx = ext === "hwpx";
  const isHwp = ext === "hwp";
  // PDF 는 pdfium 페이지 이미지로 원본 레이아웃을 본다 (SVG 경로와 별개, PdfLayoutView)
  const isPdf = ext === "pdf";
  const findTermForView = findOpen ? findTerm.trim() || undefined : undefined;

  /** 입력한 암호로 문서를 다시 연다 (kordoc v4.4.0+ — HWPX·HWP3·HWP5) */
  const unlockWithPassword = async () => {
    if (!passwordInput || unlocking) return;
    const target = filePath;
    setUnlocking(true);
    setPasswordWrong(false);
    try {
      const res = await invoke<MarkdownPreviewResponse>("load_markdown_preview", {
        filePath: target,
        password: passwordInput,
      });
      // 기다리는 사이 다른 파일로 바뀌었으면 이 응답(앞 파일 본문)은 버린다
      if (filePathRef.current !== target) return;
      if (res.needs_password) {
        // 암호가 틀렸다 — 입력값은 남겨 사용자가 고쳐 넣게 한다
        setPasswordWrong(true);
        return;
      }
      setMarkdown(res.markdown);
      setGarbled(res.garbled ?? false);
      setNeedsPassword(false);
      setPasswordInput("");
      contentRef.current?.scrollTo(0, 0);
    } catch (e) {
      if (filePathRef.current === target) setError(getErrorMessage(e));
    } finally {
      setUnlocking(false);
    }
  };

  const iconBtn = (active: boolean) =>
    `p-1.5 rounded-lg transition-colors ${active ? "text-[var(--color-accent)] bg-[var(--color-accent-light)]" : "text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-tertiary)]"}`;

  return (
    <div ref={panelRef} onKeyDown={handlePanelKeyDown} className="preview-panel flex flex-col h-full border-l bg-[var(--color-bg-primary)]" style={{ borderColor: "var(--color-border)", minWidth: "320px" }}>
      {/* 헤더 */}
      <div className="flex items-center gap-2 px-3 py-2 border-b bg-[var(--color-bg-secondary)]" style={{ borderColor: "var(--color-border)" }}>
        <FileIcon fileName={fileName} size="sm" />
        <span className="flex-1 min-w-0 overflow-hidden text-sm font-medium text-[var(--color-text-primary)]" title={fileName}>
          <HighlightedFilename filename={fileName} query="" middle />
        </span>
        {garbled && (
          <Tooltip content="복사하면 글자가 깨질 수 있는 문서" position="bottom" delay={200}>
            <Badge variant="warning" aria-label="복사하면 글자가 깨질 수 있는 문서">
              <AlertTriangle className="w-3 h-3" />
            </Badge>
          </Tooltip>
        )}
        <Badge variant={getFileTypeBadgeVariant(fileName)}>
          {ext.toUpperCase()}
        </Badge>
        <button onClick={onClose} className="p-1 rounded-lg hover:bg-[var(--color-bg-tertiary)] text-[var(--color-text-muted)] transition-colors" title="닫기" aria-label="미리보기 닫기">
          <X size={14} />
        </button>
      </div>

      {/* 통합 툴바 — 뷰 세그먼트(좌) + 액션(우) 한 줄. 파일위치·복사/내보내기·태그추가는 ⋯ 로 접어 정돈. */}
      <div className="flex items-center gap-1 px-2 py-1 border-b" style={{ borderColor: "var(--color-border)" }}>
        {/* 뷰 세그먼트 — 문서 텍스트 ↔ 원본 레이아웃 (HWPX SVG · PDF 페이지 이미지). 단축키 1·2. */}
        {(isHwpx || isPdf || isHwp) && markdown !== null && !loading && !error && (
          <div role="radiogroup" aria-label="미리보기 보기 방식" className="inline-flex gap-0.5 p-0.5 rounded-lg shrink-0" style={{ backgroundColor: "var(--color-bg-tertiary)" }}>
            {([
              { mode: "markdown", label: "문서 텍스트", key: "1" },
              { mode: "layout", label: "원본 레이아웃", key: "2" },
            ] as const).map(({ mode, label, key }) => {
              const active = viewMode === mode;
              const busy = mode === "layout" && layoutLoading;
              return (
                <button
                  key={mode}
                  role="radio"
                  aria-checked={active}
                  onClick={() => selectView(mode)}
                  className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md text-2xs font-medium transition-colors"
                  style={active
                    ? { backgroundColor: "var(--color-bg-secondary)", color: "var(--color-accent)", boxShadow: "var(--shadow-sm)" }
                    : { color: "var(--color-text-muted)" }}
                  title={`${label} · 단축키 ${key}`}
                >
                  {busy && <span className="w-3 h-3 border border-current border-t-transparent rounded-full animate-spin" />}
                  {label}
                </button>
              );
            })}
          </div>
        )}

        {/* 액션 — 우측 정렬. 자주 쓰는 것만 인라인, 나머지는 ⋯ */}
        <div className="ml-auto flex items-center gap-0.5 shrink-0">
          <button onClick={() => onOpenFile?.(filePath)} className={iconBtn(false)} title="파일 열기" aria-label="파일 열기">
            <ExternalLink size={14} />
          </button>
          {markdown && (
            <>
              <button
                onClick={handleFindToggle}
                className={iconBtn(findOpen)}
                title={`문서 내 찾기 (${MOD_KEY}+F)`}
                aria-label="문서 내 찾기"
                aria-pressed={findOpen}
              >
                <Search size={14} />
              </button>
              {/* lite: summarize_ai / ask_ai_file 이 항상 Err — AI 요약·파일 질문 진입점을 숨긴다 */}
              {!IS_LITE && (
                <>
                  <button
                    onClick={() => summary.setShowSummaryMenu((v) => !v)}
                    disabled={summary.summaryLoading}
                    className={`${iconBtn(false)} disabled:opacity-50`}
                    title="AI 요약"
                    aria-label="AI 요약"
                    aria-expanded={summary.showSummaryMenu}
                  >
                    {summary.summaryLoading
                      ? <div className="w-3.5 h-3.5 border border-[var(--color-accent)] border-t-transparent rounded-full animate-spin" />
                      : <Sparkles size={14} />
                    }
                  </button>
                  <button
                    onClick={() => setShowFileQa((v) => !v)}
                    className={iconBtn(showFileQa)}
                    title="이 파일에 대해 질문"
                    aria-label="이 파일에 대해 질문"
                    aria-pressed={showFileQa}
                  >
                    <MessageSquare size={14} />
                  </button>
                </>
              )}
            </>
          )}
          {onBookmark && (
            <button
              onClick={() => onBookmark(filePath, markdown?.slice(0, 200) || "", null, null)}
              className={`p-1.5 rounded-lg transition-colors ${isBookmarked ? "text-[var(--color-accent)]" : "text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-tertiary)]"}`}
              title={isBookmarked ? "북마크 해제" : "북마크 추가"}
              aria-label={isBookmarked ? "북마크 해제" : "북마크 추가"}
              aria-pressed={!!isBookmarked}
            >
              <Bookmark size={14} fill={isBookmarked ? "currentColor" : "none"} />
            </button>
          )}
          {/* ⋯ 더보기 — 파일 위치·복사/내보내기·태그 추가 (우측 정렬이라 메뉴는 right-0) */}
          <div className="relative inline-flex" ref={moreMenuRef}>
            <button
              onClick={() => setShowMoreMenu((v) => !v)}
              className={iconBtn(showMoreMenu)}
              title="더보기"
              aria-label="더보기"
              aria-haspopup="menu"
              aria-expanded={showMoreMenu}
            >
              <MoreHorizontal size={14} />
            </button>
            {showMoreMenu && (
              <div
                role="menu"
                className="absolute right-0 top-full mt-1.5 z-20 min-w-[168px] py-1 rounded-lg border overflow-hidden"
                style={{ borderColor: "var(--color-border)", backgroundColor: "var(--color-bg-secondary)", boxShadow: "var(--shadow-premium)" }}
              >
                <button onClick={() => { setShowMoreMenu(false); onOpenFolder?.(filePath); }} role="menuitem" className="export-dropdown-item" title={REVEAL_LABEL}>
                  <FolderOpen size={13} />파일 위치 열기
                </button>
                {markdown && (
                  <>
                    <div className="my-1 border-t" style={{ borderColor: "var(--color-border)" }} />
                    <button onClick={handleCopyText} role="menuitem" className="export-dropdown-item" title="읽어 낸 텍스트를 클립보드에 복사">
                      <ClipboardCopy size={13} />텍스트 복사
                    </button>
                    <button onClick={handleExportMarkdown} role="menuitem" className="export-dropdown-item" title=".md 파일로 저장">
                      <Save size={13} />Markdown 저장
                    </button>
                  </>
                )}
                <button onClick={() => { setShowMoreMenu(false); onCopyPath?.(filePath); }} role="menuitem" className="export-dropdown-item" title="파일 경로를 클립보드에 복사">
                  <Copy size={13} />경로 복사
                </button>
                {onAddTag && (
                  <>
                    <div className="my-1 border-t" style={{ borderColor: "var(--color-border)" }} />
                    <button onClick={() => { setShowMoreMenu(false); setTagPanelOpen(true); }} role="menuitem" className="export-dropdown-item" title="태그 추가">
                      <Tag size={13} />태그 추가
                    </button>
                  </>
                )}
              </div>
            )}
          </div>
        </div>
      </div>

      {/* 요약 유형 선택 메뉴 */}
      {summary.showSummaryMenu && <SummaryMenu summary={summary} />}

      {/* 태그 — 태그가 있거나 ⋯>태그 추가로 열었을 때만. 빈 태그 바 상시 노출을 없애 세로 공간 절약. */}
      {onAddTag && filePath && (tags.length > 0 || tagPanelOpen) && (
        <div className="px-3 py-1.5 border-b" style={{ borderColor: "var(--color-border)" }}>
          <TagInput
            tags={tags}
            suggestions={tagSuggestions}
            autoFocus={tagPanelOpen && tags.length === 0}
            onAdd={(tag) => onAddTag(filePath, tag)}
            onRemove={(tag) => onRemoveTag?.(filePath, tag)}
          />
        </div>
      )}

      {/* AI 섹션 (요약 + 파일 질문) — 스크롤 밖 고정 영역 */}
      {hasAiContent && (
        <AiSection summary={summary} showFileQa={showFileQa} filePath={filePath} markdownComponents={markdownComponents} />
      )}

      {/* 찾기 바 (Ctrl+F) — 문서 내 즉석 찾기, 패널 상단 고정 */}
      {findOpen && <FindBar find={find} />}

      {/* 본문 영역 — 레이아웃 뷰: HWPX=LayoutView(인라인 SVG·줌/팬·매치), PDF=PdfLayoutView
          (pdfium 페이지 이미지). 그 외는 마크다운 스크롤 영역 */}
      {!loading && !error && viewMode === "layout" && isPdf ? (
        <PdfLayoutView filePath={filePath} onExpand={() => setViewerOpen(true)} />
      ) : !loading && !error && viewMode === "layout" && layoutSvg ? (
        <LayoutView svg={layoutSvg} findTerm={findTermForView} onExpand={() => setViewerOpen(true)} />
      ) : (
        <div ref={contentRef} className="flex-1 overflow-y-auto overflow-x-hidden">
          {loading && <PreviewSkeleton />}

          {needsPassword && !loading && (
            <PasswordGate
              value={passwordInput}
              onChange={(v) => {
                setPasswordInput(v);
                if (passwordWrong) setPasswordWrong(false);
              }}
              onSubmit={() => void unlockWithPassword()}
              unlocking={unlocking}
              wrong={passwordWrong}
            />
          )}

          {error && (
            <div className="p-6 flex flex-col items-center gap-3 text-center" role="alert">
              <FileText size={22} className="opacity-40 text-[var(--color-text-muted)]" />
              <p className="text-sm text-[var(--color-text-primary)]">미리보기를 불러오지 못했어요</p>
              <p className="text-xs text-[var(--color-text-secondary)] max-w-xs break-words">{error}</p>
              <div className="flex items-center gap-2">
                <button
                  onClick={() => setReloadKey((k) => k + 1)}
                  className="inline-flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg border text-[var(--color-text-primary)] hover:bg-[var(--color-bg-tertiary)] transition-colors"
                  style={{ borderColor: "var(--color-border)" }}
                >
                  <RotateCw size={12} />다시 시도
                </button>
                {onOpenFile && (
                  <button
                    onClick={() => onOpenFile(filePath)}
                    className="inline-flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg border text-[var(--color-text-primary)] hover:bg-[var(--color-bg-tertiary)] transition-colors"
                    style={{ borderColor: "var(--color-border)" }}
                  >
                    <ExternalLink size={12} />원본 열기
                  </button>
                )}
              </div>
            </div>
          )}

          {!loading && !error && !needsPassword && markdown !== null && markdown.length === 0 && (
            <div className="p-6 flex flex-col items-center gap-2 text-center">
              <FileText size={24} className="opacity-30 text-[var(--color-text-muted)]" />
              <p className="text-sm text-[var(--color-text-secondary)]">보여 줄 텍스트가 없어요</p>
              <p className="text-xs text-[var(--color-text-muted)] max-w-xs">
                그림만 있는 문서나 스캔 문서일 수 있어요.
              </p>
              {onOcrReindex && !IS_LITE && OCR_ELIGIBLE_RE.test(filePath) && (
                <button
                  onClick={async () => {
                    await onOcrReindex(filePath);
                    setReloadKey((k) => k + 1);
                  }}
                  className="mt-1 inline-flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg border text-[var(--color-text-primary)] hover:bg-[var(--color-bg-tertiary)] transition-colors"
                  style={{ borderColor: "var(--color-border)" }}
                >
                  <ScanText size={12} />OCR로 다시 읽기
                </button>
              )}
            </div>
          )}

          {/* 마크다운 렌더링 */}
          {!loading && !error && markdown && viewMode === "markdown" && (
            <div ref={previewBodyRef} className="doc-preview px-6 py-5">{previewMarkdownNode}</div>
          )}
        </div>
      )}

      {/* 경로 + 글자수 */}
      <div
        className="flex items-center gap-2 px-3 py-1.5 border-t text-2xs text-[var(--color-text-muted)]"
        style={{ borderColor: "var(--color-border)" }}
      >
        <span className="truncate flex-1" title={cleanPath(filePath)}>{cleanPath(filePath)}</span>
        {markdown && <span className="shrink-0 tabular-nums">{markdown.length.toLocaleString()}자</span>}
      </div>

      {viewerOpen && (isPdf || !!layoutSvg) && (
        <LayoutViewerDialog
          filePath={filePath}
          isPdf={isPdf}
          layoutSvg={layoutSvg}
          findTerm={findTermForView}
          onClose={() => setViewerOpen(false)}
        />
      )}
    </div>
  );
});
