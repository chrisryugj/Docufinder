import { memo, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
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

const EXAMPLE_QUESTIONS = [
  "이 문서의 핵심 내용을 요약해줘",
  "계약서 해지 조건이 뭔가요?",
  "2026년 예산 총액은 얼마인가요?",
  "주요 일정이나 마감 기한은?",
];

function basename(path: string): string {
  return path.replace(/\\/g, "/").split("/").pop() || path;
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
      <div className="flex flex-col items-center justify-center py-10 px-6 text-center">
        <div
          className="w-11 h-11 rounded-full flex items-center justify-center mb-3"
          style={{ backgroundColor: "var(--color-accent-light)" }}
        >
          <svg width="20" height="20" viewBox="0 0 24 24" fill="var(--color-accent)" stroke="none">
            <path d="M12 2l2.4 6.4L21 11l-6.6 2.4L12 21l-2.4-7.6L3 11l6.6-2.4L12 2z" />
          </svg>
        </div>
        <p className="text-sm font-medium text-[var(--color-text-primary)] mb-1">
          인덱싱된 문서에 대해 질문하세요
        </p>
        <p className="text-xs text-[var(--color-text-tertiary)] mb-5">
          Enter로 전송 · Shift+Enter로 줄바꿈
        </p>

        {/* 예시 질문 칩 */}
        {onExampleClick && (
          <div className="flex flex-wrap gap-2 justify-center max-w-sm">
            {EXAMPLE_QUESTIONS.map((q) => (
              <button
                key={q}
                onClick={() => onExampleClick(q)}
                className="px-3 py-1.5 rounded-full text-xs transition-all duration-150 hover:opacity-80 active:scale-95"
                style={{
                  backgroundColor: "var(--color-bg-tertiary)",
                  border: "1px solid var(--color-border)",
                  color: "var(--color-text-secondary)",
                }}
              >
                {q}
              </button>
            ))}
          </div>
        )}
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full overflow-y-auto">
      {/* AI 답변 영역 */}
      <div className="flex-1 px-4 py-3">
        {/* 헤더 */}
        <div className="flex items-start gap-2 mb-3">
          <div className="w-5 h-5 rounded shrink-0 flex items-center justify-center mt-0.5" style={{ backgroundColor: "var(--color-accent-light)" }}>
            <svg width="11" height="11" viewBox="0 0 24 24" fill="var(--color-accent)" stroke="none">
              <path d="M12 2l2.4 6.4L21 11l-6.6 2.4L12 21l-2.4-7.6L3 11l6.6-2.4L12 2z" />
            </svg>
          </div>
          <div className="flex-1 min-w-0">
            {currentQuestion && (
              <p className="text-xs text-[var(--color-text-muted)] truncate mb-0.5" title={currentQuestion}>
                Q. {currentQuestion}
              </p>
            )}
            <div className="flex items-center gap-2">
              <span className="text-xs font-medium text-[var(--color-text-secondary)]">AI 답변</span>
              {isStreaming && (
                <span className="text-[10px] text-[var(--color-accent)] animate-pulse">생성 중...</span>
              )}
              {analysis && (
                <span className="text-[10px] text-[var(--color-text-tertiary)] ml-auto">
                  {(analysis.processing_time_ms / 1000).toFixed(1)}초
                  {analysis.tokens_used && ` · ${analysis.tokens_used.total_tokens} tokens`}
                </span>
              )}
            </div>
          </div>
        </div>

        {/* 답변 텍스트 */}
        <div className="text-sm text-[var(--color-text-primary)] leading-relaxed whitespace-pre-wrap break-words ai-answer-content">
          {answer}
          {isStreaming && (
            <span className="inline-block w-1.5 h-4 bg-[var(--color-accent)] animate-pulse ml-0.5 align-text-bottom rounded-sm" />
          )}
        </div>
      </div>

      {/* 출처 파일 */}
      {analysis && analysis.source_files.length > 0 && (
        <div className="border-t border-[var(--color-border)] px-4 py-3">
          <p className="text-[10px] font-medium text-[var(--color-text-tertiary)] uppercase tracking-wider mb-2">
            참조 문서
          </p>
          <div className="flex flex-col gap-1">
            {analysis.source_files.map((path) => {
              const name = basename(path);
              return (
                <div
                  key={path}
                  className="flex items-center gap-2 px-2 py-1.5 rounded hover:bg-[var(--color-bg-tertiary)] group cursor-pointer transition-colors"
                  onClick={() => handleOpenFile(path)}
                  title={path}
                >
                  <FileIcon fileName={name} size="sm" />
                  <span className="text-xs text-[var(--color-text-secondary)] truncate flex-1">{name}</span>
                  <button
                    onClick={(e) => { e.stopPropagation(); handleOpenFolder(path); }}
                    className="text-[10px] text-[var(--color-text-tertiary)] hover:text-[var(--color-accent)] opacity-0 group-hover:opacity-100 transition-opacity"
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
        </div>
      )}

      {/* 하단 액션 */}
      {(analysis || error) && (
        <div className="border-t border-[var(--color-border)] px-4 py-2 flex items-center justify-between">
          <button
            onClick={onReset}
            className="text-xs text-[var(--color-text-tertiary)] hover:text-[var(--color-text-secondary)] transition-colors"
          >
            새 질문
          </button>
          {analysis && (
            <span className="text-[10px] text-[var(--color-text-tertiary)]">{analysis.model}</span>
          )}
        </div>
      )}
    </div>
  );
}

export default memo(AiAnswerPanel);
