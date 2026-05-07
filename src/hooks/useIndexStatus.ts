import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { invokeWithTimeout, IPC_TIMEOUT } from "../utils/invokeWithTimeout";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { IndexStatus, AddFolderResult, IndexingProgress } from "../types/index";
import { getErrorMessage } from "../types/error";
import { open, ask } from "@tauri-apps/plugin-dialog";
import { SYSTEM_FOLDERS_HINT } from "../utils/platform";

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

type LocationKind = "local" | "unc" | "network_drive" | "cloud_placeholder";
interface FolderClassification {
  kind: LocationKind;
  skip_body_enabled: boolean;
  is_system: boolean;
  allow_system_enabled: boolean;
}

/**
 * 폴더 추가 사전 분류 + 안내 다이얼로그.
 *
 * 우선순위:
 *  1. 시스템 폴더 + 토글 OFF → 안내 후 차단 (false 반환)
 *  2. 시스템 폴더 + 토글 ON → 강한 경고 (계속/취소)
 *  3. 클라우드/네트워크 → 본문 스킵 토글에 따른 안내 (계속/취소)
 *  4. 로컬 → 통과
 */
async function confirmFolderAdd(path: string): Promise<boolean> {
  let info: FolderClassification;
  try {
    info = await invoke<FolderClassification>("classify_folder", { path });
  } catch {
    return true; // 분류 실패 시 차단하지 않음 (백엔드 validate_watch_path 가 최종 게이트)
  }

  // 1·2. 시스템 폴더 분기
  if (info.is_system) {
    if (!info.allow_system_enabled) {
      await ask(
        `이 폴더는 시스템 보호 폴더입니다.\n\n시스템 폴더(C:\\Windows · Program Files · /System · /usr/bin 등)는 기본 차단됩니다.\n\n수동으로 인덱싱하려면 [설정 → 시스템 → 시스템 폴더 추가 허용] 토글을 켜고 다시 시도하세요.`,
        { title: "시스템 폴더 차단됨", kind: "info", okLabel: "확인", cancelLabel: "닫기" },
      );
      return false;
    }
    return await ask(
      `⚠️ 이 폴더는 시스템 보호 폴더입니다.\n\n수십만 개의 시스템/바이너리 파일이 인덱싱돼 다음 영향이 있을 수 있습니다:\n• 디스크/메모리 사용량 급증\n• 파일명 검색 결과 노이즈 증가\n• 인덱싱 시간 매우 길어짐\n\n시맨틱(벡터) 검색은 자동 시작되지 않습니다.\n\n계속하시겠습니까?`,
      { title: "시스템 폴더 추가", kind: "warning", okLabel: "계속", cancelLabel: "취소" },
    );
  }

  // 3·4. 클라우드/네트워크 분기 (기존 로직)
  if (info.kind === "local") return true;

  const labelMap: Record<LocationKind, string> = {
    local: "로컬",
    unc: "네트워크(UNC) 공유 폴더",
    network_drive: "매핑된 네트워크 드라이브",
    cloud_placeholder: "클라우드 동기화 폴더",
  };
  const label = labelMap[info.kind];

  const message = info.skip_body_enabled
    ? `이 폴더는 ${label}로 감지되었습니다.\n\n현재 설정에 따라 **본문은 인덱싱하지 않고 파일명·크기·수정일만** 저장됩니다 (파일명 검색은 가능).\n\n본문까지 인덱싱하려면 [설정 → 시스템 → 클라우드/네트워크 폴더 본문 인덱싱 자동 스킵] 토글을 끄세요. (느려질 수 있음)\n\n계속하시겠습니까?`
    : `이 폴더는 ${label}로 감지되었습니다.\n\n현재 본문 인덱싱이 켜져 있어 모든 파일을 네트워크/클라우드에서 다운로드합니다 — 매우 느려질 수 있습니다.\n\n계속하시겠습니까?`;

  return await ask(message, {
    title: `${label} 추가`,
    kind: "warning",
    okLabel: "계속",
    cancelLabel: "취소",
  });
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
  // 인덱싱은 폴더 크기에 따라 수분~수십분 소요 가능 → 타임아웃 없이 raw invoke 사용
  // hang 감지는 indexing-progress 이벤트 침묵으로 별도 판단
  const indexSingleFolder = useCallback(async (path: string): Promise<AddFolderResult> => {
    return await invoke<AddFolderResult>("add_folder", { path });
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
          `드라이브 전체를 인덱싱합니다.\n시스템 폴더(${SYSTEM_FOLDERS_HINT})는 자동 제외됩니다.\n\n계속하시겠습니까?`,
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
        // 시스템/클라우드/네트워크 사전 안내 (Cancel 시 해당 폴더만 스킵)
        if (!(await confirmFolderAdd(path))) {
          continue;
        }
        try {
          const result = await indexSingleFolder(path);
          results.push(result);
        } catch (err) {
          results.push({
            success: false,
            indexed_count: 0,
            failed_count: 0,
            vectors_count: 0,
            message: getErrorMessage(err),
            errors: [],
          });
        }
        await refreshStatus();
      }

      setIsIndexing(false);
      return results;
    } catch (err) {
      setError(`폴더 추가 실패: ${getErrorMessage(err)}`);
      setIsIndexing(false);
      return null;
    }
  }, [refreshStatus, indexSingleFolder]);

  // 경로 직접 지정으로 폴더 추가 (추천 폴더 등에서 사용)
  const addFolderByPath = useCallback(async (path: string): Promise<AddFolderResult | null> => {
    try {
      if (isDriveRoot(path)) {
        const confirmed = await ask(
          `드라이브 전체를 인덱싱합니다.\n시스템 폴더(${SYSTEM_FOLDERS_HINT})는 자동 제외됩니다.\n\n계속하시겠습니까?`,
          {
            title: "드라이브 전체 인덱싱",
            kind: "warning",
            okLabel: "계속",
            cancelLabel: "취소",
          }
        );
        if (!confirmed) return null;
      }

      if (!(await confirmFolderAdd(path))) return null;

      setIsIndexing(true);
      setError(null);

      const result = await indexSingleFolder(path);
      await refreshStatus();
      setIsIndexing(false);

      return result;
    } catch (err) {
      setError(`폴더 추가 실패: ${getErrorMessage(err)}`);
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
      setError(`폴더 제거 실패: ${getErrorMessage(err)}`);
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
