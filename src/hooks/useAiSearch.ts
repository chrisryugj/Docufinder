import { useState, useCallback, useRef } from "react";
import { invokeWithTimeout } from "../utils/invokeWithTimeout";
import { logToBackend } from "../utils/errorLogger";
import type { SearchResult, AiAnalysis } from "../types/search";

const AI_TIMEOUT = 60_000; // 60초

interface UseAiSearchReturn {
  /** AI 분석 결과 */
  aiAnalysis: AiAnalysis | null;
  /** 로딩 상태 */
  isAiLoading: boolean;
  /** 에러 메시지 */
  aiError: string | null;
  /** AI 분석 요청 */
  requestAiAnalysis: (query: string, results: SearchResult[]) => Promise<void>;
  /** AI 분석 결과 초기화 */
  clearAiAnalysis: () => void;
}

export function useAiSearch(): UseAiSearchReturn {
  const [aiAnalysis, setAiAnalysis] = useState<AiAnalysis | null>(null);
  const [isAiLoading, setIsAiLoading] = useState(false);
  const [aiError, setAiError] = useState<string | null>(null);
  const abortRef = useRef(0); // 동시 호출 방지용 카운터

  const requestAiAnalysis = useCallback(
    async (query: string, results: SearchResult[]) => {
      if (!query.trim() || results.length === 0) return;

      const callId = ++abortRef.current;
      setIsAiLoading(true);
      setAiError(null);

      try {
        const analysis = await invokeWithTimeout<AiAnalysis>(
          "ask_ai",
          {
            query,
            search_results: results.slice(0, 5),
          },
          AI_TIMEOUT
        );

        // 동시 호출 시 마지막 요청만 반영
        if (callId === abortRef.current) {
          setAiAnalysis(analysis);
        }
      } catch (err) {
        if (callId === abortRef.current) {
          const msg = err instanceof Error ? err.message : String(err);
          setAiError(msg);
          logToBackend("warn", `AI analysis failed: ${msg}`);
        }
      } finally {
        if (callId === abortRef.current) {
          setIsAiLoading(false);
        }
      }
    },
    []
  );

  const clearAiAnalysis = useCallback(() => {
    setAiAnalysis(null);
    setAiError(null);
    abortRef.current++;
  }, []);

  return {
    aiAnalysis,
    isAiLoading,
    aiError,
    requestAiAnalysis,
    clearAiAnalysis,
  };
}
