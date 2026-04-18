import { useState, useEffect, useRef, useCallback } from "react";
import type { SearchResult } from "../types/search";

interface UseResultSelectionReturn {
  selectedIndex: number;
  setSelectedIndex: (i: number) => void;
}

/**
 * 검색 결과 선택 상태 + 미리보기 연동
 *
 * ## 재발 방지 원칙 (중요)
 * 과거 리팩토링마다 재발한 버그: 인덱싱 중 결과가 refresh되면 selectedIndex가 유지된 채
 * 다른 파일을 가리키게 되어 "클릭 안 했는데 프리뷰 자동 전환" 발생.
 *
 * 따라서 여기선 **선택된 파일 경로**를 source of truth로 삼고, 결과 변경 시
 * path 기반으로 index를 재매핑한다. selectedIndex는 UI 동기화용 파생 상태.
 *
 * 불변식:
 *   1. 사용자가 명시적으로 선택(setSelectedIndex)하기 전엔 프리뷰 자동 전환 없음
 *   2. 결과 refresh로 동일 path의 index가 바뀌면 selectedIndex만 조용히 갱신 (프리뷰 유지)
 *   3. 선택한 path가 결과에서 사라지면 -1로 해제 (프리뷰 닫힘)
 */
export function useResultSelection(
  filteredResults: SearchResult[],
  setPreviewFilePath: React.Dispatch<React.SetStateAction<string | null>>
): UseResultSelectionReturn {
  const [selectedIndex, setSelectedIndexRaw] = useState<number>(-1);
  // source of truth: 사용자가 선택한 파일 경로
  const selectedPathRef = useRef<string | null>(null);

  const setSelectedIndex = useCallback(
    (i: number) => {
      setSelectedIndexRaw(i);
      selectedPathRef.current =
        i >= 0 && i < filteredResults.length ? filteredResults[i].file_path : null;
    },
    [filteredResults]
  );

  // 결과 refresh 시 선택 path를 기준으로 index 재매핑 — 프리뷰 자동 전환 방지
  useEffect(() => {
    const path = selectedPathRef.current;
    if (path === null) {
      if (selectedIndex !== -1) setSelectedIndexRaw(-1);
      return;
    }
    const newIdx = filteredResults.findIndex((r) => r.file_path === path);
    if (newIdx === -1) {
      selectedPathRef.current = null;
      setSelectedIndexRaw(-1);
    } else if (newIdx !== selectedIndex) {
      setSelectedIndexRaw(newIdx);
    }
  }, [filteredResults, selectedIndex]);

  // 프리뷰는 **선택 path**에만 반응 — filteredResults 변경은 무시
  useEffect(() => {
    setPreviewFilePath(selectedPathRef.current);
  }, [selectedIndex, setPreviewFilePath]);

  return { selectedIndex, setSelectedIndex };
}
