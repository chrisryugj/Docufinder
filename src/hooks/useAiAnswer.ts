import { useState, useCallback, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AiAnalysis } from "../types/search";
import { IS_LITE } from "../utils/buildFlavor";

interface AiTokenEvent {
  request_id: string;
  token: string;
}

interface AiCompleteEvent extends AiAnalysis {
  request_id: string;
}

interface AiErrorEvent {
  request_id: string;
  error: string;
}

export interface UseAiAnswerReturn {
  answer: string;
  isStreaming: boolean;
  analysis: AiAnalysis | null;
  error: string | null;
  askedQuery: string;
  ask: (query: string, folderScope?: string | null) => void;
  reset: () => void;
}

export function useAiAnswer(): UseAiAnswerReturn {
  const [answer, setAnswer] = useState("");
  const [isStreaming, setIsStreaming] = useState(false);
  const [analysis, setAnalysis] = useState<AiAnalysis | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [askedQuery, setAskedQuery] = useState("");
  const unlistenRefs = useRef<UnlistenFn[]>([]);
  const requestIdRef = useRef("");

  // Tauri event 리스너 등록 (StrictMode 중복 방지: cancelled flag)
  useEffect(() => {
    let cancelled = false;

    const setup = async () => {
      const u1 = await listen<AiTokenEvent>("ai-token", (e) => {
        if (cancelled) return;
        if (e.payload.request_id !== requestIdRef.current) return;
        setAnswer((prev) => prev + e.payload.token);
      });
      const u2 = await listen<AiCompleteEvent>("ai-complete", (e) => {
        if (cancelled) return;
        if (e.payload.request_id !== requestIdRef.current) return;
        const { request_id: _, ...analysis } = e.payload;
        setAnalysis(analysis as AiAnalysis);
        setIsStreaming(false);
      });
      const u3 = await listen<AiErrorEvent>("ai-error", (e) => {
        if (cancelled) return;
        if (e.payload.request_id !== requestIdRef.current) return;
        setError(e.payload.error);
        setIsStreaming(false);
      });

      // setup 완료 시점에 이미 unmount 됐으면 즉시 해제
      if (cancelled) {
        u1(); u2(); u3();
      } else {
        unlistenRefs.current = [u1, u2, u3];
      }
    };
    setup();
    return () => {
      cancelled = true;
      unlistenRefs.current.forEach((fn) => fn());
      unlistenRefs.current = [];
    };
  }, []);

  const ask = useCallback((query: string, folderScope?: string | null) => {
    // lite: ask_ai 가 항상 Err — UI 진입점은 이미 숨겼지만, 혹시 남은 경로로 들어와도
    // IPC 를 쏘지 않고 백엔드와 같은 문구를 그대로 보여준다.
    if (IS_LITE) {
      setAskedQuery(query);
      setAnswer("");
      setAnalysis(null);
      setIsStreaming(false);
      setError("이 설치본(내부망 전용)에는 AI 기능이 포함되어 있지 않습니다");
      return;
    }

    const requestId = crypto.randomUUID();
    requestIdRef.current = requestId;
    setAskedQuery(query);
    setAnswer("");
    setAnalysis(null);
    setError(null);
    setIsStreaming(true);
    invoke("ask_ai", {
      query,
      folderScope: folderScope ?? null,
      requestId,
    }).catch((e) => {
      const msg = typeof e === "object" && e?.message ? e.message : String(e);
      setError(msg);
      setIsStreaming(false);
    });
  }, []);

  const reset = useCallback(() => {
    requestIdRef.current = "";
    setAskedQuery("");
    setAnswer("");
    setAnalysis(null);
    setError(null);
    setIsStreaming(false);
  }, []);

  return { answer, isStreaming, analysis, error, askedQuery, ask, reset };
}
