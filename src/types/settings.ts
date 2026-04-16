import type { Theme } from "../hooks/useTheme";
import type { SearchMode } from "./search";

export type ViewDensity = "normal" | "compact";
export type VectorIndexingMode = "manual" | "auto";
export type IndexingIntensity = "fast" | "balanced" | "background";

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
  /** X 버튼 클릭 시 트레이로 숨김 (false면 앱 종료) */
  close_to_tray: boolean;
  /** 파일명 하이라이트 색상 (hex) */
  highlight_filename_color?: string;
  /** 문서 내용 하이라이트 색상 (hex) */
  highlight_content_color?: string;
  /** 시맨틱 검색 활성화 여부 */
  semantic_search_enabled: boolean;
  /** 벡터 인덱싱 모드 */
  vector_indexing_mode: VectorIndexingMode;
  /** 인덱싱 강도 */
  indexing_intensity: IndexingIntensity;
  /** 단일 파일 최대 크기 (MB). 초과 시 스킵 */
  max_file_size_mb: number;
  /** 검색 결과 더 보기 단위 (한 번에 표시할 개수) */
  results_per_page: number;
  /** 데이터 저장 경로 (DB, 벡터 인덱스). 미설정 시 기본 AppData 사용 */
  data_root?: string;
  /** 사용자 커스텀 제외 디렉토리 목록 */
  exclude_dirs?: string[];
  /** AI 기능 활성화 */
  ai_enabled: boolean;
  /** Gemini API 키 */
  ai_api_key?: string;
  /** AI 모델 ID */
  ai_model: string;
  /** AI 응답 온도 (0.0-2.0) */
  ai_temperature: number;
  /** AI 최대 토큰 수 */
  ai_max_tokens: number;
  /** OCR 기능 활성화 (이미지 파일 텍스트 인식) */
  ocr_enabled: boolean;
}
