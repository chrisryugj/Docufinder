/** Tauri IPC 커맨드 이름 */
export type TauriCommand =
  | "search_keyword"
  | "search_filename"
  | "search_semantic"
  | "search_hybrid"
  | "search_smart"
  | "ask_ai"
  | "add_folder"
  | "remove_folder"
  | "get_index_status"
  | "get_settings"
  | "update_settings"
  | "open_file"
  | "export_xlsx"
  | "package_zip";

/** 검색 모드별 커맨드 매핑 */
export const SEARCH_COMMANDS = {
  filename: "search_filename",
  keyword: "search_keyword",
  semantic: "search_semantic",
  hybrid: "search_hybrid",
  smart: "search_smart",
} as const;
