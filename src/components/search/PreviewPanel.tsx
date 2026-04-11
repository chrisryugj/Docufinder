import { memo, useEffect, useState, useRef, useCallback, useMemo, type ComponentProps } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { X, FileText, Copy, ExternalLink, FolderOpen, Bookmark, Sparkles, ChevronDown, ChevronUp, MessageSquare } from "lucide-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { FileIcon } from "../ui/FileIcon";
import { Badge, getFileTypeBadgeVariant } from "../ui/Badge";
import { TagInput } from "../ui/TagInput";
import type { AiAnalysis } from "../../types/search";
import { extractLegalReferences } from "../../utils/legalReference";

// ─── Types ─────────────────────────────────────────────

interface MarkdownPreviewResponse {
  file_path: string;
  file_name: string;
  markdown: string;
}

interface PreviewPanelProps {
  filePath: string | null;
  highlightQuery?: string;
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
        className="inline underline decoration-dotted underline-offset-2 cursor-pointer hover:opacity-80 transition-opacity"
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

  // 나머지 <br> 태그 처리
  return result.replace(/<br\s*\/?>/gi, " ");
}

// ─── 마크다운 커스텀 컴포넌트 ──────────────────────────

function createMarkdownComponents(
  searchRegex: RegExp | null,
  onOpenUrl: (url: string) => void,
): ComponentProps<typeof ReactMarkdown>["components"] {
  // 텍스트 노드에 하이라이트 적용하는 래퍼
  const TextWrapper = ({ children }: { children: React.ReactNode }) => {
    if (typeof children === "string") {
      return <>{highlightTextWithLegal(children, searchRegex, onOpenUrl)}</>;
    }
    return <>{children}</>;
  };

  return {
    // 텍스트가 포함된 블록 요소에 하이라이트 적용
    p: ({ children }) => (
      <p className="doc-paragraph">
        {Array.isArray(children)
          ? children.map((child, i) => <TextWrapper key={i}>{child}</TextWrapper>)
          : <TextWrapper>{children}</TextWrapper>}
      </p>
    ),
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
        className="inline underline decoration-dotted underline-offset-2 cursor-pointer hover:opacity-80"
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
              <ReactMarkdown remarkPlugins={[remarkGfm]}>{answer}</ReactMarkdown>
            </div>
          )}

          {/* 메타 / 초기화 */}
          {analysis && (
            <div className="mt-3 pt-2 border-t flex items-center" style={{ borderColor: "var(--color-border)" }}>
              <button
                onClick={() => { setAnswer(""); setAnalysis(null); setError(null); setQuestion(""); }}
                className="text-[10px] px-2 py-0.5 rounded transition-colors hover:bg-[var(--color-bg-tertiary)]"
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

  const markdownComponents = useMemo(
    () => createMarkdownComponents(searchRegex, handleOpenUrl),
    [searchRegex, handleOpenUrl],
  );

  if (!filePath) return null;

  const ext = filePath.split(".").pop()?.toLowerCase() || "";
  const fileName = filePath.split(/[/\\]/).pop() || filePath;
  const dirPath = filePath.replace(/[/\\][^/\\]*$/, "");
  const hasAiContent = aiSummary || summaryError || summaryLoading || showFileQa;

  return (
    <div className="flex flex-col h-full border-l bg-[var(--color-bg-primary)]" style={{ borderColor: "var(--color-border)", minWidth: "320px" }}>
      {/* 헤더 */}
      <div className="flex items-center gap-2 px-3 py-2 border-b bg-[var(--color-bg-secondary)]" style={{ borderColor: "var(--color-border)" }}>
        <FileIcon fileName={fileName} size="sm" />
        <span className="flex-1 text-sm font-medium truncate text-[var(--color-text-primary)]" title={fileName}>
          {fileName}
        </span>
        <Badge variant={getFileTypeBadgeVariant(fileName)}>
          {ext.toUpperCase()}
        </Badge>
        <button onClick={onClose} className="p-1 rounded hover:bg-[var(--color-bg-tertiary)] text-[var(--color-text-muted)] transition-colors" title="닫기" aria-label="닫기">
          <X size={14} />
        </button>
      </div>

      {/* 액션 바 — 아이콘 전용, 컴팩트 */}
      <div className="flex items-center gap-0.5 px-2 py-1 border-b" style={{ borderColor: "var(--color-border)" }}>
        <button onClick={() => onOpenFile?.(filePath)} className="p-1.5 rounded hover:bg-[var(--color-bg-tertiary)] text-[var(--color-text-secondary)] transition-colors" title="파일 열기">
          <ExternalLink size={13} />
        </button>
        <button onClick={() => onCopyPath?.(filePath)} className="p-1.5 rounded hover:bg-[var(--color-bg-tertiary)] text-[var(--color-text-secondary)] transition-colors" title="경로 복사">
          <Copy size={13} />
        </button>
        <button onClick={() => onOpenFolder?.(dirPath)} className="p-1.5 rounded hover:bg-[var(--color-bg-tertiary)] text-[var(--color-text-secondary)] transition-colors" title="폴더 열기">
          <FolderOpen size={13} />
        </button>
        {onBookmark && (
          <button
            onClick={() => onBookmark(filePath, markdown?.slice(0, 200) || "", null, null)}
            className={`p-1.5 rounded transition-colors ${isBookmarked ? "text-[var(--color-accent)]" : "text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-tertiary)]"}`}
            title={isBookmarked ? "북마크 해제" : "북마크 추가"}
          >
            <Bookmark size={13} fill={isBookmarked ? "currentColor" : "none"} />
          </button>
        )}

        <div className="w-px h-4 mx-0.5" style={{ backgroundColor: "var(--color-border)" }} />

        {markdown && (
          <>
            <button
              onClick={() => setShowSummaryMenu((v) => !v)}
              disabled={summaryLoading}
              className="flex items-center gap-1 px-1.5 py-1 rounded text-xs hover:bg-[var(--color-bg-tertiary)] text-[var(--color-text-secondary)] transition-colors disabled:opacity-50"
              title="AI 요약"
            >
              {summaryLoading
                ? <div className="w-3 h-3 border border-[var(--color-accent)] border-t-transparent rounded-full animate-spin" />
                : <Sparkles size={12} />
              }
              요약
            </button>
            <button
              onClick={() => setShowFileQa((v) => !v)}
              className={`flex items-center gap-1 px-1.5 py-1 rounded text-xs transition-colors ${showFileQa ? "text-[var(--color-accent)] bg-[var(--color-accent-light)]" : "text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-tertiary)]"}`}
              title="이 파일에 대해 질문"
            >
              <MessageSquare size={12} />질문
            </button>
          </>
        )}

        {markdown && (
          <span className="ml-auto text-[10px] text-[var(--color-text-muted)] tabular-nums">
            {markdown.length.toLocaleString()}자
          </span>
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
              className="px-2 py-0.5 rounded text-[11px] transition-colors"
              style={{
                backgroundColor: summaryType === type && aiSummary ? "var(--color-accent-light)" : "var(--color-bg-tertiary)",
                color: summaryType === type && aiSummary ? "var(--color-accent)" : "var(--color-text-secondary)",
                border: "1px solid var(--color-border)",
              }}
            >
              {SUMMARY_TYPE_LABELS[type]}
            </button>
          ))}
          <button onClick={() => setShowSummaryMenu(false)} className="ml-auto text-[var(--color-text-muted)] hover:text-[var(--color-text-primary)] p-0.5 rounded">
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
        <div className="border-b overflow-hidden ai-section-enter shrink-0" style={{ borderColor: "var(--color-accent)", backgroundColor: "var(--color-accent-light)" }}>

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
                  <ReactMarkdown remarkPlugins={[remarkGfm]} components={markdownComponents}>
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
          <div className="doc-preview px-6 py-5">
            <ReactMarkdown remarkPlugins={[remarkGfm]} components={markdownComponents}>
              {stripHtmlForMarkdown(markdown)}
            </ReactMarkdown>
          </div>
        )}
      </div>

      {/* 경로 표시 */}
      <div
        className="px-3 py-1.5 border-t text-[10px] text-[var(--color-text-muted)] truncate"
        style={{ borderColor: "var(--color-border)" }}
        title={filePath}
      >
        {filePath}
      </div>
    </div>
  );
});
