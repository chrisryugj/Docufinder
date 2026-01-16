import { useState, useCallback } from "react";

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
    } catch (error) {
      console.error(`Error reading localStorage key "${key}":`, error);
      return initialValue;
    }
  });

  // 값 변경 시 로컬 스토리지 동기화
  const setValue = useCallback(
    (value: T | ((prev: T) => T)) => {
      try {
        const valueToStore =
          value instanceof Function ? value(storedValue) : value;
        setStoredValue(valueToStore);
        window.localStorage.setItem(key, JSON.stringify(valueToStore));
      } catch (error) {
        console.error(`Error setting localStorage key "${key}":`, error);
      }
    },
    [key, storedValue]
  );

  return [storedValue, setValue];
}

// === 특화된 훅들 ===

const RECENT_SEARCHES_KEY = "docufinder_recent_searches";
const MAX_RECENT_SEARCHES = 10;

/**
 * 최근 검색어 관리 훅
 */
export function useRecentSearches() {
  const [searches, setSearches] = useLocalStorage<string[]>(
    RECENT_SEARCHES_KEY,
    []
  );

  const addSearch = useCallback(
    (query: string) => {
      if (!query.trim()) return;

      setSearches((prev) => {
        // 중복 제거 후 앞에 추가
        const filtered = prev.filter((s) => s !== query);
        return [query, ...filtered].slice(0, MAX_RECENT_SEARCHES);
      });
    },
    [setSearches]
  );

  const removeSearch = useCallback(
    (query: string) => {
      setSearches((prev) => prev.filter((s) => s !== query));
    },
    [setSearches]
  );

  const clearSearches = useCallback(() => {
    setSearches([]);
  }, [setSearches]);

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
