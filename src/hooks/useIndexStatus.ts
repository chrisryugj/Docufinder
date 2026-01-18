import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { IndexStatus, AddFolderResult, IndexingProgress } from "../types/index";
import { open, ask } from "@tauri-apps/plugin-dialog";

/**
 * 드라이브 루트 경로인지 확인 (Windows)
 * 예: "C:\", "D:\", "\\?\C:\"
 */
function isDriveRoot(path: string): boolean {
  // 정규화
  const normalized = path.replace(/\\\\\?\\/, "").replace(/\//g, "\\");
  // C:\, D:\ 패턴
  return /^[A-Za-z]:\\?$/.test(normalized);
}

interface UseIndexStatusReturn {
  status: IndexStatus | null;
  isIndexing: boolean;
  progress: IndexingProgress | null;
  error: string | null;
  clearError: () => void;
  refreshStatus: () => Promise<void>;
  addFolder: () => Promise<AddFolderResult | null>;
  removeFolder: (path: string) => Promise<void>;
  cancelIndexing: () => Promise<void>;
}

/**
 * 인덱스 상태 관리 훅
 */
export function useIndexStatus(): UseIndexStatusReturn {
  const [status, setStatus] = useState<IndexStatus | null>(null);
  const [isIndexing, setIsIndexing] = useState(false);
  const [progress, setProgress] = useState<IndexingProgress | null>(null);
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

  // 진행률 이벤트 리스너
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;

    const setupListener = async () => {
      unlisten = await listen<IndexingProgress>("indexing-progress", (event) => {
        const p = event.payload;
        setProgress(p);

        // 완료/취소 시 인덱싱 상태 업데이트
        if (p.phase === "completed" || p.phase === "cancelled") {
          setIsIndexing(false);
          // 잠시 후 진행률 초기화
          setTimeout(() => setProgress(null), 2000);
        }
      });
    };

    setupListener();

    return () => {
      if (unlisten) unlisten();
    };
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
        // 드라이브 루트 경고
        if (isDriveRoot(selected)) {
          const confirmed = await ask(
            "전체 드라이브 인덱싱은 시간이 오래 걸릴 수 있습니다.\n(수천~수만 개의 파일을 처리합니다)\n\n계속하시겠습니까?",
            {
              title: "드라이브 전체 인덱싱",
              kind: "warning",
              okLabel: "계속",
              cancelLabel: "취소",
            }
          );

          if (!confirmed) {
            return null;
          }
        }

        setIsIndexing(true);
        setError(null);

        const result = await invoke<AddFolderResult>("add_folder", {
          path: selected,
        });

        console.log("Indexing result:", result);

        // 실패한 파일 에러 로그 출력
        if (result.errors && result.errors.length > 0) {
          console.warn(`Indexing errors (${result.errors.length}):`);
          result.errors.slice(0, 20).forEach((err, i) => {
            console.warn(`  ${i + 1}: ${err}`);
          });
          if (result.errors.length > 20) {
            console.warn(`  ... and ${result.errors.length - 20} more errors`);
          }
        }

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

  // 인덱싱 취소
  const cancelIndexing = useCallback(async (): Promise<void> => {
    try {
      await invoke("cancel_indexing");
    } catch (err) {
      console.error("Failed to cancel indexing:", err);
    }
  }, []);

  return {
    status,
    isIndexing,
    progress,
    error,
    clearError,
    refreshStatus,
    addFolder,
    removeFolder,
    cancelIndexing,
  };
}
