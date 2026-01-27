import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { VectorIndexingStatus, VectorIndexingProgress } from "../types/index";

interface UseVectorIndexingReturn {
  /** 벡터 인덱싱 상태 */
  status: VectorIndexingStatus | null;
  /** 진행률 (0-100) */
  progress: number;
  /** 완료 여부 (토스트 표시용) */
  justCompleted: boolean;
  /** 완료 플래그 리셋 */
  clearCompleted: () => void;
  /** 상태 새로고침 */
  refreshStatus: () => Promise<void>;
  /** 취소 */
  cancel: () => Promise<void>;
  /** 수동 벡터 인덱싱 시작 */
  startManual: () => Promise<void>;
  /** 실행 중 여부 */
  isRunning: boolean;
}

/**
 * 벡터 인덱싱 상태 관리 훅
 * - 백그라운드 벡터 인덱싱 진행률 추적
 * - 완료 시 토스트 표시용 플래그
 */
export function useVectorIndexing(): UseVectorIndexingReturn {
  const [status, setStatus] = useState<VectorIndexingStatus | null>(null);
  const [justCompleted, setJustCompleted] = useState(false);

  // 진행률 계산
  const progress = status?.total_chunks
    ? Math.round((status.processed_chunks / status.total_chunks) * 100)
    : 0;

  const clearCompleted = useCallback(() => setJustCompleted(false), []);

  // 상태 조회
  const refreshStatus = useCallback(async () => {
    try {
      const result = await invoke<VectorIndexingStatus>("get_vector_indexing_status");
      setStatus(result);
    } catch (err) {
      console.error("Failed to get vector indexing status:", err);
    }
  }, []);

  // 취소
  const cancel = useCallback(async () => {
    try {
      await invoke("cancel_vector_indexing");
    } catch (err) {
      console.error("Failed to cancel vector indexing:", err);
    }
  }, []);

  // 수동 시작
  const startManual = useCallback(async () => {
    try {
      await invoke("start_vector_indexing");
      await refreshStatus();
    } catch (err) {
      console.error("Failed to start vector indexing:", err);
    }
  }, [refreshStatus]);

  // 진행률 이벤트 리스너
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;

    const setupListener = async () => {
      unlisten = await listen<VectorIndexingProgress>("vector-indexing-progress", (event) => {
        const p = event.payload;

        setStatus({
          is_running: !p.is_complete,
          total_chunks: p.total_chunks,
          processed_chunks: p.processed_chunks,
          pending_chunks: Math.max(p.total_chunks - p.processed_chunks, 0),
          current_file: p.current_file,
          error: null,
        });

        // 완료 시 플래그 설정
        if (p.is_complete) {
          setJustCompleted(true);
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

  const isRunning = status?.is_running ?? false;

  return {
    status,
    progress,
    justCompleted,
    clearCompleted,
    refreshStatus,
    cancel,
    startManual,
    isRunning,
  };
}
