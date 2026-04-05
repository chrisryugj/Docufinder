import { memo, useEffect, useState, useRef, useCallback, useMemo, type ComponentProps } from "react";
import { invoke } from "@tauri-apps/api/core";
import { X, FileText, Copy, ExternalLink, FolderOpen, Bookmark, Sparkles, ChevronDown, ChevronUp } from "lucide-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { FileIcon } from "../ui/FileIcon";
import { Badge, getFileTypeBadgeVariant } from "../ui/Badge";
import { TagInput } from "../ui/TagInput";
import type { SummaryResponse, SummarySentence } from "../../types/search";
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

  // 요약 상태
  const [summary, setSummary] = useState<SummaryResponse | null>(null);
  const [summaryLoading, setSummaryLoading] = useState(false);
  const [summaryExpanded, setSummaryExpanded] = useState(true);
  const summaryRequestId = useRef(0);

  // 파일 로드 (빠른 클릭 시 debounce로 불필요한 백엔드 파싱 방지)
  useEffect(() => {
    if (!filePath) {
      setMarkdown(null);
      setSummary(null);
      return;
    }

    summaryRequestId.current++;
    setSummary(null);

    let cancelled = false;
    setLoading(true);
    setError(null);

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
    }, 80); // 80ms debounce — 빠른 키보드 탐색 시 중간 파일 로드 방지

    return () => { cancelled = true; clearTimeout(timer); };
  }, [filePath]);

  // 요약 생성
  const handleGenerateSummary = useCallback(() => {
    if (!filePath || summaryLoading) return;
    const reqId = ++summaryRequestId.current;
    setSummaryLoading(true);

    invoke<SummaryResponse>("generate_summary", { filePath, numSentences: 3 })
      .then((res) => {
        if (summaryRequestId.current === reqId) {
          setSummary(res);
          setSummaryExpanded(true);
        }
      })
      .catch(() => {})
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

  // 마크다운 컴포넌트 (메모이즈)
  const markdownComponents = useMemo(
    () => createMarkdownComponents(searchRegex, handleOpenUrl),
    [searchRegex, handleOpenUrl],
  );

  if (!filePath) return null;

  const ext = filePath.split(".").pop()?.toLowerCase() || "";
  const fileName = filePath.split(/[/\\]/).pop() || filePath;
  const dirPath = filePath.replace(/[/\\][^/\\]*$/, "");

  return (
    <div className="flex flex-col h-full border-l bg-[var(--color-bg-primary)]" style={{ borderColor: "var(--color-border)" }}>
      {/* 헤더 */}
      <div className="flex items-center gap-2 px-3 py-2 border-b bg-[var(--color-bg-secondary)]" style={{ borderColor: "var(--color-border)" }}>
        <FileIcon fileName={fileName} size="sm" />
        <span className="flex-1 text-sm font-medium truncate text-[var(--color-text-primary)]" title={fileName}>
          {fileName}
        </span>
        <Badge variant={getFileTypeBadgeVariant(fileName)}>
          {ext.toUpperCase()}
        </Badge>
        <button
          onClick={onClose}
          className="p-1 rounded hover:bg-[var(--color-bg-tertiary)] text-[var(--color-text-muted)] transition-colors"
          title="닫기"
        >
          <X size={14} />
        </button>
      </div>

      {/* 액션 바 */}
      <div className="flex items-center gap-0.5 px-2 py-1.5 border-b text-xs overflow-x-auto" style={{ borderColor: "var(--color-border)" }}>
        <button
          onClick={() => onOpenFile?.(filePath)}
          className="flex items-center gap-1 px-1.5 py-1 rounded hover:bg-[var(--color-bg-tertiary)] text-[var(--color-text-secondary)] transition-colors shrink-0 whitespace-nowrap"
          title="파일 열기"
        >
          <ExternalLink size={12} />
          열기
        </button>
        <button
          onClick={() => onCopyPath?.(filePath)}
          className="flex items-center gap-1 px-1.5 py-1 rounded hover:bg-[var(--color-bg-tertiary)] text-[var(--color-text-secondary)] transition-colors shrink-0 whitespace-nowrap"
          title="경로 복사"
        >
          <Copy size={12} />
          복사
        </button>
        <button
          onClick={() => onOpenFolder?.(dirPath)}
          className="flex items-center gap-1 px-1.5 py-1 rounded hover:bg-[var(--color-bg-tertiary)] text-[var(--color-text-secondary)] transition-colors shrink-0 whitespace-nowrap"
          title="폴더 열기"
        >
          <FolderOpen size={12} />
          폴더
        </button>
        {onBookmark && (
          <button
            onClick={() => onBookmark(filePath, markdown?.slice(0, 200) || "", null, null)}
            className={`flex items-center gap-1 px-1.5 py-1 rounded transition-colors shrink-0 whitespace-nowrap ${
              isBookmarked
                ? "text-[var(--color-accent)] bg-[var(--color-accent-bg)]"
                : "text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-tertiary)]"
            }`}
            title={isBookmarked ? "북마크 해제" : "북마크 추가"}
          >
            <Bookmark size={12} fill={isBookmarked ? "currentColor" : "none"} />
            북마크
          </button>
        )}

        {markdown && (
          <button
            onClick={handleGenerateSummary}
            disabled={summaryLoading}
            className="flex items-center gap-1 px-1.5 py-1 rounded hover:bg-[var(--color-bg-tertiary)] text-[var(--color-text-secondary)] transition-colors disabled:opacity-50 shrink-0 whitespace-nowrap"
            title="요약 생성"
          >
            {summaryLoading ? (
              <div className="w-3 h-3 border border-[var(--color-accent)] border-t-transparent rounded-full animate-spin" />
            ) : (
              <Sparkles size={12} />
            )}
            요약
          </button>
        )}

        {markdown && (
          <span className="ml-auto text-[var(--color-text-muted)] shrink-0 whitespace-nowrap">
            {markdown.length.toLocaleString()}자
          </span>
        )}
      </div>

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

      {/* 콘텐츠 */}
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

        {/* 요약 섹션 */}
        {summary && summary.sentences.length > 0 && (
          <div className="mx-4 mt-3 mb-1 rounded-lg border" style={{ borderColor: "var(--color-accent)", backgroundColor: "color-mix(in srgb, var(--color-accent) 5%, var(--color-bg-primary))" }}>
            <button
              onClick={() => setSummaryExpanded(!summaryExpanded)}
              className="flex items-center gap-2 w-full px-3 py-2 text-xs font-medium text-[var(--color-accent)]"
            >
              <Sparkles size={12} />
              요약 ({summary.sentences.length}문장)
              <span className="ml-auto text-[var(--color-text-muted)] font-normal">
                {summary.generation_time_ms}ms
              </span>
              {summaryExpanded ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
            </button>
            {summaryExpanded && (
              <div className="px-3 pb-3 space-y-2">
                {summary.sentences.map((s: SummarySentence, i: number) => (
                  <div key={i} className="flex gap-2 text-[13px] leading-relaxed">
                    <span className="shrink-0 mt-0.5 w-5 h-5 flex items-center justify-center rounded-full text-[10px] font-bold"
                      style={{ backgroundColor: "var(--color-accent)", color: "white" }}>
                      {i + 1}
                    </span>
                    <p className="flex-1 text-[var(--color-text-primary)]">{s.text}</p>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {/* 마크다운 렌더링 — 문서 스타일 */}
        {!loading && !error && markdown && (
          <div className="doc-preview px-6 py-5">
            <ReactMarkdown
              remarkPlugins={[remarkGfm]}
              components={markdownComponents}
            >
              {markdown}
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
