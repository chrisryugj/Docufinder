import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { SuggestionItem } from "../types/search";

interface UseAutoCompleteOptions {
  query: string;
  debounceMs?: number;
  minChars?: number;
  enabled?: boolean;
}

interface UseAutoCompleteReturn {
  suggestions: SuggestionItem[];
  isOpen: boolean;
  selectedIndex: number;
  setSelectedIndex: (index: number) => void;
  close: () => void;
  open: () => void;
  selectItem: (index: number) => string | null;
  handleKeyDown: (e: React.KeyboardEvent) => string | null;
}

/**
 * 검색어 자동완성 훅
 * - debounce된 IPC 호출로 제안 목록 조회
 * - 키보드 탐색 (↑↓ Enter Esc)
 */
export function useAutoComplete({
  query,
  debounceMs = 200,
  minChars = 2,
  enabled = true,
}: UseAutoCompleteOptions): UseAutoCompleteReturn {
  const [suggestions, setSuggestions] = useState<SuggestionItem[]>([]);
  const [isOpen, setIsOpen] = useState(false);
  const [selectedIndex, setSelectedIndex] = useState(-1);
  const requestIdRef = useRef(0);

  // 스크롤 시 자동 닫기 (window 레벨 캡처 — 결과 영역 외 스크롤 포함)
  useEffect(() => {
    if (!isOpen) return;
    const handleScroll = () => setIsOpen(false);
    window.addEventListener("scroll", handleScroll, { passive: true, capture: true });
    return () => window.removeEventListener("scroll", handleScroll, { capture: true });
  }, [isOpen]);

  // 쿼리 변경 시 제안 조회
  useEffect(() => {
    if (!enabled || query.trim().length < minChars) {
      setSuggestions([]);
      setIsOpen(false);
      return;
    }

    const currentId = ++requestIdRef.current;
    const timer = setTimeout(async () => {
      try {
        const result = await invoke<SuggestionItem[]>("get_suggestions", {
          query: query.trim(),
        });
        if (requestIdRef.current !== currentId) return;
        setSuggestions(result);
        setIsOpen(result.length > 0);
        setSelectedIndex(-1);
      } catch {
        if (requestIdRef.current !== currentId) return;
        setSuggestions([]);
        setIsOpen(false);
      }
    }, debounceMs);

    return () => clearTimeout(timer);
  }, [query, debounceMs, minChars, enabled]);

  const close = useCallback(() => {
    setIsOpen(false);
    setSelectedIndex(-1);
  }, []);

  const open = useCallback(() => {
    if (suggestions.length > 0) setIsOpen(true);
  }, [suggestions.length]);

  const selectItem = useCallback(
    (index: number): string | null => {
      if (index >= 0 && index < suggestions.length) {
        const text = suggestions[index].text;
        close();
        return text;
      }
      return null;
    },
    [suggestions, close]
  );

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent): string | null => {
      if (!isOpen || suggestions.length === 0) return null;

      switch (e.key) {
        case "ArrowDown":
          e.preventDefault();
          setSelectedIndex((prev) =>
            prev < suggestions.length - 1 ? prev + 1 : 0
          );
          return null;
        case "ArrowUp":
          e.preventDefault();
          setSelectedIndex((prev) =>
            prev > 0 ? prev - 1 : suggestions.length - 1
          );
          return null;
        case "Enter":
          if (selectedIndex >= 0) {
            e.preventDefault();
            return selectItem(selectedIndex);
          }
          close();
          return null;
        case "Escape":
          e.preventDefault();
          close();
          return null;
        default:
          return null;
      }
    },
    [isOpen, suggestions.length, selectedIndex, selectItem, close]
  );

  // 검색어 저장 (검색 실행 시 외부에서 호출)
  return {
    suggestions,
    isOpen,
    selectedIndex,
    setSelectedIndex,
    close,
    open,
    selectItem,
    handleKeyDown,
  };
}

/** 검색어 히스토리에 저장 */
export async function saveSearchQuery(query: string): Promise<void> {
  if (query.trim().length < 2) return;
  try {
    await invoke("save_search_query", { query: query.trim() });
  } catch {
    // 저장 실패는 무시 (검색 흐름 블로킹 방지)
  }
}
