import { memo, useEffect, useState, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { X, FileText, Copy, ExternalLink, FolderOpen, Bookmark } from "lucide-react";
import { FileIcon } from "../ui/FileIcon";
import { Badge, getFileTypeBadgeVariant } from "../ui/Badge";

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

  useEffect(() => {
    if (!filePath) {
      setPreview(null);
      return;
    }

    let cancelled = false;
    setLoading(true);
    setError(null);

    invoke<PreviewResponse>("load_document_preview", { filePath })
      .then((res) => {
        if (!cancelled) {
          setPreview(res);
          setLoading(false);
          // 스크롤 상단으로
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

  const highlightText = useCallback((text: string): React.ReactNode => {
    if (!highlightQuery?.trim()) return text;
    const keywords = highlightQuery.trim().split(/\s+/).filter(Boolean);
    if (keywords.length === 0) return text;

    const pattern = keywords.map(k => k.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')).join('|');
    const regex = new RegExp(`(${pattern})`, 'gi');
    const parts = text.split(regex);

    // split(/(pattern)/) → 홀수 인덱스가 매칭된 부분
    return parts.map((part, i) =>
      i % 2 === 1 ? (
        <mark
          key={i}
          className="rounded px-0.5"
          style={{
            backgroundColor: "var(--color-highlight-bg)",
            color: "var(--color-highlight-text)",
          }}
        >
          {part}
        </mark>
      ) : (
        part
      )
    );
  }, [highlightQuery]);

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
      <div className="flex items-center gap-1 px-3 py-1.5 border-b text-xs" style={{ borderColor: "var(--color-border)" }}>
        <button
          onClick={() => onOpenFile?.(filePath)}
          className="flex items-center gap-1 px-2 py-1 rounded hover:bg-[var(--color-bg-tertiary)] text-[var(--color-text-secondary)] transition-colors"
          title="파일 열기"
        >
          <ExternalLink size={12} />
          열기
        </button>
        <button
          onClick={() => onCopyPath?.(filePath)}
          className="flex items-center gap-1 px-2 py-1 rounded hover:bg-[var(--color-bg-tertiary)] text-[var(--color-text-secondary)] transition-colors"
          title="경로 복사"
        >
          <Copy size={12} />
          복사
        </button>
        <button
          onClick={() => onOpenFolder?.(dirPath)}
          className="flex items-center gap-1 px-2 py-1 rounded hover:bg-[var(--color-bg-tertiary)] text-[var(--color-text-secondary)] transition-colors"
          title="폴더 열기"
        >
          <FolderOpen size={12} />
          폴더
        </button>
        {onBookmark && (
          <button
            onClick={() => onBookmark(filePath, preview?.chunks?.[0]?.content?.slice(0, 200) || "", null, null)}
            className="flex items-center gap-1 px-2 py-1 rounded hover:bg-[var(--color-bg-tertiary)] text-[var(--color-text-secondary)] transition-colors"
            title="북마크 추가"
          >
            <Bookmark size={12} />
            북마크
          </button>
        )}

        {preview && (
          <span className="ml-auto text-[var(--color-text-muted)]">
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
                  <p className="text-[13px] leading-relaxed text-[var(--color-text-secondary)] whitespace-pre-wrap break-words" style={{ fontFamily: "var(--font-mono)" }}>
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
