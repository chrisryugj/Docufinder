import { memo, useCallback, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { AiAnalysis } from "../../types/search";
import { FileIcon } from "../ui/FileIcon";

interface Props {
  answer: string;
  isStreaming: boolean;
  analysis: AiAnalysis | null;
  error: string | null;
  onReset: () => void;
  currentQuestion?: string;
  onExampleClick?: (text: string) => void;
}

const EXAMPLE_CATEGORIES: { label: string; icon: string; questions: string[] }[] = [
  { label: "요약", icon: "📋", questions: ["이 문서의 핵심 내용을 요약해줘"] },
  { label: "조건·규정", icon: "📜", questions: ["계약서 해지 조건이 뭔가요?"] },
  { label: "수치·데이터", icon: "📊", questions: ["2026년 예산 총액은 얼마인가요?"] },
  { label: "일정·기한", icon: "📅", questions: ["주요 일정이나 마감 기한은?"] },
];

function basename(path: string): string {
  return path.replace(/\\/g, "/").split("/").pop() || path;
}

/** 답변 끝의 [출처: 1, 3] 패턴에서 문서 번호(0-based) 추출 + 답변 텍스트 정리 */
function parseSourceRefs(text: string): { cleanText: string; refIndices: Set<number> } {
  const match = text.match(/\[출처:\s*([\d,\s]+)\]\s*$/);
  if (!match) return { cleanText: text, refIndices: new Set() };

  const indices = match[1]
    .split(",")
    .map((s) => parseInt(s.trim(), 10) - 1) // 1-based → 0-based
    .filter((n) => !isNaN(n) && n >= 0);

  return {
    cleanText: text.slice(0, match.index).trimEnd(),
    refIndices: new Set(indices),
  };
}

function AiAnswerPanel({ answer, isStreaming, analysis, error, onReset, currentQuestion, onExampleClick }: Props) {
  const handleOpenFile = useCallback((path: string) => {
    invoke("open_file", { path }).catch(() => {});
  }, []);

  const handleOpenFolder = useCallback((path: string) => {
    invoke("open_folder", { path }).catch(() => {});
  }, []);

  // 에러 상태
  if (error) {
    return (
      <div className="flex flex-col items-center justify-center py-12 px-6 text-center">
        <div className="w-10 h-10 rounded-full bg-red-500/10 flex items-center justify-center mb-3">
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="var(--color-error, #ef4444)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <circle cx="12" cy="12" r="10" />
            <line x1="15" y1="9" x2="9" y2="15" />
            <line x1="9" y1="9" x2="15" y2="15" />
          </svg>
        </div>
        {currentQuestion && (
          <p className="text-xs text-[var(--color-text-tertiary)] mb-2">
            질문: <span className="italic">"{currentQuestion}"</span>
          </p>
        )}
        <p className="text-sm text-[var(--color-text-secondary)] mb-3 max-w-md">{error}</p>
        <button onClick={onReset} className="text-xs text-[var(--color-accent)] hover:underline">
          초기화
        </button>
      </div>
    );
  }

  // 대기 상태 (아직 질문 안 함)
  if (!answer && !isStreaming && !analysis) {
    return (
      <div className="flex flex-col items-center justify-center py-8 px-6 text-center">
        {/* 브랜드 아이콘 */}
        <div
          className="w-12 h-12 rounded-2xl flex items-center justify-center mb-4"
          style={{ background: "linear-gradient(135deg, #0d9488 0%, #14b8a6 100%)" }}
        >
          <svg width="22" height="22" viewBox="0 0 24 24" fill="white" stroke="none">
            <path d="M12 2l2.4 6.4L21 11l-6.6 2.4L12 21l-2.4-7.6L3 11l6.6-2.4L12 2z" />
          </svg>
        </div>

        {/* 타이틀 + 설명 */}
        <h3 className="text-[15px] font-semibold text-[var(--color-text-primary)] mb-1">
          Anything
        </h3>
        <p className="text-xs text-[var(--color-text-muted)] mb-1.5 max-w-xs leading-relaxed">
          인덱싱된 문서를 분석하여 질문에 답변합니다
        </p>
        <p className="text-[10px] text-[var(--color-text-tertiary)] mb-5">
          Enter로 전송 · Shift+Enter로 줄바꿈
        </p>

        {/* 카테고리별 예시 질문 */}
        {onExampleClick && (
          <div className="w-full max-w-sm space-y-1.5">
            {EXAMPLE_CATEGORIES.map((cat) => (
              <button
                key={cat.label}
                onClick={() => onExampleClick(cat.questions[0])}
                className="w-full flex items-center gap-3 px-3.5 py-2.5 rounded-lg text-left transition-all duration-150 hover:scale-[1.01] active:scale-[0.99]"
                style={{
                  backgroundColor: "var(--color-bg-secondary)",
                  border: "1px solid var(--color-border)",
                }}
              >
                <span className="text-base shrink-0">{cat.icon}</span>
                <div className="flex-1 min-w-0">
                  <span className="text-[10px] font-medium text-[var(--color-text-tertiary)] uppercase tracking-wider">
                    {cat.label}
                  </span>
                  <p className="text-xs text-[var(--color-text-secondary)] truncate">
                    {cat.questions[0]}
                  </p>
                </div>
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--color-text-muted)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="shrink-0 opacity-40">
                  <path d="M5 12h14M12 5l7 7-7 7" />
                </svg>
              </button>
            ))}
          </div>
        )}
      </div>
    );
  }

  const { cleanText, refIndices } = !isStreaming ? parseSourceRefs(answer) : { cleanText: answer, refIndices: new Set<number>() };

  return (
    <div className="flex flex-col h-full overflow-y-auto px-4 py-4 gap-3">
      {/* 질문 영역 */}
      {currentQuestion && (
        <div
          className="flex items-start gap-2.5 px-3.5 py-3 rounded-lg"
          style={{ backgroundColor: "var(--color-bg-tertiary)" }}
        >
          <div
            className="w-5 h-5 rounded-full shrink-0 flex items-center justify-center mt-0.5"
            style={{ backgroundColor: "var(--color-text-muted)", opacity: 0.2 }}
          >
            <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="var(--color-text-secondary)" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
              <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
              <circle cx="12" cy="7" r="4" />
            </svg>
          </div>
          <p className="text-[13px] text-[var(--color-text-primary)] leading-relaxed" title={currentQuestion}>
            {currentQuestion}
          </p>
        </div>
      )}

      {/* 답변 영역 */}
      <div
        className="flex-1 rounded-lg px-3.5 py-3"
        style={{
          backgroundColor: "var(--color-bg-secondary)",
          border: "1px solid var(--color-border)",
        }}
      >
        {/* 답변 라벨 */}
        <div className="flex items-center gap-2 mb-2.5">
          <div
            className="w-5 h-5 rounded-full shrink-0 flex items-center justify-center"
            style={{ background: "linear-gradient(135deg, #0d9488 0%, #14b8a6 100%)" }}
          >
            <svg width="10" height="10" viewBox="0 0 24 24" fill="white" stroke="none">
              <path d="M12 2l2.4 6.4L21 11l-6.6 2.4L12 21l-2.4-7.6L3 11l6.6-2.4L12 2z" />
            </svg>
          </div>
          <span className="text-[11px] font-medium" style={{ color: "#0d9488" }}>
            AI 문서 분석 결과
          </span>
          {isStreaming && (
            <span className="text-[10px] animate-pulse" style={{ color: "#0d9488" }}>분석 중...</span>
          )}
          {analysis && (
            <span className="text-[10px] text-[var(--color-text-tertiary)] ml-auto tabular-nums">
              {(analysis.processing_time_ms / 1000).toFixed(1)}초
            </span>
          )}
        </div>

        {/* 답변 본문 (마크다운 렌더링) */}
        <div className="text-[13px] text-[var(--color-text-primary)] leading-[1.8] break-words ai-answer-prose">
          {isStreaming ? (
            // 스트리밍 중: pre-wrap (마크다운 미완성 상태)
            <span className="whitespace-pre-wrap" aria-live="polite" role="status">
              {cleanText}
              <span
                className="inline-block w-1.5 h-3.5 rounded-sm animate-pulse ml-0.5 align-text-bottom"
                style={{ backgroundColor: "#0d9488" }}
              />
            </span>
          ) : (
            // 완료: 마크다운 렌더링
            <ReactMarkdown remarkPlugins={[remarkGfm]}>{cleanText}</ReactMarkdown>
          )}
        </div>
      </div>

      {/* 참조 문서 */}
      {analysis && analysis.source_files.length > 0 && (
        <div className="space-y-1.5">
          <p className="text-[10px] font-medium text-[var(--color-text-tertiary)] px-1">
            참조 문서 {refIndices.size > 0 && <span className="font-normal">· 근거 {refIndices.size}건</span>}
          </p>
          {(() => {
            const hasRefs = refIndices.size > 0;
            const sorted = analysis.source_files
              .map((path, i) => ({ path, i, isRef: refIndices.has(i) }))
              .sort((a, b) => (a.isRef === b.isRef ? 0 : a.isRef ? -1 : 1));

            return (
              <div className="flex flex-col gap-0.5">
                {sorted.map(({ path, isRef }) => {
                  const name = basename(path);
                  return (
                    <div
                      key={path}
                      className={`flex items-center gap-2.5 px-3 py-2 rounded-lg group cursor-pointer transition-all ${
                        hasRefs && !isRef ? "opacity-40" : ""
                      }`}
                      style={{
                        backgroundColor: isRef ? "var(--color-bg-secondary)" : "transparent",
                        border: isRef ? "1px solid var(--color-border)" : "1px solid transparent",
                      }}
                      onClick={() => handleOpenFile(path)}
                      title={path}
                    >
                      <FileIcon fileName={name} size="sm" />
                      <span className="text-[12px] text-[var(--color-text-secondary)] truncate flex-1">
                        {name}
                      </span>
                      {isRef && (
                        <span
                          className="text-[9px] font-medium px-1.5 py-0.5 rounded shrink-0"
                          style={{ backgroundColor: "rgba(13,148,136,0.1)", color: "#0d9488" }}
                        >
                          근거
                        </span>
                      )}
                      <button
                        onClick={(e) => { e.stopPropagation(); handleOpenFolder(path); }}
                        className="text-[var(--color-text-tertiary)] hover:text-[var(--color-accent)] opacity-0 group-hover:opacity-100 transition-opacity shrink-0"
                        title="폴더 열기"
                      >
                        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                          <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
                        </svg>
                      </button>
                    </div>
                  );
                })}
              </div>
            );
          })()}
        </div>
      )}

      {/* 하단 액션 바 */}
      {(analysis || error) && (
        <CopyableActionBar answer={answer} analysis={analysis} onReset={onReset} />

      )}
    </div>
  );
}

/** 하단 액션 바 — 새 질문 + 복사 버튼 */
function CopyableActionBar({ answer, analysis, onReset }: { answer: string; analysis: AiAnalysis | null; onReset: () => void }) {
  const [copied, setCopied] = useState(false);

  const handleCopy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(answer);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // clipboard API 미지원 시 fallback
      const textarea = document.createElement("textarea");
      textarea.value = answer;
      textarea.style.position = "fixed";
      textarea.style.opacity = "0";
      document.body.appendChild(textarea);
      textarea.select();
      document.execCommand("copy");
      document.body.removeChild(textarea);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  }, [answer]);

  return (
    <div className="flex items-center justify-between pt-1">
      <div className="flex items-center gap-1">
        <button
          onClick={onReset}
          className="flex items-center gap-1.5 px-3 py-1.5 text-[11px] font-medium rounded-md transition-colors hover:bg-[var(--color-bg-tertiary)]"
          style={{ color: "var(--color-text-muted)" }}
        >
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <polyline points="1 4 1 10 7 10" />
            <path d="M3.51 15a9 9 0 1 0 2.13-9.36L1 10" />
          </svg>
          새 질문
        </button>
        {answer && (
          <button
            onClick={handleCopy}
            className="flex items-center gap-1.5 px-3 py-1.5 text-[11px] font-medium rounded-md transition-colors hover:bg-[var(--color-bg-tertiary)]"
            style={{ color: copied ? "var(--color-success)" : "var(--color-text-muted)" }}
            aria-label="AI 답변 복사"
          >
            {copied ? (
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <polyline points="20 6 9 17 4 12" />
              </svg>
            ) : (
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
              </svg>
            )}
            {copied ? "복사됨" : "복사"}
          </button>
        )}
      </div>
      {analysis && (
        <span className="text-[10px] text-[var(--color-text-tertiary)] tabular-nums">
          {analysis.model}
          {analysis.tokens_used && ` · ${analysis.tokens_used.total_tokens}t`}
        </span>
      )}
    </div>
  );
}

export default memo(AiAnswerPanel);
