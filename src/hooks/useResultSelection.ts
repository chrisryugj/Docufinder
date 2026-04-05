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
  const prevResultsRef = useRef({ length: filteredResults.length, firstPath: filteredResults[0]?.file_path });

  // 결과가 변경되면 선택 초기화 (길이 + 첫 번째 결과 경로로 비교)
  useEffect(() => {
    const prev = prevResultsRef.current;
    const firstPath = filteredResults[0]?.file_path;
    if (prev.length !== filteredResults.length || prev.firstPath !== firstPath) {
      prevResultsRef.current = { length: filteredResults.length, firstPath };
      if (selectedIndex >= filteredResults.length) {
        setSelectedIndex(filteredResults.length > 0 ? 0 : -1);
      }
    }
  }, [filteredResults, selectedIndex]);

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
