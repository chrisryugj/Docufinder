import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

/** 사용자가 최근 연 문서 (홈 화면 "최근 작업한 문서"). value = last_opened_at(Unix sec). */
export interface RecentDocument {
  path: string;
  name: string;
  value: number;
}

/**
 * 사용자가 최근 연 문서 조회 (열람 신호 노출).
 * `enabled`가 false면 조회하지 않고 빈 배열을 반환한다(인덱스 없을 때).
 * `removeDoc`은 목록에서 항목 제거 — 낙관 갱신 후 백엔드 last_opened_at 클리어.
 */
export function useRecentDocuments(
  enabled: boolean,
  limit = 8,
): { docs: RecentDocument[]; removeDoc: (path: string) => void } {
  const [docs, setDocs] = useState<RecentDocument[]>([]);

  useEffect(() => {
    if (!enabled) {
      setDocs([]);
      return;
    }
    let cancelled = false;
    invoke<RecentDocument[]>("get_recently_opened_documents", { limit })
      .then((r) => { if (!cancelled) setDocs(r); })
      .catch(() => { if (!cancelled) setDocs([]); });
    return () => { cancelled = true; };
  }, [enabled, limit]);

  const removeDoc = useCallback((path: string) => {
    setDocs((prev) => prev.filter((d) => d.path !== path));
    // 실패해도 다음 로드에서 항목이 되살아나는 정도라 조용히 무시
    invoke("remove_recently_opened_document", { path }).catch(() => {});
  }, []);

  return { docs, removeDoc };
}
