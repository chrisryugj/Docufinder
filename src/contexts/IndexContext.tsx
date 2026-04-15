import React, { createContext, useCallback, useContext, useEffect, useMemo, useRef, type ReactNode } from "react";
import { useIndexStatus, useIndexingBatch, useVectorIndexing } from "../hooks";
import type { IndexStatus, AddFolderResult, IndexingProgress, BatchState } from "../types/index";
import type { VectorIndexingStatus } from "../types/index";

// ── Types ──────────────────────────────────────────────

export interface IndexContextValue {
  // FTS 인덱스 상태
  status: IndexStatus | null;
  isIndexing: boolean;
  progress: IndexingProgress | null;
  indexError: string | null;
  clearIndexError: () => void;
  refreshStatus: () => Promise<void>;
  addFolder: () => Promise<AddFolderResult[] | null>;
  addFolderByPath: (path: string) => Promise<AddFolderResult | null>;
  removeFolder: (path: string) => Promise<void>;
  cancelIndexing: () => Promise<void>;
  autoIndexAllDrives: () => Promise<void>;
  cancelledFolderPath: string | null;
  isAutoIndexing: React.RefObject<boolean>;

  // 배치 인덱싱 (멀티 드라이브/폴더)
  batch: BatchState | null;
  isBatchRunning: boolean;
  cancelBatch: () => Promise<void>;
  dismissBatch: () => void;

  // 벡터 인덱싱
  vectorStatus: VectorIndexingStatus | null;
  vectorProgress: number;
  vectorJustCompleted: boolean;
  clearVectorCompleted: () => void;
  refreshVectorStatus: () => Promise<VectorIndexingStatus | null>;
  cancelVectorIndexing: () => Promise<void>;
  startVectorIndexing: () => Promise<void>;
  isVectorIndexing: boolean;
  vectorError: string | null;
  clearVectorError: () => void;
}

// ── Context ────────────────────────────────────────────

const IndexContext = createContext<IndexContextValue | null>(null);

export function useIndexContext(): IndexContextValue {
  const ctx = useContext(IndexContext);
  if (!ctx) throw new Error("useIndexContext must be used within IndexProvider");
  return ctx;
}

// ── Provider ───────────────────────────────────────────

export function IndexProvider({ children }: { children: ReactNode }) {
  const {
    status,
    isIndexing: fsIsIndexing,
    progress,
    error: indexError,
    clearError: clearIndexError,
    refreshStatus,
    addFolder,
    addFolderByPath,
    removeFolder,
    cancelIndexing,
    getAllDrivePaths,
    cancelledFolderPath,
    isAutoIndexing,
  } = useIndexStatus();

  const {
    batch,
    isRunning: isBatchRunning,
    startBatch,
    cancelBatch,
    dismissBatch,
  } = useIndexingBatch();

  // isAutoIndexing ref 동기화: 배치 실행 중 단일 progress 이벤트 억제용
  useEffect(() => {
    isAutoIndexing.current = isBatchRunning;
  }, [isBatchRunning, isAutoIndexing]);

  // 배치 종료 시 refreshStatus (폴더 목록/통계 갱신)
  const prevRunningRef = useRef(false);
  useEffect(() => {
    if (prevRunningRef.current && !isBatchRunning) {
      refreshStatus();
    }
    prevRunningRef.current = isBatchRunning;
  }, [isBatchRunning, refreshStatus]);

  // autoIndexAllDrives: 드라이브 경로 조회 → 배치 시작
  const autoIndexAllDrives = useCallback(async () => {
    const paths = await getAllDrivePaths();
    if (paths.length === 0) return;
    await startBatch(paths);
  }, [getAllDrivePaths, startBatch]);

  const {
    status: vectorStatus,
    progress: vectorProgress,
    justCompleted: vectorJustCompleted,
    clearCompleted: clearVectorCompleted,
    refreshStatus: refreshVectorStatus,
    cancel: cancelVectorIndexing,
    startManual: startVectorIndexing,
    isRunning: isVectorIndexing,
    error: vectorError,
    clearError: clearVectorError,
  } = useVectorIndexing();

  const value = useMemo<IndexContextValue>(
    () => ({
      status,
      isIndexing: fsIsIndexing || isBatchRunning,
      progress,
      indexError,
      clearIndexError,
      refreshStatus,
      addFolder,
      addFolderByPath,
      removeFolder,
      cancelIndexing,
      autoIndexAllDrives,
      cancelledFolderPath,
      isAutoIndexing,
      batch,
      isBatchRunning,
      cancelBatch,
      dismissBatch,
      vectorStatus,
      vectorProgress,
      vectorJustCompleted,
      clearVectorCompleted,
      refreshVectorStatus,
      cancelVectorIndexing,
      startVectorIndexing,
      isVectorIndexing,
      vectorError,
      clearVectorError,
    }),
    [
      status, fsIsIndexing, isBatchRunning, progress, indexError, clearIndexError, refreshStatus,
      addFolder, addFolderByPath, removeFolder, cancelIndexing, autoIndexAllDrives,
      cancelledFolderPath, isAutoIndexing, batch, cancelBatch, dismissBatch,
      vectorStatus, vectorProgress, vectorJustCompleted, clearVectorCompleted,
      refreshVectorStatus, cancelVectorIndexing, startVectorIndexing, isVectorIndexing,
      vectorError, clearVectorError,
    ],
  );

  return <IndexContext.Provider value={value}>{children}</IndexContext.Provider>;
}
