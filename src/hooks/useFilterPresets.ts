import { useState, useCallback, useEffect } from "react";
import type { SearchFilters } from "../types/search";

const STORAGE_KEY = "docufinder_filter_presets";

export interface FilterPreset {
  id: string;
  name: string;
  filters: Omit<SearchFilters, "searchScope">; // scope는 폴더 의존이라 제외
}

function loadPresets(): FilterPreset[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? JSON.parse(raw) : [];
  } catch {
    return [];
  }
}

function savePresets(presets: FilterPreset[]) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(presets));
}

export function useFilterPresets() {
  const [presets, setPresets] = useState<FilterPreset[]>(loadPresets);

  // 다른 탭 동기화
  useEffect(() => {
    const handler = (e: StorageEvent) => {
      if (e.key === STORAGE_KEY) setPresets(loadPresets());
    };
    window.addEventListener("storage", handler);
    return () => window.removeEventListener("storage", handler);
  }, []);

  const addPreset = useCallback((name: string, filters: SearchFilters) => {
    const preset: FilterPreset = {
      id: Date.now().toString(36),
      name,
      filters: {
        sortBy: filters.sortBy,
        fileTypes: filters.fileTypes,
        dateRange: filters.dateRange,
        keywordOnly: filters.keywordOnly,
        excludeFilename: filters.excludeFilename,
      },
    };
    setPresets((prev) => {
      const next = [...prev, preset];
      savePresets(next);
      return next;
    });
  }, []);

  const removePreset = useCallback((id: string) => {
    setPresets((prev) => {
      const next = prev.filter((p) => p.id !== id);
      savePresets(next);
      return next;
    });
  }, []);

  const applyPreset = useCallback((preset: FilterPreset, currentFilters: SearchFilters): SearchFilters => {
    return {
      ...preset.filters,
      searchScope: currentFilters.searchScope, // scope 유지
    };
  }, []);

  return { presets, addPreset, removePreset, applyPreset };
}
