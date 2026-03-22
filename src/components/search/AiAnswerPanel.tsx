import { useState, memo } from "react";
import { Sparkles, ChevronDown, ChevronUp, Clock, FileText, X, Loader2 } from "lucide-react";
import type { AiAnalysis } from "../../types/search";

interface AiAnswerPanelProps {
  analysis: AiAnalysis | null;
  isLoading: boolean;
  error: string | null;
  onDismiss: () => void;
  onOpenFile?: (filePath: string) => void;
}

export const AiAnswerPanel = memo(function AiAnswerPanel({
  analysis,
  isLoading,
  error,
  onDismiss,
  onOpenFile,
}: AiAnswerPanelProps) {
  const [isExpanded, setIsExpanded] = useState(true);

  // 아무것도 표시할 게 없으면 렌더링 안 함
  if (!isLoading && !analysis && !error) return null;

  return (
    <div
      className="mb-4 rounded-lg overflow-hidden"
      style={{
        backgroundColor: "var(--color-bg-secondary)",
        border: "1px solid var(--color-border)",
      }}
    >
      {/* 헤더 */}
      <div
        className="flex items-center justify-between px-4 py-2.5 cursor-pointer select-none"
        style={{ borderBottom: isExpanded ? "1px solid var(--color-border)" : "none" }}
        onClick={() => setIsExpanded(!isExpanded)}
      >
        <div className="flex items-center gap-2">
          <Sparkles className="w-4 h-4" style={{ color: "var(--color-accent)" }} />
          <span className="text-sm font-medium" style={{ color: "var(--color-text-primary)" }}>
            AI 답변
          </span>
          {isLoading && (
            <Loader2
              className="w-3.5 h-3.5 animate-spin"
              style={{ color: "var(--color-text-muted)" }}
            />
          )}
          {analysis && (
            <span className="text-xs" style={{ color: "var(--color-text-muted)" }}>
              <Clock className="w-3 h-3 inline mr-0.5" />
              {(analysis.processing_time_ms / 1000).toFixed(1)}s
              {analysis.tokens_used && ` · ${analysis.tokens_used.total_tokens} tokens`}
            </span>
          )}
        </div>
        <div className="flex items-center gap-1">
          <button
            onClick={(e) => {
              e.stopPropagation();
              onDismiss();
            }}
            className="p-1 rounded hover:bg-[var(--color-bg-tertiary)] transition-colors"
            style={{ color: "var(--color-text-muted)" }}
            aria-label="AI 답변 닫기"
          >
            <X className="w-3.5 h-3.5" />
          </button>
          {isExpanded ? (
            <ChevronUp className="w-4 h-4" style={{ color: "var(--color-text-muted)" }} />
          ) : (
            <ChevronDown className="w-4 h-4" style={{ color: "var(--color-text-muted)" }} />
          )}
        </div>
      </div>

      {/* 본문 */}
      {isExpanded && (
        <div className="px-4 py-3">
          {/* 로딩 */}
          {isLoading && !analysis && (
            <div className="flex items-center gap-2 py-2">
              <div
                className="w-2 h-2 rounded-full animate-pulse"
                style={{ backgroundColor: "var(--color-accent)" }}
              />
              <span className="text-sm" style={{ color: "var(--color-text-muted)" }}>
                문서를 분석하고 있습니다...
              </span>
            </div>
          )}

          {/* 에러 */}
          {error && (
            <div
              className="text-sm p-2.5 rounded-md"
              style={{
                backgroundColor: "rgba(239, 68, 68, 0.1)",
                border: "1px solid rgba(239, 68, 68, 0.3)",
                color: "var(--color-error)",
              }}
            >
              {error}
            </div>
          )}

          {/* AI 답변 */}
          {analysis && (
            <div className="space-y-3">
              {/* 마크다운 답변 (간단한 렌더링) */}
              <div
                className="text-sm leading-relaxed prose prose-sm max-w-none ai-answer"
                style={{ color: "var(--color-text-primary)" }}
                dangerouslySetInnerHTML={{ __html: renderSimpleMarkdown(analysis.answer) }}
              />

              {/* 참조 문서 */}
              {analysis.source_files.length > 0 && (
                <div className="pt-2" style={{ borderTop: "1px solid var(--color-border)" }}>
                  <span className="text-xs font-medium" style={{ color: "var(--color-text-muted)" }}>
                    참조 문서
                  </span>
                  <div className="flex flex-wrap gap-1 mt-1">
                    {analysis.source_files.map((file, i) => {
                      const fileName = file.split(/[\\/]/).pop() ?? file;
                      return (
                        <button
                          key={i}
                          onClick={() => onOpenFile?.(file)}
                          className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs hover:bg-[var(--color-bg-tertiary)] transition-colors"
                          style={{
                            color: "var(--color-accent)",
                            backgroundColor: "var(--color-bg-primary)",
                            border: "1px solid var(--color-border)",
                          }}
                          title={file}
                        >
                          <FileText className="w-3 h-3" />
                          {fileName}
                        </button>
                      );
                    })}
                  </div>
                </div>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
});

/** 간단한 마크다운 → HTML 변환 (의존성 없이) */
function renderSimpleMarkdown(md: string): string {
  return md
    // 코드 블록
    .replace(/```(\w*)\n([\s\S]*?)```/g, '<pre class="bg-[var(--color-bg-primary)] rounded p-2 text-xs overflow-x-auto my-2"><code>$2</code></pre>')
    // 인라인 코드
    .replace(/`([^`]+)`/g, '<code class="bg-[var(--color-bg-primary)] px-1 py-0.5 rounded text-xs">$1</code>')
    // 볼드
    .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
    // 이탤릭
    .replace(/\*(.+?)\*/g, "<em>$1</em>")
    // 헤딩
    .replace(/^### (.+)$/gm, '<h4 class="font-semibold mt-3 mb-1">$1</h4>')
    .replace(/^## (.+)$/gm, '<h3 class="font-semibold text-base mt-3 mb-1">$1</h3>')
    // 리스트
    .replace(/^- (.+)$/gm, '<li class="ml-4 list-disc">$1</li>')
    .replace(/^(\d+)\. (.+)$/gm, '<li class="ml-4 list-decimal">$2</li>')
    // 줄바꿈
    .replace(/\n\n/g, '<br/><br/>')
    .replace(/\n/g, '<br/>');
}
