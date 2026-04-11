import { useState, useCallback, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

type ShowToastFn = (message: string, type: "success" | "error" | "loading" | "info") => string;

export interface TagInfo {
  tag: string;
  count: number;
}

interface UseFileTagsReturn {
  /** 전체 태그 목록 (사용 횟수 포함) */
  allTags: TagInfo[];
  /** 특정 파일의 태그 가져오기 */
  getFileTags: (filePath: string) => Promise<string[]>;
  /** 태그 추가 */
  addTag: (filePath: string, tag: string) => Promise<void>;
  /** 태그 제거 */
  removeTag: (filePath: string, tag: string) => Promise<void>;
  /** 태그 목록 새로고침 */
  refreshAllTags: () => Promise<void>;
}

export function useFileTags(showToast?: ShowToastFn): UseFileTagsReturn {
  const [allTags, setAllTags] = useState<TagInfo[]>([]);

  const refreshAllTags = useCallback(async () => {
    try {
      const tags = await invoke<TagInfo[]>("get_all_tags");
      setAllTags(tags);
    } catch {
      // 조용히 실패 (초기 로딩)
    }
  }, []);

  useEffect(() => {
    refreshAllTags();
  }, [refreshAllTags]);

  const getFileTags = useCallback(async (filePath: string): Promise<string[]> => {
    try {
      return await invoke<string[]>("get_file_tags", { filePath });
    } catch {
      return [];
    }
  }, []);

  const addTag = useCallback(async (filePath: string, tag: string) => {
    try {
      await invoke("add_file_tag", { filePath, tag });
      await refreshAllTags();
    } catch (e) {
      showToast?.("태그 추가에 실패했습니다", "error");
    }
  }, [refreshAllTags, showToast]);

  const removeTag = useCallback(async (filePath: string, tag: string) => {
    try {
      await invoke("remove_file_tag", { filePath, tag });
      await refreshAllTags();
    } catch (e) {
      showToast?.("태그 제거에 실패했습니다", "error");
    }
  }, [refreshAllTags, showToast]);

  return { allTags, getFileTags, addTag, removeTag, refreshAllTags };
}
