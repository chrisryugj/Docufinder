/** 인덱스 상태 */
export interface IndexStatus {
  total_files: number;
  indexed_files: number;
  watched_folders: string[];
  vectors_count: number;
  semantic_available: boolean;
  filename_cache_truncated?: boolean;
}

/** 폴더 추가 결과 */
export interface AddFolderResult {
  success: boolean;
  indexed_count: number;
  failed_count: number;
  vectors_count: number;
  message: string;
  errors: string[];
  /** 변환 대상 HWP 파일 경로 */
  hwp_files?: string[];
  /** OCR로 인덱싱된 이미지 파일 수 */
  ocr_image_count?: number;
}

/** HWP → HWPX 변환 결과 */
export interface ConvertHwpResult {
  success_count: number;
  failed_count: number;
  converted_paths: string[];
  errors: string[];
  /** 변환기 미설치 시 번들된 설치 파일 경로 */
  installer_path?: string;
}

/** 폴더별 인덱싱 통계 */
export interface FolderStats {
  file_count: number;
  indexed_count: number;
  last_indexed: number | null;
}

/** 감시 폴더 정보 (즐겨찾기 포함) */
export interface WatchedFolderInfo {
  path: string;
  is_favorite: boolean;
  added_at: number | null;
  /** 인덱싱 상태: "indexing" (미완료) | "completed" | "cancelled" (취소됨) | "failed" */
  indexing_status: "indexing" | "completed" | "cancelled" | "failed";
}

/** 인덱싱 진행률 이벤트 (1단계: FTS) */
export interface IndexingProgress {
  /** 현재 진행 단계 */
  phase: "preparing" | "scanning" | "parsing" | "indexing" | "completed" | "cancelled";
  /** 전체 파일 수 */
  total_files: number;
  /** 처리된 파일 수 */
  processed_files: number;
  /** 현재 처리 중인 파일명 */
  current_file: string | null;
  /** 폴더 경로 */
  folder_path: string;
  /** 에러 메시지 (실패 시) */
  error: string | null;
}

/** 벡터 인덱싱 상태 (2단계: 시맨틱) */
export interface VectorIndexingStatus {
  is_running: boolean;
  total_chunks: number;
  processed_chunks: number;
  pending_chunks: number;
  current_file: string | null;
  error: string | null;
}

/** 벡터 인덱싱 진행률 이벤트 */
export interface VectorIndexingProgress {
  total_chunks: number;
  processed_chunks: number;
  current_file: string | null;
  is_complete: boolean;
}
