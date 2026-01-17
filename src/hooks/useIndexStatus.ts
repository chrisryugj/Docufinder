import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { IndexStatus, AddFolderResult } from "../types/index";
import { open } from "@tauri-apps/plugin-dialog";

interface UseIndexStatusReturn {
  status: IndexStatus | null;
  isIndexing: boolean;
  error: string | null;
  clearError: () => void;
  refreshStatus: () => Promise<void>;
  addFolder: () => Promise<AddFolderResult | null>;
  removeFolder: (path: string) => Promise<void>;
}

/**
 * 인덱스 상태 관리 훅
 */
export function useIndexStatus(): UseIndexStatusReturn {
  const [status, setStatus] = useState<IndexStatus | null>(null);
  const [isIndexing, setIsIndexing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const clearError = useCallback(() => setError(null), []);

  // 상태 조회
  const refreshStatus = useCallback(async () => {
    try {
      const result = await invoke<IndexStatus>("get_index_status");
      setStatus(result);
    } catch (err) {
      console.error("Failed to get status:", err);
    }
  }, []);

  // 초기 로드
  useEffect(() => {
    refreshStatus();
  }, [refreshStatus]);

  // 폴더 추가
  const addFolder = useCallback(async (): Promise<AddFolderResult | null> => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: "인덱싱할 폴더 선택",
      });

      if (selected) {
        setIsIndexing(true);
        setError(null);

        const result = await invoke<AddFolderResult>("add_folder", {
          path: selected,
        });

        console.log("Indexing result:", result);
        await refreshStatus();
        setIsIndexing(false);

        return result;
      }

      return null;
    } catch (err) {
      console.error("Failed to add folder:", err);
      const message = err instanceof Error ? err.message : String(err);
      setError(`폴더 추가 실패: ${message}`);
      setIsIndexing(false);
      return null;
    }
  }, [refreshStatus]);

  // 폴더 제거
  const removeFolder = useCallback(async (path: string): Promise<void> => {
    try {
      setError(null);
      await invoke("remove_folder", { path });
      await refreshStatus();
    } catch (err) {
      console.error("Failed to remove folder:", err);
      const message = err instanceof Error ? err.message : String(err);
      setError(`폴더 제거 실패: ${message}`);
    }
  }, [refreshStatus]);

  return {
    status,
    isIndexing,
    error,
    clearError,
    refreshStatus,
    addFolder,
    removeFolder,
  };
}
