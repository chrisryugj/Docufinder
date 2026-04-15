import { useCallback, useEffect, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invokeWithTimeout, IPC_TIMEOUT } from "../utils/invokeWithTimeout";
import type {
  BatchJob,
  BatchJobProgressPayload,
  BatchState,
} from "../types/index";

interface UseIndexingBatchReturn {
  batch: BatchState | null;
  isRunning: boolean;
  startBatch: (paths: string[]) => Promise<string | null>;
  cancelBatch: () => Promise<void>;
  dismissBatch: () => void;
}

/**
 * 배치 인덱싱 상태 관리 훅
 *
 * - Rust의 `start_indexing_batch` 커맨드로 여러 폴더를 순차 인덱싱
 * - 이벤트 스트림(`indexing-batch-*`)으로 상태 업데이트
 * - 동일 세션 내 모달 재진입 시 `get_indexing_batch`로 상태 복구
 */
export function useIndexingBatch(): UseIndexingBatchReturn {
  const [batch, setBatch] = useState<BatchState | null>(null);

  // 마운트 시 실행 중인 배치만 복구 (완료된 배치는 다시 띄우지 않음)
  useEffect(() => {
    invokeWithTimeout<BatchState | null>(
      "get_indexing_batch",
      undefined,
      IPC_TIMEOUT.SETTINGS,
    )
      .then((b) => {
        if (b && b.is_running) setBatch(b);
      })
      .catch(() => {});
  }, []);

  // 이벤트 리스너
  useEffect(() => {
    const unlistens: UnlistenFn[] = [];
    let cancelled = false;

    const setup = async () => {
      try {
        const u1 = await listen<BatchState>("indexing-batch-started", (e) => {
          setBatch(e.payload);
        });
        const u2 = await listen<BatchJobProgressPayload>(
          "indexing-batch-job-progress",
          (e) => {
            const p = e.payload;
            setBatch((prev) => {
              if (!prev || prev.batch_id !== p.batch_id) return prev;
              const jobs = prev.jobs.slice();
              const existing = jobs[p.job_index];
              if (!existing) return prev;
              const updated: BatchJob = {
                ...existing,
                status: p.status,
                stage: p.stage,
                processed: p.processed,
                total: p.total,
                current_file: p.current_file,
                indexed_count: p.indexed_count,
                failed_count: p.failed_count,
                error: p.error,
              };
              jobs[p.job_index] = updated;
              return { ...prev, jobs, current_index: p.job_index };
            });
          },
        );
        const u3 = await listen<BatchState | null>(
          "indexing-batch-completed",
          (e) => {
            if (e.payload) {
              setBatch(e.payload);
            } else {
              setBatch((prev) =>
                prev ? { ...prev, is_running: false } : prev,
              );
            }
          },
        );
        if (cancelled) {
          u1();
          u2();
          u3();
        } else {
          unlistens.push(u1, u2, u3);
        }
      } catch {
        // 리스너 등록 실패 — 기능 저하
      }
    };

    setup();
    return () => {
      cancelled = true;
      unlistens.forEach((u) => u());
    };
  }, []);

  const startBatch = useCallback(
    async (paths: string[]): Promise<string | null> => {
      if (paths.length === 0) return null;
      try {
        const id = await invokeWithTimeout<string>(
          "start_indexing_batch",
          { paths },
          IPC_TIMEOUT.SETTINGS,
        );
        return id;
      } catch {
        return null;
      }
    },
    [],
  );

  const cancelBatch = useCallback(async () => {
    try {
      await invokeWithTimeout(
        "cancel_indexing_batch",
        undefined,
        IPC_TIMEOUT.SETTINGS,
      );
    } catch {
      // 무시
    }
  }, []);

  const dismissBatch = useCallback(() => {
    setBatch(null);
  }, []);

  return {
    batch,
    isRunning: batch?.is_running ?? false,
    startBatch,
    cancelBatch,
    dismissBatch,
  };
}
