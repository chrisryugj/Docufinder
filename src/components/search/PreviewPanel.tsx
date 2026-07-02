import { memo, useEffect, useState, useRef, useCallback, useMemo, Fragment, type ComponentProps } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { X, FileText, Copy, ExternalLink, FolderOpen, Bookmark, Sparkles, ChevronDown, ChevronUp, MessageSquare, ClipboardCopy, Save, Search } from "lucide-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import rehypeKatex from "rehype-katex";
import "katex/dist/katex.min.css";
import { save } from "@tauri-apps/plugin-dialog";
import { FileIcon } from "../ui/FileIcon";
import { Badge, getFileTypeBadgeVariant } from "../ui/Badge";
import { TagInput } from "../ui/TagInput";
import type { AiAnalysis } from "../../types/search";
import { extractLegalReferences } from "../../utils/legalReference";
import { cleanPath } from "../../utils/cleanPath";
import { useUIContext } from "../../contexts/UIContext";

// ─── Types ─────────────────────────────────────────────

interface MarkdownPreviewResponse {
  file_path: string;
  file_name: string;
  markdown: string;
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

// ─── 찾기 바(Ctrl+F) 하이라이트 ───────────────────────
// 찾기 매치를 먼저 mark.hl-find 로 분리하고, 나머지 구간에는 기존
// 검색어/법령 하이라이트(highlightTextWithLegal)를 그대로 적용한다.
function highlightWithFind(
  text: string,
  searchRegex: RegExp | null,
  findRegex: RegExp | null,
  onOpenUrl: (url: string) => void,
): React.ReactNode {
  if (!findRegex || !text) return highlightTextWithLegal(text, searchRegex, onOpenUrl);
  const parts = text.split(new RegExp(`(${findRegex.source})`, "gi"));
  if (parts.length === 1) return highlightTextWithLegal(text, searchRegex, onOpenUrl);
  return (
    <>
      {parts.map((part, i) =>
        i % 2 === 1 ? (
          <mark key={`find-${i}`} className="hl-find">{part}</mark>
        ) : (
          <Fragment key={`seg-${i}`}>
            {highlightTextWithLegal(part, searchRegex, onOpenUrl)}
          </Fragment>
        ),
      )}
    </>
  );
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

// ─── HTML 태그 전처리 (kordoc이 HTML 표를 반환하는 경우 대응) ──

/** HTML 표를 마크다운 테이블로 변환, 기타 HTML 태그 제거 */
function stripHtmlForMarkdown(md: string): string {
  // HTML <table>을 마크다운 테이블로 변환
  const result = md.replace(/<table[^>]*>[\s\S]*?<\/table>/gi, (table) => {
    const rows: string[][] = [];
    // 각 행 추출
    const trRegex = /<tr[^>]*>([\s\S]*?)<\/tr>/gi;
    let trMatch;
    while ((trMatch = trRegex.exec(table)) !== null) {
      const cells: string[] = [];
      const cellRegex = /<t[dh][^>]*>([\s\S]*?)<\/t[dh]>/gi;
      let cellMatch;
      while ((cellMatch = cellRegex.exec(trMatch[1])) !== null) {
        // 셀 내부 HTML 태그 제거 + 트림
        cells.push(cellMatch[1].replace(/<[^>]+>/g, "").trim());
      }
      if (cells.length > 0) rows.push(cells);
    }
    if (rows.length === 0) return "";

    // 최대 열 수에 맞춰 정규화
    const maxCols = Math.max(...rows.map((r) => r.length));
    const normalized = rows.map((r) => {
      while (r.length < maxCols) r.push("");
      return r;
    });

    // 마크다운 테이블 생성
    const header = `| ${normalized[0].join(" | ")} |`;
    const separator = `| ${normalized[0].map(() => "---").join(" | ")} |`;
    const body = normalized.slice(1).map((r) => `| ${r.join(" | ")} |`).join("\n");
    return `\n${header}\n${separator}\n${body}\n`;
  });

  // 나머지 <br> + 변환되지 못한 잔여 표 태그 정리.
  // 셀 안에 중첩 <table>이 있으면 위 정규식이 바깥 표의 닫는 부분을 놓쳐
  // </td></tr>…</table> 가 본문에 그대로 노출된다(정규식은 중첩을 셀 수 없음).
  // ReactMarkdown은 rehype-raw 없이 이를 텍스트로 흘려보내므로 후처리로 방어한다.
  return result
    .replace(/<br\s*\/?>/gi, " ")
    .replace(/<img[^>]*>/gi, "")
    .replace(/<\/(?:td|th)>\s*/gi, " ")
    .replace(/<\/(?:tr|table)>\s*/gi, "\n")
    .replace(/<(?:table|thead|tbody|tfoot|tr|td|th|col|colgroup)[^>]*>/gi, "")
    .replace(/<\/(?:thead|tbody|tfoot|colgroup)>/gi, "");
}

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
  findRegex: RegExp | null,
  onOpenUrl: (url: string) => void,
): ComponentProps<typeof ReactMarkdown>["components"] {
  // 텍스트 노드에 하이라이트 적용하는 래퍼
  const TextWrapper = ({ children }: { children: React.ReactNode }) => {
    if (typeof children === "string") {
      return <>{highlightWithFind(children, searchRegex, findRegex, onOpenUrl)}</>;
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
    th: ({ children }) => <th className="doc-th"><TextWrapper>{children}</TextWrapper></th>,
    td: ({ children }) => <td className="doc-td"><TextWrapper>{children}</TextWrapper></td>,
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
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const contentRef = useRef<HTMLDivElement>(null);

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

  // 텍스트 내보내기 메뉴 토글 (복사 버튼 아래 드롭다운 팝오버)
  const [showExportMenu, setShowExportMenu] = useState(false);
  const exportMenuRef = useRef<HTMLDivElement>(null);

  // 찾기 바 (Ctrl+F) — 문서 내 즉석 찾기
  const panelRef = useRef<HTMLDivElement>(null);
  const findInputRef = useRef<HTMLInputElement>(null);
  const findComposingRef = useRef(false); // IME 조합 중 여부 (compositionend 후 검색)
  const [findOpen, setFindOpen] = useState(false);
  const [findInput, setFindInput] = useState("");
  const [findTerm, setFindTerm] = useState(""); // 디바운스 + 조합 완료 후 확정값
  const [findCount, setFindCount] = useState(0);
  const [findActiveIdx, setFindActiveIdx] = useState(0);

  const { showToast, updateToast } = useUIContext();

  // 파싱된 텍스트 복사
  const handleCopyText = useCallback(async () => {
    if (!markdown) return;
    setShowExportMenu(false);
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
    setShowExportMenu(false);
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
      return;
    }
    summaryRequestId.current++;
    setAiSummary(null);
    setSummaryError(null);
    setShowSummaryMenu(false);
    setShowFileQa(false);
    setShowExportMenu(false);
    setFindOpen(false);
    setFindInput("");
    setFindTerm("");
    setFindActiveIdx(0);

    let cancelled = false;
    setLoading(true);
    setError(null);

    // 빠른 탐색 시 불필요한 파싱 방지를 위해 300ms debounce (화살표 키 고속 이동 대응)
    const timer = setTimeout(() => {
      invoke<MarkdownPreviewResponse>("load_markdown_preview", { filePath })
        .then((res) => {
          if (!cancelled) {
            setMarkdown(res.markdown);
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
    const root = contentRef.current;
    if (!root) return;

    const raf = requestAnimationFrame(() => {
      const el = findJumpTarget(root, jumpTarget.anchors, jumpTarget.page);
      jumpDoneRef.current = jumpTarget.token;
      if (el) {
        el.scrollIntoView({ block: "center", behavior: "smooth" });
        el.classList.add("cite-flash");
        window.setTimeout(() => el.classList.remove("cite-flash"), 2200);
      } else {
        root.scrollTo({ top: 0, behavior: "smooth" });
      }
    });
    return () => cancelAnimationFrame(raf);
  }, [jumpTarget, loading, markdown]);

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

  // AI 요약용 — 찾기 하이라이트 미적용 (찾기는 문서 본문 한정)
  const markdownComponents = useMemo(
    () => createMarkdownComponents(searchRegex, null, handleOpenUrl),
    [searchRegex, handleOpenUrl],
  );

  // 문서 본문용 — 찾기(Ctrl+F) 하이라이트 포함
  const previewMarkdownComponents = useMemo(
    () => createMarkdownComponents(searchRegex, activeFindRegex, handleOpenUrl),
    [searchRegex, activeFindRegex, handleOpenUrl],
  );

  // 본문 전처리 캐시 — stripHtmlForMarkdown은 전문 대상 정규식 다중 패스라
  // 렌더마다 실행하면 대형 문서에서 키 입력마다 수십~수백 ms를 태운다
  const processedMarkdown = useMemo(
    () => (markdown ? stripHtmlForMarkdown(markdown) : null),
    [markdown],
  );

  // 본문 파싱 캐시 — react-markdown은 memo가 아니라 부모 리렌더마다 remark/katex
  // 전체를 재파싱한다. 찾기 입력(디바운스 전 키스트로크)·매치 이동·메뉴 토글 등
  // 무관한 상태 변화에 수만 줄 문서를 재파싱하지 않도록, 내용과 하이라이트
  // 확정값(searchRegex/activeFindRegex)이 바뀔 때만 재구성한다.
  const previewMarkdownNode = useMemo(() => {
    if (processedMarkdown === null) return null;
    return (
      <ReactMarkdown
        remarkPlugins={[[remarkGfm, { singleTilde: false }], remarkMath]}
        rehypePlugins={[rehypeKatex]}
        components={previewMarkdownComponents}
      >
        {processedMarkdown}
      </ReactMarkdown>
    );
  }, [processedMarkdown, previewMarkdownComponents]);

  // ── 찾기 바 동작 ──

  const closeFind = useCallback(() => {
    setFindOpen(false);
    setFindActiveIdx(0);
  }, []);

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

  // Ctrl+F 토글 — 패널이 열려 있고 포커스/호버 컨텍스트일 때만 (전역 Ctrl+F 충돌 방지)
  useEffect(() => {
    if (!filePath) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey) || e.shiftKey || e.altKey) return;
      if (e.key.toLowerCase() !== "f" && e.code !== "KeyF") return;
      const panel = panelRef.current;
      if (!panel) return;
      if (!panel.contains(document.activeElement) && !panel.matches(":hover")) return;
      e.preventDefault();
      setFindOpen((v) => !v);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [filePath]);

  // 열릴 때 입력 포커스 (기존 검색어 유지 시 전체 선택)
  useEffect(() => {
    if (findOpen) {
      findInputRef.current?.focus();
      findInputRef.current?.select();
    }
  }, [findOpen]);

  // 매치 수집 — ReactMarkdown 커밋 후 DOM에서 mark.hl-find 질의 (렌더 순서 = 문서 순서)
  useEffect(() => {
    const root = contentRef.current;
    if (!activeFindRegex || loading || !markdown || !root) {
      setFindCount(0);
      setFindActiveIdx(0);
      return;
    }
    setFindCount(root.querySelectorAll("mark.hl-find").length);
    setFindActiveIdx(0);
  }, [activeFindRegex, markdown, loading]);

  // 활성 매치 강조 + 스크롤 — DOM class 직접 토글 (마크다운 전체 재렌더 회피)
  useEffect(() => {
    const root = contentRef.current;
    if (!root) return;
    const marks = root.querySelectorAll<HTMLElement>("mark.hl-find");
    marks.forEach((m, i) => m.classList.toggle("hl-find-active", i === findActiveIdx));
    if (marks.length > 0) marks[findActiveIdx]?.scrollIntoView({ block: "center" });
  }, [findActiveIdx, findCount, activeFindRegex]);

  // 복사/내보내기 드롭다운 — 바깥 클릭 시 닫기
  useEffect(() => {
    if (!showExportMenu) return;
    const onPointerDown = (e: MouseEvent) => {
      if (exportMenuRef.current && !exportMenuRef.current.contains(e.target as Node)) {
        setShowExportMenu(false);
      }
    };
    document.addEventListener("mousedown", onPointerDown);
    return () => document.removeEventListener("mousedown", onPointerDown);
  }, [showExportMenu]);

  if (!filePath) return null;

  const ext = filePath.split(".").pop()?.toLowerCase() || "";
  const fileName = filePath.split(/[/\\]/).pop() || filePath;
  const hasAiContent = aiSummary || summaryError || summaryLoading || showFileQa;

  return (
    <div ref={panelRef} onKeyDown={handlePanelKeyDown} className="preview-panel flex flex-col h-full border-l bg-[var(--color-bg-primary)]" style={{ borderColor: "var(--color-border)", minWidth: "320px" }}>
      {/* 헤더 */}
      <div className="flex items-center gap-2 px-3 py-2 border-b bg-[var(--color-bg-secondary)]" style={{ borderColor: "var(--color-border)" }}>
        <FileIcon fileName={fileName} size="sm" />
        <span className="flex-1 text-sm font-medium truncate text-[var(--color-text-primary)]" title={fileName}>
          {fileName}
        </span>
        <Badge variant={getFileTypeBadgeVariant(fileName)}>
          {ext.toUpperCase()}
        </Badge>
        <button onClick={onClose} className="p-1 rounded-lg hover:bg-[var(--color-bg-tertiary)] text-[var(--color-text-muted)] transition-colors" title="닫기" aria-label="닫기">
          <X size={14} />
        </button>
      </div>

      {/* 액션 바 — 아이콘 전용, 컴팩트 */}
      <div className="flex items-center gap-0.5 px-2 py-1 border-b" style={{ borderColor: "var(--color-border)" }}>
        <button onClick={() => onOpenFile?.(filePath)} className="p-1.5 rounded-lg hover:bg-[var(--color-bg-tertiary)] text-[var(--color-text-secondary)] transition-colors" title="파일 열기">
          <ExternalLink size={14} />
        </button>
        <div className="relative inline-flex" ref={exportMenuRef}>
          <button
            onClick={() => setShowExportMenu((v) => !v)}
            className={`p-1.5 rounded-lg transition-colors ${showExportMenu ? "text-[var(--color-accent)] bg-[var(--color-accent-light)]" : "text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-tertiary)]"}`}
            title="복사 / 내보내기"
            aria-haspopup="menu"
            aria-expanded={showExportMenu}
          >
            <Copy size={14} />
          </button>
          {showExportMenu && (
            <div
              role="menu"
              className="absolute left-0 top-full mt-1.5 z-20 min-w-[160px] py-1 rounded-lg border overflow-hidden"
              style={{ borderColor: "var(--color-border)", backgroundColor: "var(--color-bg-secondary)", boxShadow: "var(--shadow-premium)" }}
            >
              {markdown && (
                <>
                  <button onClick={handleCopyText} role="menuitem" className="export-dropdown-item" title="파싱된 텍스트를 클립보드에 복사">
                    <ClipboardCopy size={13} />텍스트 복사
                  </button>
                  <button onClick={handleExportMarkdown} role="menuitem" className="export-dropdown-item" title=".md 파일로 저장">
                    <Save size={13} />Markdown 저장
                  </button>
                </>
              )}
              <button onClick={() => { setShowExportMenu(false); onCopyPath?.(filePath); }} role="menuitem" className="export-dropdown-item" title="파일 경로를 클립보드에 복사">
                <Copy size={13} />경로 복사
              </button>
            </div>
          )}
        </div>
        <button onClick={() => onOpenFolder?.(filePath)} className="p-1.5 rounded-lg hover:bg-[var(--color-bg-tertiary)] text-[var(--color-text-secondary)] transition-colors" title="파일 위치 열기 (탐색기에서 선택)">
          <FolderOpen size={14} />
        </button>
        {onBookmark && (
          <button
            onClick={() => onBookmark(filePath, markdown?.slice(0, 200) || "", null, null)}
            className={`p-1.5 rounded-lg transition-colors ${isBookmarked ? "text-[var(--color-accent)]" : "text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-tertiary)]"}`}
            title={isBookmarked ? "북마크 해제" : "북마크 추가"}
          >
            <Bookmark size={14} fill={isBookmarked ? "currentColor" : "none"} />
          </button>
        )}

        <div className="w-px h-4 mx-0.5" style={{ backgroundColor: "var(--color-border)" }} />

        {markdown && (
          <>
            <button
              onClick={() => setFindOpen((v) => !v)}
              className={`p-1.5 rounded-lg transition-colors ${findOpen ? "text-[var(--color-accent)] bg-[var(--color-accent-light)]" : "text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-tertiary)]"}`}
              title="문서 내 찾기 (Ctrl+F)"
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

      {/* 태그 */}
      {onAddTag && filePath && (
        <div className="px-3 py-1.5 border-b" style={{ borderColor: "var(--color-border)" }}>
          <TagInput
            tags={tags}
            suggestions={tagSuggestions}
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

      {/* 마크다운 스크롤 영역 */}
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
        {!loading && !error && markdown && (
          <div className="doc-preview px-6 py-5">{previewMarkdownNode}</div>
        )}
      </div>

      {/* 경로 + 글자수 */}
      <div
        className="flex items-center gap-2 px-3 py-1.5 border-t text-[10px] text-[var(--color-text-muted)]"
        style={{ borderColor: "var(--color-border)" }}
      >
        <span className="truncate flex-1" title={cleanPath(filePath)}>{cleanPath(filePath)}</span>
        {markdown && <span className="shrink-0 tabular-nums">{markdown.length.toLocaleString()}자</span>}
      </div>
    </div>
  );
});
