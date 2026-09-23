import { memo, useEffect, useState, useRef, useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { MessageSquare, Sparkles } from "lucide-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import rehypeKatex from "rehype-katex";
import type { AiAnalysis } from "../../../types/search";
import { getErrorMessage } from "../../../types/error";
import type { ComponentProps } from "react";

const QA_REMARK: ComponentProps<typeof ReactMarkdown>["remarkPlugins"] = [[remarkGfm, { singleTilde: false }], remarkMath];
const QA_REHYPE: ComponentProps<typeof ReactMarkdown>["rehypePlugins"] = [rehypeKatex];

// ─── FileQaSection (격리 컴포넌트 — 입력 시 부모 리렌더 방지) ──

interface FileQaSectionProps {
  filePath: string;
}

export const FileQaSection = memo(function FileQaSection({ filePath }: FileQaSectionProps) {
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
      setError(getErrorMessage(e));
      setLoading(false);
    });
  }, [filePath, question, loading]);

  const hasAnswer = answer || loading;
  // 질문을 타이핑할 때마다(이 컴포넌트 리렌더) 완성된 답변을 다시 파싱하지 않게
  const answerNode = useMemo(
    () => loading ? null : (
      <ReactMarkdown remarkPlugins={QA_REMARK} rehypePlugins={QA_REHYPE}>
        {answer}
      </ReactMarkdown>
    ),
    [answer, loading],
  );

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
          <MessageSquare size={10} color="var(--color-on-accent-ai)" aria-hidden="true" />
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
          placeholder="이 파일에 대해 질문"
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
              style={{ backgroundColor: "var(--color-accent-ai)", color: "var(--color-on-accent-ai)" }}
              title="보내기 (Enter)"
              aria-label="질문 보내기"
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
        <div className="px-3 py-2 text-2xs flex items-start gap-1.5" style={{ color: "var(--color-error)", backgroundColor: "color-mix(in srgb, var(--color-error) 6%, transparent)" }}>
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
            <span className="text-2xs font-semibold" style={{ color: "var(--color-accent-ai)" }}>
              답변
            </span>
            {loading && (
              <span className="text-2xs animate-pulse" style={{ color: "var(--color-accent-ai)" }}>답변 쓰는 중</span>
            )}
            {analysis && (
              <span className="text-2xs text-[var(--color-text-muted)] ml-auto tabular-nums">
                {(analysis.processing_time_ms / 1000).toFixed(1)}초
                {analysis.tokens_used && ` · 토큰 ${analysis.tokens_used.total_tokens.toLocaleString()}`}
              </span>
            )}
          </div>

          {/* 답변 본문 */}
          {loading ? (
            <div className="text-sm leading-[1.8] text-[var(--color-text-primary)] whitespace-pre-wrap break-words">
              {answer || <span className="text-[var(--color-text-muted)]">문서를 읽는 중</span>}
              {answer && (
                <span
                  className="inline-block w-1.5 h-3.5 rounded-sm animate-pulse ml-0.5 align-text-bottom"
                  style={{ backgroundColor: "var(--color-accent-ai)" }}
                />
              )}
            </div>
          ) : (
            <div className="text-sm leading-[1.8] text-[var(--color-text-primary)] doc-preview summary-inline ai-answer-prose">
              {answerNode}
            </div>
          )}

          {/* 메타 / 초기화 */}
          {analysis && (
            <div className="mt-3 pt-2 border-t flex items-center" style={{ borderColor: "var(--color-border)" }}>
              <button
                onClick={() => { setAnswer(""); setAnalysis(null); setError(null); setQuestion(""); }}
                className="text-2xs px-2 py-0.5 rounded-lg transition-colors hover:bg-[var(--color-bg-tertiary)]"
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
