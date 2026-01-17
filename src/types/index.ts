/** 인덱스 상태 */
export interface IndexStatus {
  total_files: number;
  indexed_files: number;
  watched_folders: string[];
  vectors_count: number;
  semantic_available: boolean;
}

/** 폴더 추가 결과 */
export interface AddFolderResult {
  success: boolean;
  indexed_count: number;
  failed_count: number;
  message: string;
}

/** 폴더별 인덱싱 통계 */
export interface FolderStats {
  file_count: number;
  last_indexed: number | null;
}

/** 감시 폴더 정보 (즐겨찾기 포함) */
export interface WatchedFolderInfo {
  path: string;
  is_favorite: boolean;
  added_at: number | null;
}

/** 인덱싱 진행률 이벤트 */
export interface IndexingProgress {
  /** 현재 진행 단계 */
  phase: "scanning" | "parsing" | "indexing" | "completed" | "cancelled";
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
