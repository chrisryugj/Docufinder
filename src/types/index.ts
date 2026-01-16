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
