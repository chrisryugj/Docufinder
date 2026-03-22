import { useEffect, useRef } from "react";
import { saveSearchQuery } from "./useAutoComplete";

const RECENT_SEARCH_SAVE_DELAY_MS = 3000;

/**
 * 검색 결과가 있고 3초 유지 시 최근 검색에 자동 저장
 */
export function useRecentSearchSaver(
  query: string,
  filteredResultsLength: number,
  addSearch: (query: string) => void
): void {
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (timerRef.current) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }

    const trimmedQuery = query.trim();
    if (trimmedQuery.length >= 2 && filteredResultsLength > 0) {
      timerRef.current = setTimeout(() => {
        addSearch(trimmedQuery);
        saveSearchQuery(trimmedQuery);
        timerRef.current = null;
      }, RECENT_SEARCH_SAVE_DELAY_MS);
    }

    return () => {
      if (timerRef.current) {
        clearTimeout(timerRef.current);
      }
    };
  }, [query, filteredResultsLength, addSearch]);
}
