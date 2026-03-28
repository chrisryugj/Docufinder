import React, { createContext, useContext, type ReactNode } from "react";
import { useIndexStatus, useVectorIndexing } from "../hooks";
import type { IndexStatus, AddFolderResult, IndexingProgress } from "../types/index";
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
    isIndexing,
    progress,
    error: indexError,
    clearError: clearIndexError,
    refreshStatus,
    addFolder,
    addFolderByPath,
    removeFolder,
    cancelIndexing,
    autoIndexAllDrives,
    cancelledFolderPath,
    isAutoIndexing,
  } = useIndexStatus();

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

  const value: IndexContextValue = {
    status,
    isIndexing,
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
  };

  return <IndexContext.Provider value={value}>{children}</IndexContext.Provider>;
}
