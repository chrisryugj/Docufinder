import { useState, useCallback } from "react";
import type { RecentSearch } from "../types/search";

/**
 * 로컬 스토리지 동기화 훅
 */
export function useLocalStorage<T>(
  key: string,
  initialValue: T
): [T, (value: T | ((prev: T) => T)) => void] {
  // 초기값 로드
  const [storedValue, setStoredValue] = useState<T>(() => {
    try {
      const item = window.localStorage.getItem(key);
      return item ? (JSON.parse(item) as T) : initialValue;
    } catch {
      return initialValue;
    }
  });

  // 값 변경 시 로컬 스토리지 동기화 (stale closure 방지)
  const setValue = useCallback(
    (value: T | ((prev: T) => T)) => {
      setStoredValue((prev) => {
        try {
          const valueToStore = value instanceof Function ? value(prev) : value;
          window.localStorage.setItem(key, JSON.stringify(valueToStore));
          return valueToStore;
        } catch {
          return prev;
        }
      });
    },
    [key]
  );

  return [storedValue, setValue];
}

// === 특화된 훅들 ===

const RECENT_SEARCHES_KEY = "docufinder_recent_searches_v2";
const LEGACY_SEARCHES_KEY = "docufinder_recent_searches";
const MAX_RECENT_SEARCHES = 10;

/**
 * 기존 string[] 형식을 RecentSearch[] 형식으로 마이그레이션
 */
function migrateRecentSearches(): RecentSearch[] {
  try {
    // 새 키 먼저 확인
    const newData = window.localStorage.getItem(RECENT_SEARCHES_KEY);
    if (newData) {
      return JSON.parse(newData) as RecentSearch[];
    }

    // 레거시 데이터 마이그레이션
    const legacyData = window.localStorage.getItem(LEGACY_SEARCHES_KEY);
    if (legacyData) {
      const parsed = JSON.parse(legacyData);
      // string[] 형식이면 마이그레이션
      if (Array.isArray(parsed) && parsed.length > 0 && typeof parsed[0] === "string") {
        const migrated: RecentSearch[] = parsed.map((query: string) => ({
          query,
          timestamp: Date.now(),
        }));
        // 새 키로 저장
        window.localStorage.setItem(RECENT_SEARCHES_KEY, JSON.stringify(migrated));
        // 레거시 키 삭제
        window.localStorage.removeItem(LEGACY_SEARCHES_KEY);
        return migrated;
      }
    }
  } catch {
    // 마이그레이션 실패 시 빈 배열 반환
  }
  return [];
}

/**
 * 최근 검색어 관리 훅
 * - RecentSearch 형식으로 저장 (query + timestamp)
 * - 기존 string[] 형식 자동 마이그레이션
 */
export function useRecentSearches() {
  const [searches, setSearches] = useState<RecentSearch[]>(() => migrateRecentSearches());

  // localStorage 동기화
  const saveSearches = useCallback((newSearches: RecentSearch[]) => {
    setSearches(newSearches);
    try {
      window.localStorage.setItem(RECENT_SEARCHES_KEY, JSON.stringify(newSearches));
    } catch {
      // localStorage 저장 실패 무시 (graceful degradation)
    }
  }, []);

  const addSearch = useCallback(
    (query: string) => {
      if (!query.trim()) return;

      setSearches((prev) => {
        // 중복 제거 후 앞에 추가 (새 타임스탬프로)
        const filtered = prev.filter((s) => s.query !== query);
        const newSearches: RecentSearch[] = [
          { query, timestamp: Date.now() },
          ...filtered,
        ].slice(0, MAX_RECENT_SEARCHES);

        // localStorage 저장
        try {
          window.localStorage.setItem(RECENT_SEARCHES_KEY, JSON.stringify(newSearches));
        } catch {
          // localStorage 저장 실패 무시 (graceful degradation)
        }

        return newSearches;
      });
    },
    []
  );

  const removeSearch = useCallback(
    (query: string) => {
      setSearches((prev) => {
        const newSearches = prev.filter((s) => s.query !== query);
        try {
          window.localStorage.setItem(RECENT_SEARCHES_KEY, JSON.stringify(newSearches));
        } catch {
          // localStorage 저장 실패 무시 (graceful degradation)
        }
        return newSearches;
      });
    },
    []
  );

  const clearSearches = useCallback(() => {
    saveSearches([]);
  }, [saveSearches]);

  return {
    searches,
    addSearch,
    removeSearch,
    clearSearches,
  };
}

const FAVORITES_KEY = "docufinder_favorites";

/**
 * 즐겨찾기 폴더 관리 훅
 */
export function useFavorites() {
  const [favorites, setFavorites] = useLocalStorage<string[]>(
    FAVORITES_KEY,
    []
  );

  const addFavorite = useCallback(
    (path: string) => {
      setFavorites((prev) => {
        if (prev.includes(path)) return prev;
        return [...prev, path];
      });
    },
    [setFavorites]
  );

  const removeFavorite = useCallback(
    (path: string) => {
      setFavorites((prev) => prev.filter((p) => p !== path));
    },
    [setFavorites]
  );

  const isFavorite = useCallback(
    (path: string) => favorites.includes(path),
    [favorites]
  );

  return {
    favorites,
    addFavorite,
    removeFavorite,
    isFavorite,
  };
}
