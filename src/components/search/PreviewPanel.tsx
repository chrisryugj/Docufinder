import { memo, useEffect, useState, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { X, FileText, Copy, ExternalLink, FolderOpen, Bookmark, Sparkles, ChevronDown, ChevronUp } from "lucide-react";
import { FileIcon } from "../ui/FileIcon";
import { Badge, getFileTypeBadgeVariant } from "../ui/Badge";
import type { SummaryResponse, SummarySentence } from "../../types/search";
import { extractLegalReferences } from "../../utils/legalReference";

interface PreviewChunk {
  chunk_id: number;
  chunk_index: number;
  content: string;
  page_number: number | null;
  location_hint: string | null;
}

interface PreviewResponse {
  file_path: string;
  file_name: string;
  chunks: PreviewChunk[];
  total_chars: number;
}

interface PreviewPanelProps {
  filePath: string | null;
  highlightQuery?: string;
  onClose: () => void;
  onOpenFile?: (filePath: string, page?: number | null) => void;
  onCopyPath?: (path: string) => void;
  onOpenFolder?: (path: string) => void;
  onBookmark?: (filePath: string, contentPreview: string, pageNumber?: number | null, locationHint?: string | null) => void;
}

export const PreviewPanel = memo(function PreviewPanel({
  filePath,
  highlightQuery,
  onClose,
  onOpenFile,
  onCopyPath,
  onOpenFolder,
  onBookmark,
}: PreviewPanelProps) {
  const [preview, setPreview] = useState<PreviewResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const contentRef = useRef<HTMLDivElement>(null);

  // 요약 상태
  const [summary, setSummary] = useState<SummaryResponse | null>(null);
  const [summaryLoading, setSummaryLoading] = useState(false);
  const [summaryExpanded, setSummaryExpanded] = useState(true);
  const summaryRequestId = useRef(0);

  useEffect(() => {
    if (!filePath) {
      setPreview(null);
      setSummary(null);
      return;
    }

    let cancelled = false;
    setLoading(true);
    setError(null);
    setSummary(null);

    invoke<PreviewResponse>("load_document_preview", { filePath })
      .then((res) => {
        if (!cancelled) {
          setPreview(res);
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

    return () => { cancelled = true; };
  }, [filePath]);

  const handleGenerateSummary = useCallback(() => {
    if (!filePath || summaryLoading) return;
    const reqId = ++summaryRequestId.current;
    setSummaryLoading(true);

    invoke<SummaryResponse>("generate_summary", { filePath, numSentences: 3 })
      .then((res) => {
        // stale 응답 무시 (파일 전환 시)
        if (summaryRequestId.current === reqId) {
          setSummary(res);
          setSummaryExpanded(true);
        }
      })
      .catch(() => {
        // 요약 실패는 무시 (핵심 기능 아님)
      })
      .finally(() => {
        if (summaryRequestId.current === reqId) {
          setSummaryLoading(false);
        }
      });
  }, [filePath, summaryLoading]);

  const handleOpenUrl = useCallback((url: string) => {
    invoke("open_url", { url }).catch(() => {
      // fallback: 브라우저에서 직접 열기 시도 무시
    });
  }, []);

  const highlightText = useCallback((text: string): React.ReactNode => {
    // 1단계: 법령 참조 감지
    const legalRefs = extractLegalReferences(text);

    // 2단계: 검색어 하이라이트 패턴
    let searchRegex: RegExp | null = null;
    if (highlightQuery?.trim()) {
      const keywords = highlightQuery.trim().split(/\s+/).filter(Boolean);
      if (keywords.length > 0) {
        const pattern = keywords.map(k => k.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')).join('|');
        searchRegex = new RegExp(pattern, 'gi');
      }
    }

    // 법령 참조도 없고 검색어도 없으면 텍스트 그대로
    if (legalRefs.length === 0 && !searchRegex) return text;

    // 법령 참조 없으면 기존 검색어 하이라이트만
    if (legalRefs.length === 0 && searchRegex) {
      const parts = text.split(new RegExp(`(${searchRegex.source})`, 'gi'));
      return parts.map((part, i) =>
        i % 2 === 1 ? (
          <mark key={i} className="hl-search">{part}</mark>
        ) : part
      );
    }

    // 법령 참조 기준으로 텍스트 분할 후 각 조각에 검색어 하이라이트 적용
    const segments: React.ReactNode[] = [];
    let lastEnd = 0;

    const applySearchHighlight = (str: string, keyBase: string): React.ReactNode[] => {
      if (!searchRegex || !str) return [str];
      const parts = str.split(new RegExp(`(${searchRegex.source})`, 'gi'));
      return parts.map((part, i) =>
        i % 2 === 1 ? (
          <mark key={`${keyBase}-h${i}`} className="hl-search">{part}</mark>
        ) : (
          <span key={`${keyBase}-t${i}`}>{part}</span>
        )
      );
    };

    for (let li = 0; li < legalRefs.length; li++) {
      const ref = legalRefs[li];
      // 법령 참조 앞 일반 텍스트
      if (ref.start > lastEnd) {
        segments.push(...applySearchHighlight(text.slice(lastEnd, ref.start), `pre-${li}`));
      }
      // 법령 참조 링크
      segments.push(
        <button
          key={`legal-${li}`}
          onClick={() => handleOpenUrl(ref.url)}
          className="inline underline decoration-dotted underline-offset-2 cursor-pointer hover:opacity-80 transition-opacity"
          style={{ color: "var(--color-accent)" }}
          title={`${ref.lawName ? ref.lawName + " " : ""}${ref.article || ref.text} — 법제처에서 열기`}
        >
          {ref.text}
        </button>
      );
      lastEnd = ref.end;
    }

    // 마지막 법령 참조 뒤 텍스트
    if (lastEnd < text.length) {
      segments.push(...applySearchHighlight(text.slice(lastEnd), "post"));
    }

    return segments;
  }, [highlightQuery, handleOpenUrl]);

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
            onClick={() => onBookmark(filePath, preview?.chunks?.[0]?.content?.slice(0, 200) || "", null, null)}
            className="flex items-center gap-1 px-1.5 py-1 rounded hover:bg-[var(--color-bg-tertiary)] text-[var(--color-text-secondary)] transition-colors shrink-0 whitespace-nowrap"
            title="북마크 추가"
          >
            <Bookmark size={12} />
            북마크
          </button>
        )}

        {preview && preview.chunks.length > 0 && (
          <button
            onClick={handleGenerateSummary}
            disabled={summaryLoading}
            className="flex items-center gap-1 px-1.5 py-1 rounded hover:bg-[var(--color-bg-tertiary)] text-[var(--color-text-secondary)] transition-colors disabled:opacity-50 shrink-0 whitespace-nowrap"
            title="AI 요약 생성 (TextRank)"
          >
            {summaryLoading ? (
              <div className="w-3 h-3 border border-[var(--color-accent)] border-t-transparent rounded-full animate-spin" />
            ) : (
              <Sparkles size={12} />
            )}
            요약
          </button>
        )}

        {preview && (
          <span className="ml-auto text-[var(--color-text-muted)] shrink-0 whitespace-nowrap">
            {preview.chunks.length}개 청크 · {preview.total_chars.toLocaleString()}자
          </span>
        )}
      </div>

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

        {!loading && !error && preview && preview.chunks.length === 0 && (
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
              요약 ({summary.sentences.length}문장 / {summary.total_sentences}문장)
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
                    <div className="flex-1">
                      <p className="text-[var(--color-text-primary)]">{s.text}</p>
                      {s.location_hint && (
                        <span className="text-[10px] text-[var(--color-text-muted)]">{s.location_hint}</span>
                      )}
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {!loading && !error && preview && preview.chunks.length > 0 && (
          <div className="p-4 space-y-1">
            {preview.chunks.map((chunk, i) => {
              const showPageHeader = i === 0 || chunk.location_hint !== preview.chunks[i - 1]?.location_hint;
              return (
                <div key={chunk.chunk_id}>
                  {showPageHeader && chunk.location_hint && (
                    <div className="mt-3 mb-1 first:mt-0">
                      <span className="text-[10px] font-semibold tracking-wider uppercase text-[var(--color-text-muted)]">
                        {chunk.location_hint}
                      </span>
                    </div>
                  )}
                  <p
                    className="whitespace-pre-wrap break-words text-[var(--color-text-secondary)]"
                    style={{
                      fontFamily: "var(--font-sans)",
                      fontSize: "var(--text-sm)",
                      lineHeight: "1.7",
                      letterSpacing: "0.3px",
                    }}
                  >
                    {highlightText(chunk.content)}
                  </p>
                </div>
              );
            })}
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
