import { useState, useEffect, useCallback } from "react";
import { invokeWithTimeout, IPC_TIMEOUT } from "../utils/invokeWithTimeout";
import type { Settings, ViewDensity } from "../types/settings";
import type { SearchMode } from "../types/search";

function isLightColor(hex: string): boolean {
  const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex);
  if (!result) return true;
  const r = parseInt(result[1], 16);
  const g = parseInt(result[2], 16);
  const b = parseInt(result[3], 16);
  return (r * 299 + g * 587 + b * 114) / 1000 > 128;
}

interface UseAppSettingsOptions {
  setSearchMode: (mode: SearchMode) => void;
}

export function useAppSettings({ setSearchMode }: UseAppSettingsOptions) {
  const [minConfidence, setMinConfidence] = useState(0);
  const [viewDensity, setViewDensity] = useState<ViewDensity>("compact");
  const [semanticEnabled, setSemanticEnabled] = useState(false);

  const applyHighlightColors = useCallback((settings: Settings) => {
    const root = document.documentElement;

    if (settings.highlight_filename_color) {
      root.style.setProperty("--color-highlight-filename-bg", settings.highlight_filename_color);
      const isLightBg = isLightColor(settings.highlight_filename_color);
      root.style.setProperty("--color-highlight-filename-text", isLightBg ? "#0f172a" : "#fef3c7");
    } else {
      root.style.removeProperty("--color-highlight-filename-bg");
      root.style.removeProperty("--color-highlight-filename-text");
    }

    if (settings.highlight_content_color) {
      root.style.setProperty("--color-highlight-bg", settings.highlight_content_color);
      const isLightBg = isLightColor(settings.highlight_content_color);
      root.style.setProperty("--color-highlight-text", isLightBg ? "#0f172a" : "#fef08a");
    } else {
      root.style.removeProperty("--color-highlight-bg");
      root.style.removeProperty("--color-highlight-text");
    }
  }, []);

  // 설정 로드
  useEffect(() => {
    const loadSettings = async () => {
      try {
        const settings = await invokeWithTimeout<Settings>("get_settings", undefined, IPC_TIMEOUT.SETTINGS);
        setSearchMode(settings.search_mode ?? ("keyword" as SearchMode));
        setMinConfidence(settings.min_confidence ?? 0);
        setViewDensity(settings.view_density ?? "compact");
        setSemanticEnabled(settings.semantic_search_enabled ?? false);
        applyHighlightColors(settings);
      } catch (err) {
        console.warn("Failed to load settings:", err);
      }
    };
    loadSettings();
  }, [setSearchMode, applyHighlightColors]);

  const applySettings = useCallback(
    (settings: Settings) => {
      setSearchMode(settings.search_mode ?? ("keyword" as SearchMode));
      setMinConfidence(settings.min_confidence ?? 0);
      setViewDensity(settings.view_density ?? "compact");
      setSemanticEnabled(settings.semantic_search_enabled ?? false);
      applyHighlightColors(settings);
    },
    [setSearchMode, applyHighlightColors]
  );

  return {
    minConfidence,
    viewDensity,
    setViewDensity,
    semanticEnabled,
    setSemanticEnabled,
    applySettings,
  };
}
