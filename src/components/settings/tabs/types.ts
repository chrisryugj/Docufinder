import type { Settings } from "../../../types/settings";

export interface TabProps {
  settings: Settings;
  onChange: <K extends keyof Settings>(key: K, value: Settings[K]) => void;
  setError?: (error: string | null) => void;
}

export const SEARCH_MODE_OPTIONS = [
  { value: "keyword", label: "키워드 검색 (권장)" },
  { value: "filename", label: "파일명 검색" },
];

export const THEME_OPTIONS = [
  { value: "light", label: "라이트 모드" },
  { value: "dark", label: "다크 모드" },
  { value: "system", label: "시스템 설정" },
];

export const MAX_RESULTS_OPTIONS = [
  { value: "20", label: "20개" },
  { value: "50", label: "50개 (기본)" },
  { value: "100", label: "100개" },
  { value: "200", label: "200개" },
  { value: "500", label: "500개" },
  { value: "1000", label: "1000개" },
];

export const VIEW_DENSITY_OPTIONS = [
  { value: "normal", label: "기본 (넓게)" },
  { value: "compact", label: "컴팩트 (좁게)" },
];

export const VECTOR_INDEXING_MODE_OPTIONS = [
  { value: "manual", label: "수동" },
  { value: "auto", label: "자동" },
];

export const INDEXING_INTENSITY_OPTIONS = [
  { value: "fast", label: "빠르게 (CPU 최대)" },
  { value: "balanced", label: "균형 (권장)" },
  { value: "background", label: "백그라운드 (최소 부하)" },
];

export const RESULTS_PER_PAGE_OPTIONS = [
  { value: "20", label: "20개" },
  { value: "50", label: "50개 (기본)" },
  { value: "100", label: "100개" },
  { value: "200", label: "200개" },
];

/// 단일 파일 최대 크기 기본값 (MB) — src-tauri/src/constants.rs::DEFAULT_MAX_FILE_SIZE_MB와 동기화
export const DEFAULT_MAX_FILE_SIZE_MB = 200;
/// 단일 파일 크기 절대 상한 (MB) — src-tauri/src/constants.rs::MAX_FILE_SIZE_LIMIT_MB와 동기화
export const MAX_FILE_SIZE_LIMIT_MB = 500;

export const MAX_FILE_SIZE_OPTIONS = [50, 100, 200, 400, 500].map((mb) => ({
  value: String(mb),
  label:
    mb === DEFAULT_MAX_FILE_SIZE_MB
      ? `${mb} MB (기본)`
      : mb === MAX_FILE_SIZE_LIMIT_MB
        ? `${mb} MB (최대)`
        : `${mb} MB`,
}));

export const AI_MODEL_OPTIONS = [
  { value: "gemini-3.1-flash-lite-preview", label: "Gemini 3.1 Flash Lite (빠름/저렴)" },
  { value: "gemini-3-flash-preview", label: "Gemini 3 Flash (표준)" },
  { value: "gemini-2.5-flash", label: "Gemini 2.5 Flash (균형)" },
  { value: "gemini-3.1-pro-preview", label: "Gemini 3.1 Pro (고품질)" },
];

export const UI_ZOOM_OPTIONS = [
  { value: "0.85", label: "85%" },
  { value: "0.9", label: "90%" },
  { value: "0.95", label: "95%" },
  { value: "1", label: "100% (기본)" },
  { value: "1.05", label: "105%" },
  { value: "1.1", label: "110%" },
  { value: "1.15", label: "115%" },
  { value: "1.2", label: "120%" },
];

export const CONFIDENCE_STEP = 5;

export const HIGHLIGHT_COLOR_PRESETS = [
  { value: "", label: "기본", light: "#fde047", dark: "#854d0e" },
  { value: "#fbbf24", label: "앰버", light: "#fbbf24", dark: "#b45309" },
  { value: "#fb923c", label: "오렌지", light: "#fb923c", dark: "#c2410c" },
  { value: "#f87171", label: "레드", light: "#f87171", dark: "#b91c1c" },
  { value: "#c084fc", label: "퍼플", light: "#c084fc", dark: "#7c3aed" },
  { value: "#60a5fa", label: "블루", light: "#60a5fa", dark: "#1d4ed8" },
  { value: "#34d399", label: "그린", light: "#34d399", dark: "#059669" },
  { value: "#2dd4bf", label: "틸", light: "#2dd4bf", dark: "#0d9488" },
];
