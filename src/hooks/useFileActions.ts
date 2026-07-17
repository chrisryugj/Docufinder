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

  // query 는 키스트로크마다 바뀌므로 deps 에서 빼고 ref 로 최신값을 읽는다(P2-2).
  // 이렇게 해야 handleOpenFile 의 identity 가 안정적이라, 이를 prop 으로 받는
  // memo 컴포넌트(PreviewPanel — 문서 전체 재하이라이트 / 결과 아이템)가 타이핑 중
  // 불필요하게 재렌더되지 않는다.
  const queryRef = useRef(query);
  queryRef.current = query;

  const handleOpenFile = useCallback(
    async (filePath: string, page?: number | null) => {
      const trimmedQuery = queryRef.current.trim();
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
    [addSearch, showToast, updateToast]
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
        showToast("탐색기에서 열었습니다", "success");
      } catch {
        showToast("탐색기 열기 실패", "error");
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

  const handleOcrReindex = useCallback(
    async (filePath: string) => {
      const name = filePath.split(/[\\/]/).pop() ?? filePath;
      const toastId = showToast(
        `"${name}" OCR로 다시 읽는 중... (첫 사용 시 모델 다운로드로 오래 걸릴 수 있어요)`,
        "loading"
      );
      try {
        const res = await invokeWithTimeout<{ message?: string }>(
          "reindex_file",
          { path: filePath },
          IPC_TIMEOUT.OCR_REINDEX
        );
        updateToast(toastId, {
          message: res?.message ?? "OCR 재인덱싱 완료 — 다시 검색하면 반영됩니다",
          type: "success",
        });
        invalidateSearch();
      } catch (e) {
        updateToast(toastId, {
          message: `OCR 재인덱싱 실패: ${e instanceof Error ? e.message : String(e)}`,
          type: "error",
        });
      }
    },
    [showToast, updateToast, invalidateSearch]
  );

  return {
    handleOpenFile,
    handleCopyPath,
    handleOpenFolder,
    handleAddFolder,
    handleAddFolderByPath,
    handleRemoveFolder,
    handleOcrReindex,
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
