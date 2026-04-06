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
}

/** 파일 경로에서 파일명 추출 */
function basename(path: string): string {
  return path.replace(/\\/g, "/").split("/").pop() || path;
}

/** AI 답변 패널 — 스트리밍 마크다운 + 출처 */
function AiAnswerPanel({ answer, isStreaming, analysis, error, onReset }: Props) {
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
        <p className="text-sm text-[var(--color-text-secondary)] mb-3 max-w-md">{error}</p>
        <button
          onClick={onReset}
          className="text-xs text-[var(--color-accent)] hover:underline"
        >
          다시 시도
        </button>
      </div>
    );
  }

  // 대기 상태 (아직 질문 안 함)
  if (!answer && !isStreaming && !analysis) {
    return (
      <div className="flex flex-col items-center justify-center py-16 px-6 text-center">
        <div className="w-12 h-12 rounded-full bg-[var(--color-accent)]/10 flex items-center justify-center mb-4">
          <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="var(--color-accent)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3" />
            <circle cx="12" cy="12" r="10" />
            <line x1="12" y1="17" x2="12.01" y2="17" />
          </svg>
        </div>
        <p className="text-sm font-medium text-[var(--color-text-primary)] mb-1">
          문서에 대해 질문하세요
        </p>
        <p className="text-xs text-[var(--color-text-tertiary)] max-w-sm">
          인덱싱된 문서를 기반으로 AI가 답변합니다.
          Enter를 눌러 질문을 보내세요.
        </p>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full overflow-y-auto">
      {/* AI 답변 영역 */}
      <div className="flex-1 px-4 py-3">
        {/* 헤더 */}
        <div className="flex items-center gap-2 mb-3">
          <div className="w-5 h-5 rounded bg-[var(--color-accent)]/15 flex items-center justify-center">
            <svg width="12" height="12" viewBox="0 0 24 24" fill="var(--color-accent)" stroke="none">
              <path d="M12 2L15.09 8.26L22 9.27L17 14.14L18.18 21.02L12 17.77L5.82 21.02L7 14.14L2 9.27L8.91 8.26L12 2Z" />
            </svg>
          </div>
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

        {/* 마크다운 답변 (간단한 렌더링) */}
        <div className="text-sm text-[var(--color-text-primary)] leading-relaxed whitespace-pre-wrap break-words ai-answer-content">
          {answer}
          {isStreaming && <span className="inline-block w-1.5 h-4 bg-[var(--color-accent)] animate-pulse ml-0.5 align-text-bottom rounded-sm" />}
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
                  <span className="text-xs text-[var(--color-text-secondary)] truncate flex-1">
                    {name}
                  </span>
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      handleOpenFolder(path);
                    }}
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
            초기화
          </button>
          {analysis && (
            <span className="text-[10px] text-[var(--color-text-tertiary)]">
              {analysis.model}
            </span>
          )}
        </div>
      )}
    </div>
  );
}

export default memo(AiAnswerPanel);
