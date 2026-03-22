import { useCallback } from "react";
import { invokeWithTimeout, IPC_TIMEOUT } from "../utils/invokeWithTimeout";
import type { useToast } from "./useToast";
import type { useIndexStatus } from "./useIndexStatus";

interface UseFileActionsOptions {
  query: string;
  addSearch: (query: string) => void;
  showToast: ReturnType<typeof useToast>["showToast"];
  updateToast: ReturnType<typeof useToast>["updateToast"];
  addFolder: ReturnType<typeof useIndexStatus>["addFolder"];
  addFolderByPath: ReturnType<typeof useIndexStatus>["addFolderByPath"];
  removeFolder: ReturnType<typeof useIndexStatus>["removeFolder"];
  invalidateSearch: () => void;
  refreshVectorStatus?: () => Promise<unknown>;
}

export function useFileActions({
  query,
  addSearch,
  showToast,
  updateToast,
  addFolder,
  addFolderByPath,
  removeFolder,
  invalidateSearch,
  refreshVectorStatus,
}: UseFileActionsOptions) {
  const handleOpenFile = useCallback(
    async (filePath: string, page?: number | null) => {
      const trimmedQuery = query.trim();
      if (trimmedQuery.length >= 2) {
        addSearch(trimmedQuery);
      }

      const toastId = showToast("파일 여는 중...", "loading");
      try {
        await invokeWithTimeout("open_file", { path: filePath, page: page ?? null }, IPC_TIMEOUT.FILE_ACTION);
        updateToast(toastId, { message: "파일을 열었습니다", type: "success" });
      } catch {
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
      } catch {
        showToast("경로 복사 실패", "error");
      }
    },
    [showToast]
  );

  const handleOpenFolder = useCallback(
    async (folderPath: string) => {
      try {
        const cleanPath = folderPath.replace(/^\\\\\?\\/, "");
        await invokeWithTimeout("open_folder", { path: cleanPath }, IPC_TIMEOUT.FILE_ACTION);
        showToast("폴더를 열었습니다", "success");
      } catch {
        showToast("폴더 열기 실패", "error");
      }
    },
    [showToast]
  );

  const handleAddFolder = useCallback(async () => {
    const results = await addFolder();
    if (results && results.length > 0) {
      const totalIndexed = results.reduce((sum, r) => sum + r.indexed_count, 0);
      const totalFailed = results.reduce((sum, r) => sum + r.failed_count, 0);

      if (totalFailed > 0) {
        showToast(
          `${totalIndexed}개 인덱싱 완료, ${totalFailed}개 파싱 실패`,
          "error",
          5000
        );
      } else if (totalIndexed > 0) {
        const folderCount = results.length;
        const msg = folderCount > 1
          ? `${folderCount}개 폴더, ${totalIndexed}개 파일 인덱싱 완료`
          : `${totalIndexed}개 파일 인덱싱 완료`;
        showToast(msg, "success");
      }
    }
    return results;
  }, [addFolder, showToast]);

  const handleAddFolderByPath = useCallback(async (path: string) => {
    const result = await addFolderByPath(path);
    if (result) {
      if (result.failed_count > 0) {
        showToast(
          `${result.indexed_count}개 인덱싱 완료, ${result.failed_count}개 파싱 실패`,
          "error",
          5000
        );
      } else if (result.indexed_count > 0) {
        showToast(`${result.indexed_count}개 파일 인덱싱 완료`, "success");
      }
    }
    return result;
  }, [addFolderByPath, showToast]);

  const handleRemoveFolder = useCallback(
    async (path: string) => {
      const toastId = showToast("폴더 제거 중...", "loading");
      try {
        await removeFolder(path);
        invalidateSearch();
        await refreshVectorStatus?.();
        updateToast(toastId, { message: "폴더가 제거되었습니다", type: "success" });
      } catch {
        updateToast(toastId, { message: "폴더 제거 실패", type: "error" });
      }
    },
    [removeFolder, invalidateSearch, showToast, updateToast, refreshVectorStatus]
  );

  return {
    handleOpenFile,
    handleCopyPath,
    handleOpenFolder,
    handleAddFolder,
    handleAddFolderByPath,
    handleRemoveFolder,
  };
}
