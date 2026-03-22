import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { SearchResult } from "../types/search";

/**
 * 문서 카테고리 자동 분류 (시맨틱 검색 활성 시)
 */
export function useDocumentCategories(
  filteredResults: SearchResult[],
  semanticEnabled: boolean
): Record<string, string> {
  const [categories, setCategories] = useState<Record<string, string>>({});
  const classifiedPathsRef = useRef(new Set<string>());

  useEffect(() => {
    if (!semanticEnabled || filteredResults.length === 0) return;

    const newPaths = filteredResults
      .map(r => r.file_path)
      .filter((p, i, arr) => arr.indexOf(p) === i && !classifiedPathsRef.current.has(p));

    if (newPaths.length === 0) return;

    const batch = newPaths.slice(0, 10);
    batch.forEach(p => classifiedPathsRef.current.add(p));
    Promise.all(
      batch.map(async (filePath) => {
        try {
          const cat = await invoke<string>("classify_document", { filePath });
          setCategories(prev => ({ ...prev, [filePath]: cat }));
        } catch {
          classifiedPathsRef.current.delete(filePath);
        }
      })
    );
  }, [filteredResults, semanticEnabled]);

  return categories;
}
