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
    // 배치 커맨드 1회 + setCategories 1회 — 파일당 개별 IPC/리렌더 방지 (T2-6)
    invoke<Record<string, string>>("classify_documents", { filePaths: batch })
      .then(cats => {
        // 응답에 빠진 경로(개별 조회 실패)는 다음 결과 변경 때 재시도되도록 되돌림
        batch.forEach(p => {
          if (!(p in cats)) classifiedPathsRef.current.delete(p);
        });
        if (Object.keys(cats).length > 0) {
          setCategories(prev => ({ ...prev, ...cats }));
        }
      })
      .catch(() => {
        batch.forEach(p => classifiedPathsRef.current.delete(p));
      });
  }, [filteredResults]);

  return categories;
}
