import { useState, useCallback, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AiAnalysis } from "../types/search";

export interface UseAiAnswerReturn {
  answer: string;
  isStreaming: boolean;
  analysis: AiAnalysis | null;
  error: string | null;
  ask: (query: string, folderScope?: string | null) => void;
  reset: () => void;
}

export function useAiAnswer(): UseAiAnswerReturn {
  const [answer, setAnswer] = useState("");
  const [isStreaming, setIsStreaming] = useState(false);
  const [analysis, setAnalysis] = useState<AiAnalysis | null>(null);
  const [error, setError] = useState<string | null>(null);
  const unlistenRefs = useRef<UnlistenFn[]>([]);

  // Tauri event 리스너 등록
  useEffect(() => {
    const setup = async () => {
      const u1 = await listen<string>("ai-token", (e) => {
        setAnswer((prev) => prev + e.payload);
      });
      const u2 = await listen<AiAnalysis>("ai-complete", (e) => {
        setAnalysis(e.payload);
        setIsStreaming(false);
      });
      const u3 = await listen<string>("ai-error", (e) => {
        setError(e.payload);
        setIsStreaming(false);
      });
      unlistenRefs.current = [u1, u2, u3];
    };
    setup();
    return () => {
      unlistenRefs.current.forEach((fn) => fn());
      unlistenRefs.current = [];
    };
  }, []);

  const ask = useCallback((query: string, folderScope?: string | null) => {
    setAnswer("");
    setAnalysis(null);
    setError(null);
    setIsStreaming(true);
    invoke("ask_ai", {
      query,
      folderScope: folderScope ?? null,
    }).catch((e) => {
      const msg = typeof e === "object" && e?.message ? e.message : String(e);
      setError(msg);
      setIsStreaming(false);
    });
  }, []);

  const reset = useCallback(() => {
    setAnswer("");
    setAnalysis(null);
    setError(null);
    setIsStreaming(false);
  }, []);

  return { answer, isStreaming, analysis, error, ask, reset };
}
