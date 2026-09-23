import { useCallback, useEffect, useMemo, useRef, useState, type ComponentProps } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ChevronDown, ChevronUp, Sparkles, X } from "lucide-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import rehypeKatex from "rehype-katex";
import type { AiAnalysis } from "../../../types/search";
import { getErrorMessage } from "../../../types/error";
import { FileQaSection } from "./FileQaSection";

export type SummaryType = "brief" | "structured" | "keywords";

// 렌더마다 새 배열을 만들면 ReactMarkdown 이 매번 다시 파싱한다
const SUMMARY_REMARK: ComponentProps<typeof ReactMarkdown>["remarkPlugins"] = [[remarkGfm, { singleTilde: false }], remarkMath];
const SUMMARY_REHYPE: ComponentProps<typeof ReactMarkdown>["rehypePlugins"] = [rehypeKatex];

export const SUMMARY_TYPE_LABELS: Record<SummaryType, string> = {
  brief: "핵심 3줄",
  structured: "항목별 정리",
  keywords: "핵심 키워드",
};

/** 문서 AI 요약 상태. 파일이 바뀌면 진행 중 응답을 무효화하고 초기화한다. */
export function useAiSummary(filePath: string | null) {
  const [aiSummary, setAiSummary] = useState<AiAnalysis | null>(null);
  const [summaryLoading, setSummaryLoading] = useState(false);
  const [summaryError, setSummaryError] = useState<string | null>(null);
  const [summaryExpanded, setSummaryExpanded] = useState(true);
  const [summaryType, setSummaryType] = useState<SummaryType>("brief");
  const [showSummaryMenu, setShowSummaryMenu] = useState(false);
  const summaryRequestId = useRef(0);

  // 파일이 바뀌면 진행 중 요청을 무효화한다. 무효화된 요청의 finally 는 스피너를 끄지 않으므로
  // 여기서 같이 끈다 (종전엔 요약 중 파일을 바꾸면 스피너가 계속 돌았다).
  useEffect(() => {
    summaryRequestId.current++;
    setAiSummary(null);
    setSummaryError(null);
    setSummaryLoading(false);
    setShowSummaryMenu(false);
  }, [filePath]);

  const generate = useCallback((type: SummaryType) => {
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
        if (summaryRequestId.current === reqId) setSummaryError(getErrorMessage(e));
      })
      .finally(() => {
        if (summaryRequestId.current === reqId) setSummaryLoading(false);
      });
  }, [filePath, summaryLoading]);

  return {
    aiSummary, summaryLoading, summaryError, summaryExpanded, setSummaryExpanded,
    summaryType, showSummaryMenu, setShowSummaryMenu, generate,
  };
}

export type AiSummaryState = ReturnType<typeof useAiSummary>;

/** 요약 유형 선택 줄 */
export function SummaryMenu({ summary }: { summary: AiSummaryState }) {
  const { summaryType, aiSummary, setShowSummaryMenu, generate } = summary;
  return (
    <div className="flex items-center gap-1.5 px-3 py-2 border-b" style={{ borderColor: "var(--color-border)", backgroundColor: "var(--color-bg-secondary)" }}>
      <span className="text-2xs text-[var(--color-text-muted)] shrink-0">요약 유형</span>
      {(["brief", "structured", "keywords"] as SummaryType[]).map((type) => (
        <button
          key={type}
          onClick={() => { setShowSummaryMenu(false); generate(type); }}
          className="px-2 py-0.5 rounded-lg text-2xs transition-colors"
          style={{
            backgroundColor: summaryType === type && aiSummary ? "var(--color-accent-light)" : "var(--color-bg-tertiary)",
            color: summaryType === type && aiSummary ? "var(--color-accent)" : "var(--color-text-secondary)",
            border: "1px solid var(--color-border)",
          }}
        >
          {SUMMARY_TYPE_LABELS[type]}
        </button>
      ))}
      <button
        onClick={() => setShowSummaryMenu(false)}
        className="ml-auto text-[var(--color-text-muted)] hover:text-[var(--color-text-primary)] p-0.5 rounded-lg"
        aria-label="요약 유형 닫기"
      >
        <X size={12} />
      </button>
    </div>
  );
}

interface AiSectionProps {
  summary: AiSummaryState;
  showFileQa: boolean;
  filePath: string;
  markdownComponents: ComponentProps<typeof ReactMarkdown>["components"];
}

/** AI 섹션 (요약 + 파일 질문): 스크롤 밖 고정 영역 */
export function AiSection({ summary, showFileQa, filePath, markdownComponents }: AiSectionProps) {
  const { summaryLoading, summaryError, aiSummary, summaryType, summaryExpanded, setSummaryExpanded, generate } = summary;
  // 찾기 바 입력·토스트 등 부모 리렌더마다 요약 마크다운을 다시 파싱하지 않게
  const summaryNode = useMemo(
    () => aiSummary ? (
      <ReactMarkdown remarkPlugins={SUMMARY_REMARK} rehypePlugins={SUMMARY_REHYPE} components={markdownComponents}>
        {aiSummary.answer}
      </ReactMarkdown>
    ) : null,
    [aiSummary, markdownComponents],
  );
  return (
    <div className="preview-ai-section border-b overflow-hidden ai-section-enter shrink-0" style={{ borderColor: "var(--color-accent-border)" }}>
      {summaryLoading && (
        <div className="flex items-center gap-2 px-3 py-2.5 text-xs" style={{ color: "var(--color-accent)" }}>
          <div className="w-3 h-3 border border-current border-t-transparent rounded-full animate-spin shrink-0" />
          <span>{SUMMARY_TYPE_LABELS[summaryType]} 요약을 만드는 중</span>
        </div>
      )}

      {summaryError && !summaryLoading && (
        <div className="px-3 py-2.5" role="alert">
          <div className="flex items-center gap-1.5 text-xs mb-1" style={{ color: "var(--color-error)" }}>
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
            AI 요약 실패
          </div>
          <p className="text-2xs text-[var(--color-text-secondary)]">{summaryError}</p>
          <button
            onClick={() => generate(summaryType)}
            className="mt-1.5 text-2xs text-[var(--color-accent)] hover:underline"
          >
            다시 시도
          </button>
        </div>
      )}

      {aiSummary && !summaryLoading && (
        <>
          <button
            onClick={() => setSummaryExpanded(!summaryExpanded)}
            className="flex items-center gap-2 w-full px-3 py-2 text-xs font-medium"
            style={{ color: "var(--color-accent)" }}
            aria-expanded={summaryExpanded}
          >
            <Sparkles size={12} />
            AI 요약 · {SUMMARY_TYPE_LABELS[summaryType]}
            <span className="ml-auto text-[var(--color-text-muted)] font-normal">
              {(aiSummary.processing_time_ms / 1000).toFixed(1)}초
              {aiSummary.tokens_used && ` · 토큰 ${aiSummary.tokens_used.total_tokens.toLocaleString()}`}
            </span>
            {summaryExpanded ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
          </button>
          {summaryExpanded && (
            <div className="px-3 pb-3 text-sm leading-relaxed text-[var(--color-text-primary)] doc-preview summary-inline" style={{ backgroundColor: "var(--color-bg-primary)" }}>
              {summaryNode}
            </div>
          )}
        </>
      )}

      {/* 파일 질문 섹션 (별도 컴포넌트 — 입력 시 부모 리렌더 방지) */}
      {showFileQa && <FileQaSection filePath={filePath} />}
    </div>
  );
}
