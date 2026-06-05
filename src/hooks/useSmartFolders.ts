import { useState, useCallback, useEffect } from "react";
import type { SearchFilters, SearchMode, KeywordMatchMode } from "../types/search";

const STORAGE_KEY = "docufinder_smart_folders";

/**
 * 스마트 폴더 = 저장된 검색(쿼리 + 필터 + 모드).
 * 자주 보는 관심 주제를 한 번 저장해두고 사이드바에서 클릭 한 번으로 재실행한다.
 * (필터 프리셋은 필터만, 최근 검색은 쿼리만 — 스마트 폴더는 셋을 묶어 영속화)
 */
export interface SmartFolder {
  id: string;
  name: string;
  query: string;
  filters: Omit<SearchFilters, "searchScope">; // scope는 폴더 의존이라 제외(프리셋과 동일 정책)
  searchMode: SearchMode;
  keywordMatchMode: KeywordMatchMode;
}

function load(): SmartFolder[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? JSON.parse(raw) : [];
  } catch {
    return [];
  }
}

function persist(folders: SmartFolder[]) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(folders));
}

export function useSmartFolders() {
  const [smartFolders, setSmartFolders] = useState<SmartFolder[]>(load);

  // 다른 창/탭 동기화 (프리셋과 동일 패턴)
  useEffect(() => {
    const handler = (e: StorageEvent) => {
      if (e.key === STORAGE_KEY) setSmartFolders(load());
    };
    window.addEventListener("storage", handler);
    return () => window.removeEventListener("storage", handler);
  }, []);

  /** 추가. 같은 query 가 이미 있으면 false 반환(중복 저장 방지). */
  const addSmartFolder = useCallback(
    (folder: Omit<SmartFolder, "id">): boolean => {
      let added = false;
      setSmartFolders((prev) => {
        if (prev.some((f) => f.query === folder.query)) return prev;
        added = true;
        const next = [...prev, { ...folder, id: Date.now().toString(36) }];
        persist(next);
        return next;
      });
      return added;
    },
    []
  );

  const removeSmartFolder = useCallback((id: string) => {
    setSmartFolders((prev) => {
      const next = prev.filter((f) => f.id !== id);
      persist(next);
      return next;
    });
  }, []);

  return { smartFolders, addSmartFolder, removeSmartFolder };
}
