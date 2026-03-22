import { useState, useEffect, useRef } from "react";
import type { SearchResult } from "../types/search";

interface UseResultSelectionReturn {
  selectedIndex: number;
  setSelectedIndex: React.Dispatch<React.SetStateAction<number>>;
}

/**
 * 검색 결과 선택 상태 + 미리보기 연동
 */
export function useResultSelection(
  filteredResults: SearchResult[],
  setPreviewFilePath: React.Dispatch<React.SetStateAction<string | null>>
): UseResultSelectionReturn {
  const [selectedIndex, setSelectedIndex] = useState<number>(-1);
  const prevResultsLength = useRef(filteredResults.length);

  // 결과가 변경되면 선택 초기화
  useEffect(() => {
    if (prevResultsLength.current !== filteredResults.length) {
      prevResultsLength.current = filteredResults.length;
      if (selectedIndex >= filteredResults.length) {
        setSelectedIndex(filteredResults.length > 0 ? 0 : -1);
      }
    }
  }, [filteredResults.length, selectedIndex]);

  // 선택된 결과 변경 시 미리보기 패널 업데이트
  useEffect(() => {
    if (selectedIndex >= 0 && selectedIndex < filteredResults.length) {
      setPreviewFilePath(filteredResults[selectedIndex].file_path);
    } else if (filteredResults.length === 0) {
      setPreviewFilePath(null);
    }
  }, [selectedIndex, filteredResults, setPreviewFilePath]);

  return { selectedIndex, setSelectedIndex };
}
