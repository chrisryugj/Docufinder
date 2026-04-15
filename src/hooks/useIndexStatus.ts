import { useState, useEffect, useCallback, useRef } from "react";
import { invokeWithTimeout, IPC_TIMEOUT } from "../utils/invokeWithTimeout";
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

interface SuggestedFolder {
  path: string;
  label: string;
  category: "known" | "drive";
  exists: boolean;
}

interface UseIndexStatusReturn {
  status: IndexStatus | null;
  isIndexing: boolean;
  progress: IndexingProgress | null;
  error: string | null;
  clearError: () => void;
  refreshStatus: () => Promise<void>;
  addFolder: () => Promise<AddFolderResult[] | null>;
  addFolderByPath: (path: string) => Promise<AddFolderResult | null>;
  removeFolder: (path: string) => Promise<void>;
  cancelIndexing: () => Promise<void>;
  /** 드라이브 경로 목록을 반환 (배치 시작은 IndexContext에서 수행) */
  getAllDrivePaths: () => Promise<string[]>;
  cancelledFolderPath: string | null;
  /** 배치 인덱싱 실행 중 여부 (FolderTree auto-resume 억제용) */
  isAutoIndexing: React.RefObject<boolean>;
}

/**
 * 인덱스 상태 관리 훅
 */
export function useIndexStatus(): UseIndexStatusReturn {
  const [status, setStatus] = useState<IndexStatus | null>(null);
  const [isIndexing, setIsIndexing] = useState(false);
  const [progress, setProgress] = useState<IndexingProgress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [cancelledFolderPath, setCancelledFolderPath] = useState<string | null>(null);
  const autoIndexingRef = useRef(false);

  const clearError = useCallback(() => setError(null), []);

  // 상태 조회
  const refreshStatus = useCallback(async () => {
    try {
      const result = await invokeWithTimeout<IndexStatus>("get_index_status", undefined, IPC_TIMEOUT.SETTINGS);
      setStatus(result);
    } catch {
      // 상태 조회 실패 시 무시 (다음 폴링에서 재시도)
    }
  }, []);

  // 진행률 이벤트 리스너
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;

    const setupListener = async () => {
      try {
        unlisten = await listen<IndexingProgress>("indexing-progress", (event) => {
          const p = event.payload;
          // 배치 인덱싱 중에는 단일 progress 무시 (DriveIndexingPanel이 담당)
          if (autoIndexingRef.current) {
            return;
          }
          setProgress(p);

          if (p.phase === "cancelled") {
            setCancelledFolderPath(p.folder_path);
          } else if (p.phase === "preparing" || p.phase === "scanning" || p.phase === "completed") {
            setCancelledFolderPath(null);
          }

          // 완료/취소 시 인덱싱 상태 업데이트
          if (p.phase === "completed" || p.phase === "cancelled") {
            setIsIndexing(false);
            // 잠시 후 진행률 초기화
            setTimeout(() => setProgress(null), 2000);
          }
        });
      } catch {
        // 리스너 등록 실패 — 진행률 표시 안 됨 (기능 저하)
      }
    };

    setupListener();

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  // folder-removed 이벤트 리스너 (백그라운드 삭제 완료/실패 알림)
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;

    const setup = async () => {
      try {
        unlisten = await listen<{ path: string; success: boolean; error?: string }>(
          "folder-removed",
          (event) => {
            const { success, path, error } = event.payload;
            if (success) {
              refreshStatus();
            } else {
              setError(`폴더 제거 실패 (${path}): ${error ?? "알 수 없는 오류"}`);
              refreshStatus(); // 실패해도 상태는 갱신
            }
          },
        );
      } catch {
        // 리스너 등록 실패
      }
    };

    setup();
    return () => { if (unlisten) unlisten(); };
  }, [refreshStatus]);

  // 초기 로드
  useEffect(() => {
    refreshStatus();
  }, [refreshStatus]);

  // 단일 경로 인덱싱 (내부 공통 로직)
  const indexSingleFolder = useCallback(async (path: string): Promise<AddFolderResult> => {
    const result = await invokeWithTimeout<AddFolderResult>("add_folder", {
      path,
    }, IPC_TIMEOUT.INDEXING);

    return result;
  }, []);

  // 폴더 추가 (다이얼로그, 다중 선택 지원)
  const addFolder = useCallback(async (): Promise<AddFolderResult[] | null> => {
    try {
      const selected = await open({
        directory: true,
        multiple: true,
        title: "인덱싱할 폴더 선택",
      });

      if (!selected) return null;

      const paths = Array.isArray(selected) ? selected : [selected];
      if (paths.length === 0) return null;

      // 드라이브 루트가 포함되어 있으면 경고 1회
      const hasDriveRoot = paths.some(isDriveRoot);
      if (hasDriveRoot) {
        const confirmed = await ask(
          "드라이브 전체를 인덱싱합니다.\n시스템 폴더(Windows, Program Files 등)는 자동 제외됩니다.\n\n계속하시겠습니까?",
          {
            title: "드라이브 전체 인덱싱",
            kind: "warning",
            okLabel: "계속",
            cancelLabel: "취소",
          }
        );
        if (!confirmed) return null;
      }

      setIsIndexing(true);
      setError(null);

      // 순차 처리 (DB 잠금 충돌 방지)
      const results: AddFolderResult[] = [];
      for (const path of paths) {
        try {
          const result = await indexSingleFolder(path);
          results.push(result);
        } catch (err) {
          results.push({
            success: false,
            indexed_count: 0,
            failed_count: 0,
            vectors_count: 0,
            message: err instanceof Error ? err.message : String(err),
            errors: [],
          });
        }
        await refreshStatus();
      }

      setIsIndexing(false);
      return results;
    } catch (err) {
      setError(`폴더 추가 실패: ${err instanceof Error ? err.message : String(err)}`);
      setIsIndexing(false);
      return null;
    }
  }, [refreshStatus, indexSingleFolder]);

  // 경로 직접 지정으로 폴더 추가 (추천 폴더 등에서 사용)
  const addFolderByPath = useCallback(async (path: string): Promise<AddFolderResult | null> => {
    try {
      if (isDriveRoot(path)) {
        const confirmed = await ask(
          "드라이브 전체를 인덱싱합니다.\n시스템 폴더(Windows, Program Files 등)는 자동 제외됩니다.\n\n계속하시겠습니까?",
          {
            title: "드라이브 전체 인덱싱",
            kind: "warning",
            okLabel: "계속",
            cancelLabel: "취소",
          }
        );
        if (!confirmed) return null;
      }

      setIsIndexing(true);
      setError(null);

      const result = await indexSingleFolder(path);
      await refreshStatus();
      setIsIndexing(false);

      return result;
    } catch (err) {
      setError(`폴더 추가 실패: ${err instanceof Error ? err.message : String(err)}`);
      setIsIndexing(false);
      return null;
    }
  }, [refreshStatus, indexSingleFolder]);

  // 폴더 제거 (즉시 반환, 백그라운드 삭제 — folder-removed 이벤트로 완료 알림)
  const removeFolder = useCallback(async (path: string): Promise<void> => {
    try {
      setError(null);
      await invokeWithTimeout("remove_folder", { path }, IPC_TIMEOUT.SETTINGS);
      // 즉시 반환됨 — optimistic UI 갱신
      await refreshStatus();
    } catch (err) {
      setError(`폴더 제거 실패: ${err instanceof Error ? err.message : String(err)}`);
    }
  }, [refreshStatus]);

  // 인덱싱 취소 (FTS)
  const cancelIndexing = useCallback(async (): Promise<void> => {
    try {
      await invokeWithTimeout("cancel_indexing", undefined, IPC_TIMEOUT.SETTINGS);
    } catch {
      // 취소 실패 무시
    }
  }, []);

  // 드라이브 경로 목록 조회 (배치 인덱싱 대상 선정용)
  const getAllDrivePaths = useCallback(async (): Promise<string[]> => {
    try {
      const folders = await invokeWithTimeout<SuggestedFolder[]>(
        "get_suggested_folders",
        undefined,
        IPC_TIMEOUT.SETTINGS,
      );
      return folders
        .filter((f) => f.category === "drive" && f.exists)
        .map((f) => f.path);
    } catch {
      return [];
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
    addFolderByPath,
    removeFolder,
    cancelIndexing,
    getAllDrivePaths,
    cancelledFolderPath,
    isAutoIndexing: autoIndexingRef,
  };
}
