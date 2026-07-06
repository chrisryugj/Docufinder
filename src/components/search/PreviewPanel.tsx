import { memo, useEffect, useState, useRef, useCallback, useMemo, type ComponentProps } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { X, FileText, Copy, ExternalLink, FolderOpen, Bookmark, Sparkles, ChevronDown, ChevronUp, MessageSquare, ClipboardCopy, Save, Search, MoreHorizontal, Tag, AlertTriangle } from "lucide-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import rehypeKatex from "rehype-katex";
import rehypeRaw from "rehype-raw";
import rehypeSanitize, { defaultSchema } from "rehype-sanitize";
import "katex/dist/katex.min.css";
import { save } from "@tauri-apps/plugin-dialog";
import { FileIcon } from "../ui/FileIcon";
import { LayoutView } from "./LayoutView";
import { PdfLayoutView } from "./PdfLayoutView";
import { Badge, getFileTypeBadgeVariant } from "../ui/Badge";
import { Tooltip } from "../ui/Tooltip";
import { TagInput } from "../ui/TagInput";
import type { AiAnalysis } from "../../types/search";
import { extractLegalReferences } from "../../utils/legalReference";
import { cleanPath } from "../../utils/cleanPath";
import { MOD_KEY } from "../../utils/platform";
import { useUIContext } from "../../contexts/UIContext";

// ─── Types ─────────────────────────────────────────────

interface MarkdownPreviewResponse {
  file_path: string;
  file_name: string;
  markdown: string;
  /** 복사 시 글자가 깨지는 문서(CID/PUA 매핑 손상)로 감지됨 */
  garbled: boolean;
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
  isBookmarked?: boolean;
  tags?: string[];
  tagSuggestions?: string[];
  onAddTag?: (filePath: string, tag: string) => void;
  onRemoveTag?: (filePath: string, tag: string) => void;
}

// ─── 검색어 하이라이트 + 법령 참조 유틸 ────────────────

function highlightTextWithLegal(
  text: string,
  searchRegex: RegExp | null,
  onOpenUrl: (url: string) => void,
): React.ReactNode {
  const legalRefs = extractLegalReferences(text);

  if (legalRefs.length === 0 && !searchRegex) return text;

  const applySearchHighlight = (str: string, keyBase: string): React.ReactNode[] => {
    if (!searchRegex || !str) return [str];
    const parts = str.split(new RegExp(`(${searchRegex.source})`, "gi"));
    return parts.map((part, i) =>
      i % 2 === 1 ? (
        <mark key={`${keyBase}-h${i}`} className="hl-search">{part}</mark>
      ) : (
        <span key={`${keyBase}-t${i}`}>{part}</span>
      ),
    );
  };

  if (legalRefs.length === 0) {
    return <>{applySearchHighlight(text, "s")}</>;
  }

  const segments: React.ReactNode[] = [];
  let lastEnd = 0;

  for (let li = 0; li < legalRefs.length; li++) {
    const ref = legalRefs[li];
    if (ref.start > lastEnd) {
      segments.push(...applySearchHighlight(text.slice(lastEnd, ref.start), `pre-${li}`));
    }
    segments.push(
      <button
        key={`legal-${li}`}
        onClick={() => onOpenUrl(ref.url)}
        className="preview-inline-link inline underline decoration-dotted underline-offset-2 cursor-pointer"
        style={{ color: "var(--color-accent)" }}
        title={`${ref.lawName ? ref.lawName + " " : ""}${ref.article || ref.text} — 법제처에서 열기`}
      >
        {ref.text}
      </button>,
    );
    lastEnd = ref.end;
  }

  if (lastEnd < text.length) {
    segments.push(...applySearchHighlight(text.slice(lastEnd), "post"));
  }

  return <>{segments}</>;
}

// ─── 찾기 바(Ctrl+F) 하이라이트 — CSS Custom Highlight API (T3-6) ──
// 이전에는 찾기 정규식이 markdown components 를 갈아끼워 확정 검색어마다
// remark/katex 전체 재파싱을 유발했다. 이제 커밋된 DOM 텍스트 노드를
// TreeWalker 로 순회해 Range 를 만들고 CSS.highlights 에 등록만 한다 —
// DOM 을 바꾸지 않으므로 React 재조정과 충돌하지 않고, 재파싱도 없다.

const FIND_HIGHLIGHT = "docufinder-find";
const FIND_ACTIVE_HIGHLIGHT = "docufinder-find-active";

/** CSS Custom Highlight API 지원 여부 (WebView2/Safari 17.2+ — 미지원 시 카운트/이동만 동작) */
const cssHighlightsSupported = (): boolean =>
  typeof CSS !== "undefined" && "highlights" in CSS;

/** 본문 텍스트 노드를 순회하며 찾기 매치 Range 수집 (문서 순서 보장) */
function collectFindRanges(root: HTMLElement, regex: RegExp): Range[] {
  const ranges: Range[] = [];
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
    // KaTeX 는 시각용 HTML 과 보조기기용 트리에 같은 텍스트가 중복되어
    // 이중 카운트를 유발하므로 제외
    acceptNode: (node) =>
      node.parentElement?.closest(".katex")
        ? NodeFilter.FILTER_REJECT
        : NodeFilter.FILTER_ACCEPT,
  });
  for (let node = walker.nextNode(); node; node = walker.nextNode()) {
    const text = node.nodeValue;
    if (!text) continue;
    regex.lastIndex = 0;
    let m: RegExpExecArray | null;
    while ((m = regex.exec(text)) !== null) {
      if (m[0].length === 0) break; // 빈 매치 무한루프 방어
      const r = document.createRange();
      r.setStart(node, m.index);
      r.setEnd(node, m.index + m[0].length);
      ranges.push(r);
    }
  }
  return ranges;
}

// ─── 인용 점프: 앵커 텍스트 → 미리보기 DOM 위치 탐색 ──

const JUMP_BLOCK_SELECTOR = "p, li, td, th, h1, h2, h3, h4, h5, h6, blockquote";

/** NFC 정규화 + 공백/비가시 문자 + 마크다운 마커 제거 — 앵커(DB 청크, 백엔드에서
 *  normalize_text 적용됨)와 미리보기(kordoc 재파싱, 비정규화)의 차이를 흡수한다.
 *  `\s` 가 NBSP·ideographic space·BOM 을 이미 포함하므로, 그 밖의 zero-width
 *  (U+200B/C/D)·soft-hyphen(U+00AD) 만 추가 제거하면 백엔드 normalize_text 와 정합.
 *  앵커와 DOM textContent 양쪽 모두 이 함수를 거치므로 어느 쪽에 차이가 있든 정렬된다. */
const normForMatch = (s: string): string =>
  s.normalize("NFC").replace(/[\s\u200B\u200C\u200D\u00AD*#`|_~[\]]/g, "");

/**
 * 백엔드 앵커 텍스트(청크 앞부분)를 렌더된 미리보기에서 찾아 해당 블록 요소를 반환.
 * offset 좌표가 아닌 텍스트 검색인 이유: kordoc 경로에서 page/offset이 비거나
 * 좌표 단위가 혼재해 신뢰 불가하기 때문. 못 찾으면 페이지 헤딩으로 폴백.
 */
function findJumpTarget(
  root: HTMLElement,
  anchors: string[],
  page: number | null,
): HTMLElement | null {
  const blocks = Array.from(root.querySelectorAll<HTMLElement>(JUMP_BLOCK_SELECTOR));

  // 1) 앵커 텍스트 매칭 (검색 score 순 → 가장 관련 높은 청크 우선)
  for (const anchor of anchors) {
    const needle = normForMatch(anchor).slice(0, 20);
    if (needle.length < 6) continue;
    for (const el of blocks) {
      if (normForMatch(el.textContent || "").includes(needle)) return el;
    }
  }

  // 2) 페이지 헤딩 폴백 — DB 폴백 경로가 주입하는 "## N페이지" 헤딩
  if (page != null) {
    const tag = normForMatch(`${page}페이지`);
    for (const el of blocks) {
      if (/^h[1-6]$/i.test(el.tagName) && normForMatch(el.textContent || "").includes(tag)) {
        return el;
      }
    }
  }

  return null;
}

// ─── 미리보기 HTML 살균 스키마 (kordoc HTML 표 네이티브 렌더용) ──
//
// kordoc은 병합·중첩 표를 <table><td colspan rowspan> HTML로 방출한다(단순 표는 GFM).
// 과거엔 이 HTML을 정규식으로 GFM 파이프표에 눌러 담았으나, GFM은 colspan/rowspan·중첩표를
// 표현하지 못하고 non-greedy 정규식이 중첩표에서 바깥 표 닫힘을 놓쳐 결재문서(실측 194건 중
// 43%가 중첩표)의 도장란·문서번호표가 뭉개졌다. 이제 rehype-raw로 HTML 표를 그대로 렌더해
// 브라우저가 병합·중첩을 네이티브 처리하게 하고(rhwp HTML 백엔드·kordoc renderHtml과 동일 접근),
// rehype-sanitize로 XSS를 차단한다. 표 구조 태그 + 병합 속성 + KaTeX placeholder 통과용
// className만 기본 스키마에 추가 허용한다. (sanitize는 katex 앞에 두어 수식 출력은 통과.)
//
// className은 code 요소의 화이트리스트 값(language-*·math-inline·math-display, remark-math
// 공식 sanitize 레시피)으로만 허용한다. 전역("*") className 허용은 신뢰 불가 문서가 번들
// Tailwind 클래스(.fixed .inset-0 .z-50 .bg-white …)로 전창 오버레이 디페이스/피싱 UI를
// 구성할 수 있어 금지 — KaTeX 는 code.math-inline/math-display 만 보므로 수식 렌더는 유지.
const PREVIEW_SANITIZE_SCHEMA = {
  ...defaultSchema,
  tagNames: [
    ...(defaultSchema.tagNames ?? []),
    "table", "thead", "tbody", "tfoot", "tr", "td", "th", "col", "colgroup", "br",
  ],
  attributes: {
    ...defaultSchema.attributes,
    td: [...(defaultSchema.attributes?.td ?? []), "colSpan", "rowSpan", "colspan", "rowspan", "align"],
    th: [...(defaultSchema.attributes?.th ?? []), "colSpan", "rowSpan", "colspan", "rowspan", "align", "scope"],
    col: ["span", "width"],
    code: [["className", /^language-./, "math-inline", "math-display"]],
  },
};

// ─── 마크다운 커스텀 컴포넌트 ──────────────────────────

/**
 * 문단 선두 불릿 문자로 위계 판정
 * - Level 1 (상위): ■ □ ▣ ▢ ◆ ◇ — 두껍고 크게
 * - Level 2 (중간): ● ○ ◉ ◎ ▸ ▹ — 보통
 * - Level 3 (하위): - * · • ◦ — 작고 흐리게
 */
function detectBulletLevel(text: string): 1 | 2 | 3 | null {
  const trimmed = text.trimStart();
  const first = trimmed.charAt(0);
  if (!first) return null;
  if (/[■□▣▢◆◇]/.test(first)) return 1;
  if (/[●○◉◎▸▹]/.test(first)) return 2;
  if (/[\-*·•◦]/.test(first)) return 3;
  return null;
}

/** React children에서 첫 문자열 추출 (불릿 감지용, React 엘리먼트 재귀 파고듦) */
function firstTextOf(children: React.ReactNode): string {
  if (children == null || typeof children === "boolean") return "";
  if (typeof children === "string") return children;
  if (typeof children === "number") return String(children);
  if (Array.isArray(children)) {
    for (const c of children) {
      const t = firstTextOf(c);
      if (t) return t;
    }
    return "";
  }
  // React 엘리먼트 — props.children 재귀 (kordoc가 전체 문단을 **bold**로 감싼 경우 대응)
  if (typeof children === "object" && "props" in (children as object)) {
    const el = children as { props?: { children?: React.ReactNode } };
    if (el.props?.children !== undefined) {
      return firstTextOf(el.props.children);
    }
  }
  return "";
}

function createMarkdownComponents(
  searchRegex: RegExp | null,
  onOpenUrl: (url: string) => void,
): ComponentProps<typeof ReactMarkdown>["components"] {
  // 텍스트 노드에 하이라이트 적용하는 래퍼 (찾기 하이라이트는 렌더 후
  // CSS Custom Highlight 로 별도 적용 — 여기서는 검색어/법령만)
  const TextWrapper = ({ children }: { children: React.ReactNode }) => {
    if (typeof children === "string") {
      return <>{highlightTextWithLegal(children, searchRegex, onOpenUrl)}</>;
    }
    return <>{children}</>;
  };

  return {
    // 텍스트가 포함된 블록 요소에 하이라이트 적용
    p: ({ children }) => {
      const level = detectBulletLevel(firstTextOf(children));
      const bulletClass = level ? ` doc-bullet-${level}` : "";
      return (
        <p className={`doc-paragraph${bulletClass}`}>
          {Array.isArray(children)
            ? children.map((child, i) => <TextWrapper key={i}>{child}</TextWrapper>)
            : <TextWrapper>{children}</TextWrapper>}
        </p>
      );
    },
    // 헤딩
    h1: ({ children }) => <h1 className="doc-h1"><TextWrapper>{children}</TextWrapper></h1>,
    h2: ({ children }) => <h2 className="doc-h2"><TextWrapper>{children}</TextWrapper></h2>,
    h3: ({ children }) => <h3 className="doc-h3"><TextWrapper>{children}</TextWrapper></h3>,
    h4: ({ children }) => <h4 className="doc-h4"><TextWrapper>{children}</TextWrapper></h4>,
    h5: ({ children }) => <h5 className="doc-h5">{children}</h5>,
    h6: ({ children }) => <h6 className="doc-h6">{children}</h6>,
    // 테이블
    table: ({ children }) => (
      <div className="doc-table-wrapper">
        <table className="doc-table">{children}</table>
      </div>
    ),
    thead: ({ children }) => <thead className="doc-thead">{children}</thead>,
    // node 제외 후 나머지(colSpan/rowSpan/align 등 병합 속성) 전달 — HTML 표 병합셀 보존
    th: ({ children, node: _node, ...rest }) => <th className="doc-th" {...rest}><TextWrapper>{children}</TextWrapper></th>,
    td: ({ children, node: _node, ...rest }) => <td className="doc-td" {...rest}><TextWrapper>{children}</TextWrapper></td>,
    // 링크: 외부 브라우저로 열기
    a: ({ href, children }) => (
      <button
        onClick={() => href && onOpenUrl(href)}
        className="preview-inline-link inline underline decoration-dotted underline-offset-2 cursor-pointer"
        style={{ color: "var(--color-accent)" }}
        title={href}
      >
        {children}
      </button>
    ),
    // 리스트
    ul: ({ children }) => <ul className="doc-ul">{children}</ul>,
    ol: ({ children }) => <ol className="doc-ol">{children}</ol>,
    li: ({ children }) => <li className="doc-li"><TextWrapper>{children}</TextWrapper></li>,
    // 구분선
    hr: () => <hr className="doc-hr" />,
    // 인용문
    blockquote: ({ children }) => <blockquote className="doc-blockquote">{children}</blockquote>,
    // 강조
    strong: ({ children }) => <strong className="doc-strong">{children}</strong>,
    em: ({ children }) => <em className="doc-em">{children}</em>,
    del: ({ children }) => <del className="doc-del">{children}</del>,
  };
}

// ─── 상수 ─────────────────────────────────────────────

type SummaryType = "brief" | "structured" | "keywords";

const SUMMARY_TYPE_LABELS: Record<SummaryType, string> = {
  brief: "핵심 3줄",
  structured: "항목별 정리",
  keywords: "핵심 키워드",
};

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

// ─── FileQaSection (격리 컴포넌트 — 입력 시 부모 리렌더 방지) ──

interface FileQaSectionProps {
  filePath: string;
}

const FileQaSection = memo(function FileQaSection({ filePath }: FileQaSectionProps) {
  const [question, setQuestion] = useState("");
  const [answer, setAnswer] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [analysis, setAnalysis] = useState<AiAnalysis | null>(null);
  const unlistenRef = useRef<UnlistenFn[]>([]);
  const requestIdRef = useRef("");

  // Tauri 이벤트 리스너 (StrictMode 중복 방지: cancelled flag)
  useEffect(() => {
    let cancelled = false;

    const setup = async () => {
      const u1 = await listen<{ request_id: string; token: string }>("ai-file-token", (e) => {
        if (cancelled) return;
        if (e.payload.request_id !== requestIdRef.current) return;
        setAnswer((prev) => prev + e.payload.token);
      });
      const u2 = await listen<AiAnalysis & { request_id: string }>("ai-file-complete", (e) => {
        if (cancelled) return;
        if (e.payload.request_id !== requestIdRef.current) return;
        const { request_id: _, ...a } = e.payload;
        setAnalysis(a as AiAnalysis);
        setLoading(false);
      });
      const u3 = await listen<{ request_id: string; error: string }>("ai-file-error", (e) => {
        if (cancelled) return;
        if (e.payload.request_id !== requestIdRef.current) return;
        setError(e.payload.error);
        setLoading(false);
      });

      if (cancelled) {
        u1(); u2(); u3();
      } else {
        unlistenRef.current = [u1, u2, u3];
      }
    };
    setup();
    return () => {
      cancelled = true;
      unlistenRef.current.forEach((fn) => fn());
      unlistenRef.current = [];
    };
  }, []);

  // 파일 변경 시 초기화
  useEffect(() => {
    setAnswer("");
    setAnalysis(null);
    setError(null);
    setLoading(false);
    requestIdRef.current = "";
  }, [filePath]);

  const handleSubmit = useCallback(() => {
    if (!filePath || !question.trim() || loading) return;
    const rid = crypto.randomUUID();
    requestIdRef.current = rid;
    setAnswer("");
    setAnalysis(null);
    setError(null);
    setLoading(true);

    invoke("ask_ai_file", { filePath, query: question, requestId: rid }).catch((e) => {
      const msg = typeof e === "string" ? e : e?.message || "질문 처리 실패";
      setError(msg);
      setLoading(false);
    });
  }, [filePath, question, loading]);

  const hasAnswer = answer || loading;

  return (
    <div className="border-t" style={{ borderColor: "var(--color-border)" }}>
      {/* 질문 입력 */}
      <div
        className="flex items-center gap-2 px-3 py-2.5"
        style={{ backgroundColor: "var(--color-bg-secondary)" }}
      >
        <div
          className="w-5 h-5 rounded-full shrink-0 flex items-center justify-center"
          style={{ background: "linear-gradient(135deg, var(--color-accent-ai) 0%, var(--color-accent-ai-hover) 100%)" }}
        >
          <MessageSquare size={10} color="white" />
        </div>
        <input
          type="text"
          value={question}
          onChange={(e) => setQuestion(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.nativeEvent.isComposing) {
              e.preventDefault();
              handleSubmit();
            }
          }}
          placeholder="이 파일에 대해 질문하세요..."
          className="flex-1 bg-transparent border-none focus:outline-none text-xs"
          style={{ color: "var(--color-text-primary)" }}
        />
        {loading ? (
          <div
            className="w-4 h-4 border-2 rounded-full animate-spin shrink-0"
            style={{ borderColor: "var(--color-accent-ai)", borderTopColor: "transparent" }}
          />
        ) : (
          question.trim() && (
            <button
              onClick={handleSubmit}
              className="shrink-0 p-1.5 rounded-lg transition-all hover:scale-105 active:scale-95"
              style={{ backgroundColor: "var(--color-accent-ai)", color: "white" }}
              title="전송 (Enter)"
            >
              <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <path d="M22 2L11 13" /><path d="M22 2L15 22L11 13L2 9L22 2" />
              </svg>
            </button>
          )
        )}
      </div>

      {/* 에러 */}
      {error && (
        <div className="px-3 py-2 text-[11px] flex items-start gap-1.5" style={{ color: "var(--color-error)", backgroundColor: "color-mix(in srgb, var(--color-error) 6%, transparent)" }}>
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="shrink-0 mt-0.5"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
          {error}
        </div>
      )}

      {/* 답변 */}
      {hasAnswer && (
        <div className="px-3 py-3 max-h-60 overflow-y-auto" style={{ backgroundColor: "var(--color-bg-primary)" }}>
          {/* 답변 라벨 */}
          <div className="flex items-center gap-1.5 mb-2">
            <Sparkles size={10} style={{ color: "var(--color-accent-ai)" }} />
            <span className="text-[10px] font-semibold uppercase tracking-wider" style={{ color: "var(--color-accent-ai)" }}>
              답변
            </span>
            {loading && (
              <span className="text-[10px] animate-pulse" style={{ color: "var(--color-accent-ai)" }}>분석 중...</span>
            )}
            {analysis && (
              <span className="text-[10px] text-[var(--color-text-muted)] ml-auto tabular-nums">
                {(analysis.processing_time_ms / 1000).toFixed(1)}초
                {analysis.tokens_used && ` · ${analysis.tokens_used.total_tokens.toLocaleString()} tok`}
              </span>
            )}
          </div>

          {/* 답변 본문 */}
          {loading ? (
            <div className="text-[12.5px] leading-[1.8] text-[var(--color-text-primary)] whitespace-pre-wrap break-words">
              {answer || <span className="text-[var(--color-text-muted)]">문서를 분석하고 있습니다...</span>}
              {answer && (
                <span
                  className="inline-block w-1.5 h-3.5 rounded-sm animate-pulse ml-0.5 align-text-bottom"
                  style={{ backgroundColor: "var(--color-accent-ai)" }}
                />
              )}
            </div>
          ) : (
            <div className="text-[12.5px] leading-[1.8] text-[var(--color-text-primary)] doc-preview summary-inline ai-answer-prose">
              <ReactMarkdown
                remarkPlugins={[[remarkGfm, { singleTilde: false }], remarkMath]}
                rehypePlugins={[rehypeKatex]}
              >
                {answer}
              </ReactMarkdown>
            </div>
          )}

          {/* 메타 / 초기화 */}
          {analysis && (
            <div className="mt-3 pt-2 border-t flex items-center" style={{ borderColor: "var(--color-border)" }}>
              <button
                onClick={() => { setAnswer(""); setAnalysis(null); setError(null); setQuestion(""); }}
                className="text-[10px] px-2 py-0.5 rounded-lg transition-colors hover:bg-[var(--color-bg-tertiary)]"
                style={{ color: "var(--color-text-muted)" }}
              >
                새 질문
              </button>
            </div>
          )}
        </div>
      )}
    </div>
  );
});

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
  isBookmarked,
  tags = [],
  tagSuggestions = [],
  onAddTag,
  onRemoveTag,
}: PreviewPanelProps) {
  const [markdown, setMarkdown] = useState<string | null>(null);
  // 복사 시 글자 깨짐 표식 — 최종 반환 markdown 기준 판정값 (백엔드 계산)
  const [garbled, setGarbled] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  // 찾기(Ctrl+F) 대상은 문서 본문 한정 — AI 요약·질문답변 영역 제외용 전용 ref
  const previewBodyRef = useRef<HTMLDivElement>(null);
  const findRangesRef = useRef<Range[]>([]);

  // AI 요약 상태
  const [aiSummary, setAiSummary] = useState<AiAnalysis | null>(null);
  const [summaryLoading, setSummaryLoading] = useState(false);
  const [summaryError, setSummaryError] = useState<string | null>(null);
  const [summaryExpanded, setSummaryExpanded] = useState(true);
  const [summaryType, setSummaryType] = useState<SummaryType>("brief");
  const [showSummaryMenu, setShowSummaryMenu] = useState(false);
  const summaryRequestId = useRef(0);

  // 파일 질문 토글
  const [showFileQa, setShowFileQa] = useState(false);

  // 더보기(⋯) 메뉴 토글 — 파일 위치·복사/내보내기·태그 추가를 담는 오버플로 팝오버
  const [showMoreMenu, setShowMoreMenu] = useState(false);
  const moreMenuRef = useRef<HTMLDivElement>(null);
  // 태그 패널 — 태그가 있거나 ⋯>태그 추가로 열었을 때만 노출 (빈 태그 바 상시 노출 제거)
  const [tagPanelOpen, setTagPanelOpen] = useState(false);

  // 찾기 바 (Ctrl+F) — 문서 내 즉석 찾기
  const panelRef = useRef<HTMLDivElement>(null);
  const findInputRef = useRef<HTMLInputElement>(null);
  const findComposingRef = useRef(false); // IME 조합 중 여부 (compositionend 후 검색)
  const [findOpen, setFindOpen] = useState(false);
  const [findInput, setFindInput] = useState("");
  const [findTerm, setFindTerm] = useState(""); // 디바운스 + 조합 완료 후 확정값
  const [findCount, setFindCount] = useState(0);
  const [findActiveIdx, setFindActiveIdx] = useState(0);

  // 레이아웃 뷰 (kordoc render SVG) — HWPX 전용, 파일당 1회 렌더 후 캐시.
  // kordoc 은 조판 캐시 기반 첫 페이지만 렌더한다 — UI 라벨도 "첫 페이지"로 정직하게.
  const [viewMode, setViewMode] = useState<PreviewView>("markdown");
  const [layoutSvg, setLayoutSvg] = useState<string | null>(null);
  const [layoutLoading, setLayoutLoading] = useState(false);
  const [viewerOpen, setViewerOpen] = useState(false); // 문서 크게 보기(팝업) 오버레이
  const viewerRef = useRef<HTMLDivElement>(null); // 팝업 다이얼로그 — 포커스 트랩 대상
  const viewerReturnFocusRef = useRef<HTMLElement | null>(null); // 닫힐 때 포커스 복원 대상
  const layoutReqRef = useRef(0); // 파일 전환 시 in-flight 렌더 응답 무효화
  const prefAppliedRef = useRef<string | null>(null); // 선호 뷰 자동 진입: 파일당 1회
  const desiredViewRef = useRef<PreviewView>("markdown"); // 의도한 뷰 — in-flight 렌더가 사용자 전환/점프를 덮지 않게

  const { showToast, updateToast } = useUIContext();

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
    const baseName = filePath.replace(/^\\\\\?\\/, "").split(/[\\/]/).pop() || "preview";
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
    const toastId = showToast("Markdown 저장 중...", "loading");
    try {
      await invoke("export_markdown", { content: markdown, outputPath });
      updateToast(toastId, { message: "Markdown 파일로 저장했습니다", type: "success" });
    } catch (e) {
      const msg = typeof e === "string" ? e : ((e as { message?: string })?.message ?? "저장 실패");
      updateToast(toastId, { message: `저장 실패: ${msg}`, type: "error" });
    }
  }, [markdown, filePath, showToast, updateToast]);

  // 파일 변경 시 AI 상태 초기화
  useEffect(() => {
    if (!filePath) {
      setMarkdown(null);
      setGarbled(false);
      return;
    }
    summaryRequestId.current++;
    setAiSummary(null);
    setSummaryError(null);
    setShowSummaryMenu(false);
    setShowFileQa(false);
    setShowMoreMenu(false);
    setTagPanelOpen(false);
    setFindOpen(false);
    setFindInput("");
    setFindTerm("");
    setFindActiveIdx(0);
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

    // 빠른 탐색 시 불필요한 파싱 방지를 위해 300ms debounce (화살표 키 고속 이동 대응)
    const timer = setTimeout(() => {
      invoke<MarkdownPreviewResponse>("load_markdown_preview", { filePath })
        .then((res) => {
          if (!cancelled) {
            setMarkdown(res.markdown);
            setGarbled(res.garbled ?? false);
            setLoading(false);
            contentRef.current?.scrollTo(0, 0);
          }
        })
        .catch((e) => {
          if (!cancelled) {
            setError(typeof e === "string" ? e : e?.message || "미리보기 로드 실패");
            setLoading(false);
          }
        });
    }, 300);

    return () => { cancelled = true; clearTimeout(timer); };
  }, [filePath]);

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

  // AI 요약 생성
  const handleGenerateSummary = useCallback((type: SummaryType) => {
    if (!filePath || summaryLoading) return;
    const reqId = ++summaryRequestId.current;
    setSummaryLoading(true);
    setSummaryError(null);
    setAiSummary(null);
    setSummaryType(type);

    invoke<AiAnalysis>("summarize_ai", { filePath, summaryType: type })
      .then((res) => {
        if (summaryRequestId.current === reqId) {
          setAiSummary(res);
          setSummaryExpanded(true);
        }
      })
      .catch((e) => {
        if (summaryRequestId.current === reqId) {
          const msg = typeof e === "string" ? e : e?.message || "AI 요약 실패";
          setSummaryError(msg);
        }
      })
      .finally(() => {
        if (summaryRequestId.current === reqId) setSummaryLoading(false);
      });
  }, [filePath, summaryLoading]);

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

  // 찾기 바 정규식 (확정 검색어 기준, 바 닫힘 시 비활성)
  const findRegex = useMemo(() => {
    const term = findTerm.trim();
    if (!term) return null;
    return new RegExp(term.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), "gi");
  }, [findTerm]);
  const activeFindRegex = findOpen ? findRegex : null;

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
  // rehypePlugins 순서: raw(원시 HTML 표 파싱) → sanitize(XSS 차단) → katex(수식). katex 출력은
  // sanitize 뒤라 그대로 통과한다. kordoc의 병합·중첩 HTML 표는 여기서 네이티브로 렌더된다.
  const previewMarkdownNode = useMemo(() => {
    if (!markdown) return null;
    return (
      <ReactMarkdown
        remarkPlugins={[[remarkGfm, { singleTilde: false }], remarkMath]}
        rehypePlugins={[rehypeRaw, [rehypeSanitize, PREVIEW_SANITIZE_SCHEMA], rehypeKatex]}
        components={markdownComponents}
      >
        {markdown}
      </ReactMarkdown>
    );
  }, [markdown, markdownComponents]);

  // ── 찾기 바 동작 ──

  const closeFind = useCallback(() => {
    setFindOpen(false);
    setFindActiveIdx(0);
  }, []);

  // 찾기 토글 — 레이아웃 뷰에서도 인라인 SVG <text> 매치를 LayoutView 가 처리하므로
  // 뷰 전환 없이 찾기 바만 토글한다 (버튼·Ctrl+F 공용).
  const handleFindToggle = useCallback(() => {
    setFindOpen((v) => !v);
  }, []);

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
          const msg = typeof e === "string" ? e : ((e as { message?: string })?.message ?? "레이아웃 렌더 실패");
          showToast(`레이아웃 렌더 실패: ${msg}`, "error");
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

  const handleFindNav = useCallback((dir: 1 | -1) => {
    setFindActiveIdx((prev) => (findCount > 0 ? (prev + dir + findCount) % findCount : 0));
  }, [findCount]);

  const handleFindInputKeyDown = useCallback((e: React.KeyboardEvent<HTMLInputElement>) => {
    // 전역 단축키와 격리 — Escape(선택 해제→프리뷰 닫힘), ↑/↓(결과 이동) 누출 방지
    e.stopPropagation();
    if (e.key === "Escape") {
      e.preventDefault();
      closeFind();
    } else if (e.key === "Enter") {
      if (e.nativeEvent.isComposing) return;
      e.preventDefault();
      handleFindNav(e.shiftKey ? -1 : 1);
    } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "f") {
      e.preventDefault();
      e.currentTarget.select();
    }
  }, [closeFind, handleFindNav]);

  // 찾기 바 열린 상태에서 패널 내 어디서든 Esc → 찾기 바만 닫기 (프리뷰 닫힘 차단)
  const handlePanelKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === "Escape" && findOpen) {
      e.preventDefault();
      e.stopPropagation();
      closeFind();
    }
  }, [findOpen, closeFind]);

  // 입력 디바운스 → 확정 검색어 (IME 조합 중에는 compositionend 핸들러가 확정)
  useEffect(() => {
    if (findComposingRef.current) return;
    const timer = setTimeout(() => setFindTerm(findInput), 150);
    return () => clearTimeout(timer);
  }, [findInput]);

  // Ctrl/Cmd+F 토글 — 미리보기가 열려 있으면 포커스 위치와 무관하게 동작.
  // (이전엔 패널 포커스/호버를 요구해 결과 리스트에 마우스를 둔 채 누르면
  //  무반응이라 "안 먹는" 것으로 보였다. 앱 내 다른 Ctrl+F 소비자가 없고
  //  Tauri WebView 에는 브라우저 기본 찾기도 없어 충돌 대상이 없다.)
  useEffect(() => {
    if (!filePath) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey) || e.shiftKey || e.altKey) return;
      if (e.key.toLowerCase() !== "f" && e.code !== "KeyF") return;
      // 다이얼로그(크게 보기 팝업 등)가 열려 있으면 무시 — 오버레이 뒤 안 보이는 찾기 바를
      // 열어 포커스를 모달 밖으로 빼가는 것을 막는다(bare-key 뷰 전환 가드와 동일 관례).
      if (document.querySelector("[role='dialog']")) return;
      e.preventDefault();
      handleFindToggle();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [filePath, handleFindToggle]);

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

  // 크게 보기(팝업) 다이얼로그 접근성 — 초기 포커스 + Tab 트랩 + 포커스 복원.
  // (Escape 닫기는 LayoutView 가 capture 단계에서 이미 처리하므로 여기선 Tab 만.)
  // ui/Modal 의 포커스 트랩과 동일 규약: disabled/숨김 요소 제외, 리스너는 document 에
  // 걸어 포커스가 모달 밖으로 새더라도 되돌린다. dialog 노드는 `viewerOpen && layoutSvg`
  // 로 조건부 렌더되므로 layoutSvg 재렌더로 노드가 교체되면 재설치되도록 deps 에 포함.
  useEffect(() => {
    if (!viewerOpen) return;
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
    viewerReturnFocusRef.current = document.activeElement as HTMLElement | null;
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
      const prev = viewerReturnFocusRef.current;
      if (prev?.isConnected) prev.focus();
    };
  }, [viewerOpen, layoutSvg]);

  // 열릴 때 입력 포커스 (기존 검색어 유지 시 전체 선택)
  useEffect(() => {
    if (findOpen) {
      findInputRef.current?.focus();
      findInputRef.current?.select();
    }
  }, [findOpen]);

  // 매치 수집 + 하이라이트 등록 — 커밋된 본문 DOM 에 Range 만 등록, 재파싱 없음 (T3-6).
  // previewMarkdownNode 의존: 본문 서브트리가 재커밋되면(내용·검색어 변경) 이전
  // Range 가 분리된 노드를 가리키므로 새 DOM 에서 재수집해야 한다.
  useEffect(() => {
    const body = previewBodyRef.current;
    findRangesRef.current = [];
    if (cssHighlightsSupported()) {
      CSS.highlights.delete(FIND_HIGHLIGHT);
      CSS.highlights.delete(FIND_ACTIVE_HIGHLIGHT);
    }
    if (!activeFindRegex || loading || !markdown || !body) {
      setFindCount(0);
      setFindActiveIdx(0);
      return;
    }
    const ranges = collectFindRanges(body, activeFindRegex);
    findRangesRef.current = ranges;
    if (cssHighlightsSupported() && ranges.length > 0) {
      CSS.highlights.set(FIND_HIGHLIGHT, new Highlight(...ranges));
    }
    setFindCount(ranges.length);
    setFindActiveIdx(0);
    return () => {
      // 패널 언마운트 시 전역 레지스트리 잔류 방지
      if (cssHighlightsSupported()) {
        CSS.highlights.delete(FIND_HIGHLIGHT);
        CSS.highlights.delete(FIND_ACTIVE_HIGHLIGHT);
      }
    };
  }, [activeFindRegex, markdown, loading, previewMarkdownNode]);

  // 활성 매치 강조 + 스크롤 — Highlight 레지스트리 엔트리만 교체 (재렌더 없음)
  useEffect(() => {
    const active = findRangesRef.current[findActiveIdx];
    if (!active) {
      if (cssHighlightsSupported()) CSS.highlights.delete(FIND_ACTIVE_HIGHLIGHT);
      return;
    }
    if (cssHighlightsSupported()) {
      CSS.highlights.set(FIND_ACTIVE_HIGHLIGHT, new Highlight(active));
    }
    // Range 자체는 scrollIntoView 가 없어 매치가 속한 요소 기준으로 스크롤
    active.startContainer.parentElement?.scrollIntoView({ block: "center" });
  }, [findActiveIdx, findCount, activeFindRegex, previewMarkdownNode]);

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
  const hasAiContent = aiSummary || summaryError || summaryLoading || showFileQa;
  // 원본 레이아웃 세그먼트: HWPX=kordoc render SVG, HWP=rhwp 네이티브 SVG (둘 다 LayoutView 소비)
  const isHwpx = ext === "hwpx";
  const isHwp = ext === "hwp";
  // PDF 는 pdfium 페이지 이미지로 원본 레이아웃을 본다 (SVG 경로와 별개, PdfLayoutView)
  const isPdf = ext === "pdf";

  return (
    <div ref={panelRef} onKeyDown={handlePanelKeyDown} className="preview-panel flex flex-col h-full border-l bg-[var(--color-bg-primary)]" style={{ borderColor: "var(--color-border)", minWidth: "320px" }}>
      {/* 헤더 */}
      <div className="flex items-center gap-2 px-3 py-2 border-b bg-[var(--color-bg-secondary)]" style={{ borderColor: "var(--color-border)" }}>
        <FileIcon fileName={fileName} size="sm" />
        <span className="flex-1 text-sm font-medium truncate text-[var(--color-text-primary)]" title={fileName}>
          {fileName}
        </span>
        {garbled && (
          <Tooltip content="복사 시 글자가 깨질 수 있는 문서" position="bottom" delay={200}>
            <Badge variant="warning" aria-label="복사 시 글자가 깨질 수 있는 문서">
              <AlertTriangle className="w-3 h-3" />
            </Badge>
          </Tooltip>
        )}
        <Badge variant={getFileTypeBadgeVariant(fileName)}>
          {ext.toUpperCase()}
        </Badge>
        <button onClick={onClose} className="p-1 rounded-lg hover:bg-[var(--color-bg-tertiary)] text-[var(--color-text-muted)] transition-colors" title="닫기" aria-label="닫기">
          <X size={14} />
        </button>
      </div>

      {/* 통합 툴바 — 뷰 세그먼트(좌) + 액션(우) 한 줄. 세그먼트를 별도 바에서 승격,
          액션바와 병합해 세로 크롬 축소. 파일위치·복사/내보내기·태그추가는 ⋯ 로 접어 정돈. */}
      <div className="flex items-center gap-1 px-2 py-1 border-b" style={{ borderColor: "var(--color-border)" }}>
        {/* 뷰 세그먼트 — 문서 텍스트 ↔ 원본 레이아웃 (HWPX SVG · PDF 페이지 이미지). 단축키 1·2. */}
        {(isHwpx || isPdf || isHwp) && markdown !== null && !loading && !error && (
          <div role="radiogroup" aria-label="미리보기 뷰" className="inline-flex gap-0.5 p-0.5 rounded-lg shrink-0" style={{ backgroundColor: "var(--color-bg-tertiary)" }}>
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
                  className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md text-[11px] font-medium transition-colors"
                  style={active
                    ? { backgroundColor: "var(--color-bg-secondary)", color: "var(--color-accent)", boxShadow: "var(--shadow-sm)" }
                    : { color: "var(--color-text-muted)" }}
                  title={`${label}  ·  단축키 ${key}`}
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
          <button onClick={() => onOpenFile?.(filePath)} className="p-1.5 rounded-lg hover:bg-[var(--color-bg-tertiary)] text-[var(--color-text-secondary)] transition-colors" title="파일 열기">
            <ExternalLink size={14} />
          </button>
          {markdown && (
            <>
              <button
                onClick={handleFindToggle}
                className={`p-1.5 rounded-lg transition-colors ${findOpen ? "text-[var(--color-accent)] bg-[var(--color-accent-light)]" : "text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-tertiary)]"}`}
                title={`문서 내 찾기 (${MOD_KEY}+F)`}
                aria-label="문서 내 찾기"
              >
                <Search size={14} />
              </button>
              <button
                onClick={() => setShowSummaryMenu((v) => !v)}
                disabled={summaryLoading}
                className="p-1.5 rounded-lg hover:bg-[var(--color-bg-tertiary)] text-[var(--color-text-secondary)] transition-colors disabled:opacity-50"
                title="AI 요약"
              >
                {summaryLoading
                  ? <div className="w-3.5 h-3.5 border border-[var(--color-accent)] border-t-transparent rounded-full animate-spin" />
                  : <Sparkles size={14} />
                }
              </button>
              <button
                onClick={() => setShowFileQa((v) => !v)}
                className={`p-1.5 rounded-lg transition-colors ${showFileQa ? "text-[var(--color-accent)] bg-[var(--color-accent-light)]" : "text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-tertiary)]"}`}
                title="이 파일에 대해 질문"
              >
                <MessageSquare size={14} />
              </button>
            </>
          )}
          {onBookmark && (
            <button
              onClick={() => onBookmark(filePath, markdown?.slice(0, 200) || "", null, null)}
              className={`p-1.5 rounded-lg transition-colors ${isBookmarked ? "text-[var(--color-accent)]" : "text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-tertiary)]"}`}
              title={isBookmarked ? "북마크 해제" : "북마크 추가"}
            >
              <Bookmark size={14} fill={isBookmarked ? "currentColor" : "none"} />
            </button>
          )}
          {/* ⋯ 더보기 — 파일 위치·복사/내보내기·태그 추가 (우측 정렬이라 메뉴는 right-0) */}
          <div className="relative inline-flex" ref={moreMenuRef}>
            <button
              onClick={() => setShowMoreMenu((v) => !v)}
              className={`p-1.5 rounded-lg transition-colors ${showMoreMenu ? "text-[var(--color-accent)] bg-[var(--color-accent-light)]" : "text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-tertiary)]"}`}
              title="더보기"
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
                <button onClick={() => { setShowMoreMenu(false); onOpenFolder?.(filePath); }} role="menuitem" className="export-dropdown-item" title="파일 위치 열기 (탐색기에서 선택)">
                  <FolderOpen size={13} />파일 위치 열기
                </button>
                {markdown && (
                  <>
                    <div className="my-1 border-t" style={{ borderColor: "var(--color-border)" }} />
                    <button onClick={handleCopyText} role="menuitem" className="export-dropdown-item" title="파싱된 텍스트를 클립보드에 복사">
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
      {showSummaryMenu && (
        <div className="flex items-center gap-1.5 px-3 py-2 border-b" style={{ borderColor: "var(--color-border)", backgroundColor: "var(--color-bg-secondary)" }}>
          <span className="text-[10px] text-[var(--color-text-muted)] shrink-0">요약 유형:</span>
          {(["brief", "structured", "keywords"] as SummaryType[]).map((type) => (
            <button
              key={type}
              onClick={() => { setShowSummaryMenu(false); handleGenerateSummary(type); }}
              className="px-2 py-0.5 rounded-lg text-[11px] transition-colors"
              style={{
                backgroundColor: summaryType === type && aiSummary ? "var(--color-accent-light)" : "var(--color-bg-tertiary)",
                color: summaryType === type && aiSummary ? "var(--color-accent)" : "var(--color-text-secondary)",
                border: "1px solid var(--color-border)",
              }}
            >
              {SUMMARY_TYPE_LABELS[type]}
            </button>
          ))}
          <button onClick={() => setShowSummaryMenu(false)} className="ml-auto text-[var(--color-text-muted)] hover:text-[var(--color-text-primary)] p-0.5 rounded-lg">
            <X size={12} />
          </button>
        </div>
      )}

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
        <div className="preview-ai-section border-b overflow-hidden ai-section-enter shrink-0" style={{ borderColor: "var(--color-accent-border)" }}>

          {/* 요약 로딩 */}
          {summaryLoading && (
            <div className="flex items-center gap-2 px-3 py-2.5 text-xs" style={{ color: "var(--color-accent)" }}>
              <div className="w-3 h-3 border border-current border-t-transparent rounded-full animate-spin shrink-0" />
              <span>"{SUMMARY_TYPE_LABELS[summaryType]}" 요약 생성 중...</span>
            </div>
          )}

          {/* 요약 에러 */}
          {summaryError && !summaryLoading && (
            <div className="px-3 py-2.5">
              <div className="flex items-center gap-1.5 text-xs mb-1" style={{ color: "var(--color-error)" }}>
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
                AI 요약 실패
              </div>
              <p className="text-[11px] text-[var(--color-text-secondary)]">{summaryError}</p>
              <button
                onClick={() => handleGenerateSummary(summaryType)}
                className="mt-1.5 text-[11px] text-[var(--color-accent)] hover:underline"
              >
                다시 시도
              </button>
            </div>
          )}

          {/* 요약 결과 */}
          {aiSummary && !summaryLoading && (
            <>
              <button
                onClick={() => setSummaryExpanded(!summaryExpanded)}
                className="flex items-center gap-2 w-full px-3 py-2 text-xs font-medium"
                style={{ color: "var(--color-accent)" }}
              >
                <Sparkles size={12} />
                AI 요약 — {SUMMARY_TYPE_LABELS[summaryType]}
                <span className="ml-auto text-[var(--color-text-muted)] font-normal">
                  {(aiSummary.processing_time_ms / 1000).toFixed(1)}초
                  {aiSummary.tokens_used && ` · ${aiSummary.tokens_used.total_tokens} tokens`}
                </span>
                {summaryExpanded ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
              </button>
              {summaryExpanded && (
                <div className="px-3 pb-3 text-[13px] leading-relaxed text-[var(--color-text-primary)] doc-preview summary-inline" style={{ backgroundColor: "var(--color-bg-primary)" }}>
                  <ReactMarkdown
                    remarkPlugins={[[remarkGfm, { singleTilde: false }], remarkMath]}
                    rehypePlugins={[rehypeKatex]}
                    components={markdownComponents}
                  >
                    {aiSummary.answer}
                  </ReactMarkdown>
                </div>
              )}
            </>
          )}

          {/* 파일 질문 섹션 (별도 컴포넌트 — 입력 시 부모 리렌더 방지) */}
          {showFileQa && <FileQaSection filePath={filePath} />}
        </div>
      )}

      {/* 찾기 바 (Ctrl+F) — 문서 내 즉석 찾기, 패널 상단 고정 */}
      {findOpen && (
        <div
          className="flex items-center gap-1.5 px-3 py-1.5 border-b shrink-0"
          style={{ borderColor: "var(--color-border)", backgroundColor: "var(--color-bg-secondary)" }}
        >
          <Search size={12} className="shrink-0 text-[var(--color-text-muted)]" />
          <input
            ref={findInputRef}
            type="text"
            value={findInput}
            onChange={(e) => setFindInput(e.target.value)}
            onCompositionStart={() => { findComposingRef.current = true; }}
            onCompositionEnd={(e) => {
              findComposingRef.current = false;
              setFindInput(e.currentTarget.value);
              setFindTerm(e.currentTarget.value);
            }}
            onKeyDown={handleFindInputKeyDown}
            placeholder="문서 내 찾기..."
            className="flex-1 min-w-0 bg-transparent border-none focus:outline-none text-xs"
            style={{ color: "var(--color-text-primary)" }}
            aria-label="문서 내 찾기"
          />
          {findTerm.trim() && (
            <span
              className="text-[10px] tabular-nums shrink-0"
              aria-live="polite"
              style={{ color: findCount === 0 ? "var(--color-error)" : "var(--color-text-muted)" }}
            >
              {findCount === 0 ? "0/0" : `${findActiveIdx + 1}/${findCount}`}
            </span>
          )}
          <button
            onClick={() => handleFindNav(-1)}
            disabled={findCount === 0}
            className="p-1 rounded-lg hover:bg-[var(--color-bg-tertiary)] text-[var(--color-text-secondary)] disabled:opacity-40 transition-colors"
            title="이전 매치 (Shift+Enter)"
            aria-label="이전 매치"
          >
            <ChevronUp size={12} />
          </button>
          <button
            onClick={() => handleFindNav(1)}
            disabled={findCount === 0}
            className="p-1 rounded-lg hover:bg-[var(--color-bg-tertiary)] text-[var(--color-text-secondary)] disabled:opacity-40 transition-colors"
            title="다음 매치 (Enter)"
            aria-label="다음 매치"
          >
            <ChevronDown size={12} />
          </button>
          <button
            onClick={closeFind}
            className="p-1 rounded-lg hover:bg-[var(--color-bg-tertiary)] text-[var(--color-text-muted)] transition-colors"
            title="찾기 닫기 (Esc)"
            aria-label="찾기 닫기"
          >
            <X size={12} />
          </button>
        </div>
      )}

      {/* 본문 영역 — 레이아웃 뷰: HWPX=LayoutView(인라인 SVG·줌/팬·매치), PDF=PdfLayoutView
          (pdfium 페이지 이미지). 그 외는 마크다운 스크롤 영역 */}
      {!loading && !error && viewMode === "layout" && isPdf ? (
        <PdfLayoutView filePath={filePath} onExpand={() => setViewerOpen(true)} />
      ) : !loading && !error && viewMode === "layout" && layoutSvg ? (
        <LayoutView svg={layoutSvg} findTerm={findOpen ? findTerm.trim() || undefined : undefined} onExpand={() => setViewerOpen(true)} />
      ) : (
        <div ref={contentRef} className="flex-1 overflow-y-auto overflow-x-hidden">
          {loading && (
            <div className="flex items-center justify-center h-32">
              <div className="w-5 h-5 border-2 border-[var(--color-accent)] border-t-transparent rounded-full animate-spin" />
            </div>
          )}

          {error && (
            <div className="p-4 text-sm text-[var(--color-error)]">
              <FileText size={20} className="mx-auto mb-2 opacity-50" />
              <p className="text-center">{error}</p>
            </div>
          )}

          {!loading && !error && markdown !== null && markdown.length === 0 && (
            <div className="p-4 text-sm text-center text-[var(--color-text-muted)]">
              <FileText size={24} className="mx-auto mb-2 opacity-30" />
              인덱싱된 텍스트가 없습니다
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
        className="flex items-center gap-2 px-3 py-1.5 border-t text-[10px] text-[var(--color-text-muted)]"
        style={{ borderColor: "var(--color-border)" }}
      >
        <span className="truncate flex-1" title={cleanPath(filePath)}>{cleanPath(filePath)}</span>
        {markdown && <span className="shrink-0 tabular-nums">{markdown.length.toLocaleString()}자</span>}
      </div>

      {/* 문서 크게 보기 — 레이아웃 렌더를 팝업으로 크게. 휠=스크롤, Ctrl/⌘+휠(트랙패드
          핀치)=줌, 너비맞춤은 창 크기 따라 스케일. 검증된 LayoutView 를 onClose 로 재사용.
          role=dialog 라 앱 bare-key 가드에 자동 편입(뒤 뷰 몰래 전환 방지). */}
      {viewerOpen && (isPdf ? true : !!layoutSvg) && (
        <div
          ref={viewerRef}
          role="dialog"
          aria-modal="true"
          aria-label="문서 크게 보기"
          tabIndex={-1}
          className="fixed inset-0 z-[1200] flex p-4 sm:p-8 outline-none"
          style={{ backgroundColor: "rgba(0,0,0,0.6)" }}
          onClick={(e) => { if (e.target === e.currentTarget) setViewerOpen(false); }}
        >
          <div
            className="flex-1 min-h-0 rounded-xl overflow-hidden border shadow-2xl"
            style={{ borderColor: "var(--color-border)", backgroundColor: "var(--color-bg-primary)" }}
          >
            {isPdf ? (
              <PdfLayoutView filePath={filePath} onClose={() => setViewerOpen(false)} />
            ) : (
              <LayoutView
                svg={layoutSvg!}
                onClose={() => setViewerOpen(false)}
                findTerm={findOpen ? findTerm.trim() || undefined : undefined}
              />
            )}
          </div>
        </div>
      )}
    </div>
  );
});
