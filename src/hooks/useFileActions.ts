import { useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { useToast } from "./useToast";
import type { useIndexStatus } from "./useIndexStatus";

interface UseFileActionsOptions {
  query: string;
  addSearch: (query: string) => void;
  showToast: ReturnType<typeof useToast>["showToast"];
  updateToast: ReturnType<typeof useToast>["updateToast"];
  addFolder: ReturnType<typeof useIndexStatus>["addFolder"];
  removeFolder: ReturnType<typeof useIndexStatus>["removeFolder"];
  invalidateSearch: () => void;
}

export function useFileActions({
  query,
  addSearch,
  showToast,
  updateToast,
  addFolder,
  removeFolder,
  invalidateSearch,
}: UseFileActionsOptions) {
  const handleOpenFile = useCallback(
    async (filePath: string, page?: number | null) => {
      const trimmedQuery = query.trim();
      if (trimmedQuery.length >= 2) {
        addSearch(trimmedQuery);
      }

      const toastId = showToast("파일 여는 중...", "loading");
      try {
        await invoke("open_file", { path: filePath, page: page ?? null });
        updateToast(toastId, { message: "파일을 열었습니다", type: "success" });
      } catch (err) {
        console.error("Failed to open file:", err);
        updateToast(toastId, { message: "파일 열기 실패", type: "error" });
      }
    },
    [query, addSearch, showToast, updateToast]
  );

  const handleCopyPath = useCallback(
    async (path: string) => {
      try {
        const cleanPath = path.replace(/^\\\\\?\\/, "");
        await navigator.clipboard.writeText(cleanPath);
        showToast("경로가 복사되었습니다", "success");
      } catch (err) {
        console.error("Failed to copy path:", err);
        showToast("경로 복사 실패", "error");
      }
    },
    [showToast]
  );

  const handleOpenFolder = useCallback(
    async (folderPath: string) => {
      try {
        const cleanPath = folderPath.replace(/^\\\\\?\\/, "");
        await invoke("open_folder", { path: cleanPath });
        showToast("폴더를 열었습니다", "success");
      } catch (err) {
        console.error("Failed to open folder:", err);
        showToast("폴더 열기 실패", "error");
      }
    },
    [showToast]
  );

  const handleAddFolder = useCallback(async () => {
    const result = await addFolder();
    if (result) {
      const { indexed_count, failed_count, errors } = result;
      if (failed_count > 0) {
        showToast(
          `${indexed_count}개 인덱싱 완료, ${failed_count}개 파싱 실패`,
          "error",
          5000
        );
        if ((import.meta as any).env.DEV && errors?.length) {
          console.warn("[파싱 실패 목록]", errors.slice(0, 20));
        }
      } else if (indexed_count > 0) {
        showToast(`${indexed_count}개 파일 인덱싱 완료`, "success");
      }
    }
    return result;
  }, [addFolder, showToast]);

  const handleRemoveFolder = useCallback(
    async (path: string) => {
      const toastId = showToast("폴더 제거 중...", "loading");
      try {
        await removeFolder(path);
        invalidateSearch();
        updateToast(toastId, { message: "폴더가 제거되었습니다", type: "success" });
      } catch {
        updateToast(toastId, { message: "폴더 제거 실패", type: "error" });
      }
    },
    [removeFolder, invalidateSearch, showToast, updateToast]
  );

  return {
    handleOpenFile,
    handleCopyPath,
    handleOpenFolder,
    handleAddFolder,
    handleRemoveFolder,
  };
}
