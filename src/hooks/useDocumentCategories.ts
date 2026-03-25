import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { SearchResult } from "../types/search";

/**
 * 문서 카테고리 자동 분류 (키워드 패턴 매칭 — 시맨틱 검색 불필요)
 */
export function useDocumentCategories(
  filteredResults: SearchResult[],
  _semanticEnabled?: boolean
): Record<string, string> {
  const [categories, setCategories] = useState<Record<string, string>>({});
  const classifiedPathsRef = useRef(new Set<string>());

  useEffect(() => {
    if (filteredResults.length === 0) return;

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
  }, [filteredResults]);

  return categories;
}
