import type { Theme } from "../hooks/useTheme";
import type { SearchMode } from "./search";

export type ViewDensity = "normal" | "compact";

export interface Settings {
  search_mode: SearchMode;
  max_results: number;
  chunk_size: number;
  chunk_overlap: number;
  theme: Theme;
  min_confidence: number;
  view_density: ViewDensity;
  include_subfolders: boolean;
  auto_start: boolean;
  start_minimized: boolean;
  /** 파일명 하이라이트 색상 (hex) */
  highlight_filename_color?: string;
  /** 문서 내용 하이라이트 색상 (hex) */
  highlight_content_color?: string;
}
