import { useState, useEffect, useCallback } from "react";
import { invokeWithTimeout, IPC_TIMEOUT } from "../utils/invokeWithTimeout";
import { setErrorReportingEnabled } from "../utils/errorLogger";
import type { Settings, VectorIndexingMode, ViewDensity } from "../types/settings";
import type { SearchMode } from "../types/search";

function hexToRgba(hex: string, alpha: number): string {
  const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex);
  if (!result) return hex;
  const r = parseInt(result[1], 16);
  const g = parseInt(result[2], 16);
  const b = parseInt(result[3], 16);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

interface UseAppSettingsOptions {
  setSearchMode: (mode: SearchMode) => void;
  setMinConfidence: (v: number) => void;
}

export function useAppSettings({ setSearchMode, setMinConfidence }: UseAppSettingsOptions) {
  const [viewDensity, setViewDensity] = useState<ViewDensity>("compact");
  const [semanticEnabled, setSemanticEnabled] = useState(false);
  const [vectorIndexingMode, setVectorIndexingMode] =
    useState<VectorIndexingMode>("manual");
  const [resultsPerPage, setResultsPerPage] = useState(50);

  const applyHighlightColors = useCallback((settings: Settings) => {
    const root = document.documentElement;

    if (settings.highlight_filename_color) {
      const c = settings.highlight_filename_color;
      root.style.setProperty("--color-highlight-filename-bg", hexToRgba(c, 0.25));
      root.style.setProperty("--color-highlight-filename-border", hexToRgba(c, 0.7));
      root.style.setProperty("--color-highlight-filename-text", "inherit");
    } else {
      root.style.removeProperty("--color-highlight-filename-bg");
      root.style.removeProperty("--color-highlight-filename-border");
      root.style.removeProperty("--color-highlight-filename-text");
    }

    if (settings.highlight_content_color) {
      const c = settings.highlight_content_color;
      root.style.setProperty("--color-highlight-bg", hexToRgba(c, 0.25));
      root.style.setProperty("--color-highlight-border", hexToRgba(c, 0.6));
      root.style.setProperty("--color-highlight-text", "inherit");
    } else {
      root.style.removeProperty("--color-highlight-bg");
      root.style.removeProperty("--color-highlight-border");
      root.style.removeProperty("--color-highlight-text");
    }
  }, []);

  // 설정 로드
  useEffect(() => {
    const loadSettings = async () => {
      try {
        const settings = await invokeWithTimeout<Settings>("get_settings", undefined, IPC_TIMEOUT.SETTINGS);
        // hybrid/semantic은 UI에서 제거됨 → keyword로 폴백
        const mode = settings.search_mode ?? "keyword";
        setSearchMode((mode === "hybrid" || mode === "semantic") ? "keyword" as SearchMode : mode);
        setMinConfidence(settings.min_confidence ?? 0);
        setViewDensity(settings.view_density ?? "compact");
        setSemanticEnabled(settings.semantic_search_enabled ?? false);
        setVectorIndexingMode(settings.vector_indexing_mode ?? "manual");
        setResultsPerPage(settings.results_per_page ?? 50);
        setErrorReportingEnabled(settings.error_reporting_enabled ?? true);

        applyHighlightColors(settings);
      } catch {
        // 설정 로드 실패 시 기본값 유지
      }
    };
    loadSettings();
  }, [setSearchMode, setMinConfidence, applyHighlightColors]);

  const applySettings = useCallback(
    (settings: Settings) => {
      const mode = settings.search_mode ?? "keyword";
      setSearchMode((mode === "hybrid" || mode === "semantic") ? "keyword" as SearchMode : mode);
      setMinConfidence(settings.min_confidence ?? 0);
      setViewDensity(settings.view_density ?? "compact");
      setSemanticEnabled(settings.semantic_search_enabled ?? false);
      setVectorIndexingMode(settings.vector_indexing_mode ?? "manual");
      setResultsPerPage(settings.results_per_page ?? 50);
      setErrorReportingEnabled(settings.error_reporting_enabled ?? true);
      applyHighlightColors(settings);
    },
    [setSearchMode, setMinConfidence, applyHighlightColors]
  );

  return {
    viewDensity,
    setViewDensity,
    semanticEnabled,
    setSemanticEnabled,
    vectorIndexingMode,
    resultsPerPage,
    applySettings,
  };
}
