/** Tauri IPC 커맨드 이름 */
export type TauriCommand =
  | "search_keyword"
  | "search_semantic"
  | "search_hybrid"
  | "add_folder"
  | "remove_folder"
  | "get_index_status"
  | "get_settings"
  | "update_settings"
  | "open_file";

/** 검색 모드별 커맨드 매핑 */
export const SEARCH_COMMANDS = {
  keyword: "search_keyword",
  semantic: "search_semantic",
  hybrid: "search_hybrid",
} as const;
