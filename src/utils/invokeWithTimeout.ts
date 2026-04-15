import { invoke } from "@tauri-apps/api/core";

/** IPC 타임아웃 기본값 (ms) */
// 인덱싱은 폴더 크기에 따라 수분~수십분 소요 가능 → 타임아웃 사용 금지 (raw invoke 사용)
export const IPC_TIMEOUT = {
  SEARCH: 30_000,
  FILE_ACTION: 5_000,
  SETTINGS: 10_000,
} as const;

class IpcTimeoutError extends Error {
  constructor(command: string, timeoutMs: number) {
    super(`IPC 타임아웃: ${command} (${timeoutMs / 1000}초 초과)`);
    this.name = "IpcTimeoutError";
  }
}

/**
 * Tauri invoke에 타임아웃을 추가한 래퍼
 * 백엔드 hang 시 무한 대기 방지
 */
export async function invokeWithTimeout<T>(
  command: string,
  args?: Record<string, unknown>,
  timeoutMs: number = IPC_TIMEOUT.SETTINGS,
): Promise<T> {
  let timeoutId: ReturnType<typeof setTimeout>;
  const timeoutPromise = new Promise<never>((_, reject) => {
    timeoutId = setTimeout(() => reject(new IpcTimeoutError(command, timeoutMs)), timeoutMs);
  });
  try {
    return await Promise.race([
      args ? invoke<T>(command, args) : invoke<T>(command),
      timeoutPromise,
    ]);
  } finally {
    clearTimeout(timeoutId!);
  }
}
