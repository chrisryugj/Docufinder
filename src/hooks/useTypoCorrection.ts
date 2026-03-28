import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";

interface SuggestedWord {
  word: string;
  distance: number;
  frequency: number;
}

interface CorrectionSuggestion {
  original: string;
  suggestions: SuggestedWord[];
}

export function useTypoCorrection(query: string, enabled: boolean) {
  const [suggestion, setSuggestion] = useState<CorrectionSuggestion | null>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout>>(undefined);
  const lastQueryRef = useRef("");

  useEffect(() => {
    if (!enabled || !query.trim() || query.trim().length < 2) {
      setSuggestion(null);
      return;
    }

    // 같은 쿼리면 무시
    if (query.trim() === lastQueryRef.current) return;

    clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(async () => {
      try {
        lastQueryRef.current = query.trim();
        const result = await invoke<CorrectionSuggestion>("suggest_correction", {
          query: query.trim(),
        });
        // 제안이 있을 때만 표시
        if (result.suggestions.length > 0) {
          setSuggestion(result);
        } else {
          setSuggestion(null);
        }
      } catch {
        setSuggestion(null);
      }
    }, 500); // 검색 후 0.5초 대기

    return () => clearTimeout(debounceRef.current);
  }, [query, enabled]);

  const dismiss = () => setSuggestion(null);

  return { suggestion, dismiss };
}
