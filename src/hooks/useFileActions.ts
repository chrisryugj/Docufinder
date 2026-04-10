import { useCallback, useRef } from "react";
import { invokeWithTimeout, IPC_TIMEOUT } from "../utils/invokeWithTimeout";
import type { useToast } from "./useToast";
import type { useIndexStatus } from "./useIndexStatus";
import type { AddFolderResult } from "../types/index";

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
  // ── 추천 폴더 큐 (연속 클릭 → 순차 실행 → 통합 토스트) ──
  const folderQueueRef = useRef<string[]>([]);
  const isProcessingRef = useRef(false);
  const queueResultsRef = useRef<AddFolderResult[]>([]);

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
      showFolderResultToast(results, showToast);
    }
    return results;
  }, [addFolder, showToast]);

  // 큐 처리 루프 — 큐가 빌 때까지 순차 실행 후 통합 토스트
  const processQueue = useCallback(async () => {
    if (isProcessingRef.current) return; // 이미 처리 중
    isProcessingRef.current = true;

    while (folderQueueRef.current.length > 0) {
      const path = folderQueueRef.current.shift()!;
      const result = await addFolderByPath(path);
      if (result) {
        queueResultsRef.current.push(result);
      } else {
        queueResultsRef.current.push({
          success: false,
          indexed_count: 0,
          failed_count: 0,
          vectors_count: 0,
          message: "인덱싱 실패",
          errors: [],
        });
      }
    }

    // 전부 끝남 → 통합 토스트
    const results = queueResultsRef.current;
    if (results.length > 0) {
      showFolderResultToast(results, showToast);
    }
    queueResultsRef.current = [];
    isProcessingRef.current = false;
  }, [addFolderByPath, showToast]);

  const handleAddFolderByPath = useCallback(async (path: string) => {
    folderQueueRef.current.push(path);

    // 이미 처리 중이면 큐에만 추가하고 리턴 (루프가 알아서 처리)
    if (!isProcessingRef.current) {
      await processQueue();
    }
  }, [processQueue]);

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

/** 인덱싱 결과 배열 → 통합 토스트 1번 */
function showFolderResultToast(
  results: AddFolderResult[],
  showToast: ReturnType<typeof useToast>["showToast"],
) {
  const totalIndexed = results.reduce((sum, r) => sum + r.indexed_count, 0);
  const totalFailed = results.reduce((sum, r) => sum + r.failed_count, 0);
  const folderCount = results.length;

  if (totalFailed > 0) {
    showToast(
      `${totalIndexed}개 인덱싱 완료, ${totalFailed}개 파싱 실패`,
      "error",
      5000,
    );
  } else if (totalIndexed > 0) {
    const msg = folderCount > 1
      ? `${folderCount}개 폴더, ${totalIndexed}개 파일 인덱싱 완료`
      : `${totalIndexed}개 파일 인덱싱 완료`;
    showToast(msg, "success");
  }
}
