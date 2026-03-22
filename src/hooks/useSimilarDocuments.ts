import { useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { SearchResult } from "../types/search";

type ShowToastFn = (message: string, type: "success" | "error" | "loading" | "info", duration?: number) => string;

interface UseSimilarDocumentsReturn {
  similarResults: SearchResult[];
  similarSourceFile: string | null;
  handleFindSimilar: (filePath: string) => Promise<void>;
  clearSimilarResults: () => void;
}

/**
 * 유사 문서 검색 (벡터 기반)
 */
export function useSimilarDocuments(showToast: ShowToastFn): UseSimilarDocumentsReturn {
  const [similarResults, setSimilarResults] = useState<SearchResult[]>([]);
  const [similarSourceFile, setSimilarSourceFile] = useState<string | null>(null);

  const handleFindSimilar = useCallback(async (filePath: string) => {
    try {
      showToast("유사 문서 검색 중...", "info");
      const response = await invoke<{ results: SearchResult[] }>("find_similar_documents", { filePath });
      setSimilarResults(response.results);
      setSimilarSourceFile(filePath.split(/[/\\]/).pop() || filePath);
      showToast(`유사 문서 ${response.results.length}건 발견`, "success");
    } catch {
      showToast("유사 문서 검색 실패 (시맨틱 검색이 활성화되어 있어야 합니다)", "error");
    }
  }, [showToast]);

  const clearSimilarResults = useCallback(() => {
    setSimilarResults([]);
    setSimilarSourceFile(null);
  }, []);

  return { similarResults, similarSourceFile, handleFindSimilar, clearSimilarResults };
}
