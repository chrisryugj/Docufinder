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
  /** 검색 결과에서 같은 문서의 여러 버전을 대표 1개로 접기 (Document Lineage) */
  group_versions: boolean;
  /** 자동 동기화 주기 (분). 0 = 끄기, 기본 10분. watcher 이벤트 누락 보완. */
  auto_sync_interval_minutes: number;
  /** 오류 자동 리포트 (Telegram). 파일 경로는 익명화, 문서 내용은 전송하지 않음. */
  error_reporting_enabled: boolean;
  /**
   * PDF 수식 OCR 활성화 (기본 false). 토글 켜면 첫 사용 시 ~155MB 모델 자동 다운로드.
   * 인식된 수식은 `$...$` (inline) / `$$...$$` (display) 로 검색/미리보기에 반영.
   */
  formula_ocr_enabled: boolean;
  /**
   * 클라우드/네트워크 폴더(OneDrive·구글·NAVER Works·UNC·매핑 SMB 등)의 본문 인덱싱 자동 스킵.
   * 기본 true: 메타데이터만 인덱싱(파일명 검색 가능), hydrate/다운로드 차단.
   * false: 일반 로컬 폴더처럼 본문까지 인덱싱 (NAS 등 빠른 환경에서 사용자 선택).
   */
  skip_cloud_body_indexing: boolean;
}
