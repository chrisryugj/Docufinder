import { useCallback, useState } from "react";
import type { SearchResult } from "../types/search";

type ShowToastFn = (message: string, type: "success" | "error" | "loading" | "info") => string;

interface UseExportOptions {
  showToast?: ShowToastFn;
}

interface UseExportReturn {
  exportToCSV: (results: SearchResult[], query: string) => void;
  copyToClipboard: (results: SearchResult[], query: string) => Promise<void>;
  isExporting: boolean;
}

/**
 * 검색 결과 내보내기 훅
 */
export function useExport(options?: UseExportOptions): UseExportReturn {
  const [isExporting, setIsExporting] = useState(false);

  // 외부 showToast가 없으면 no-op (프로덕션에서 console.log 방지)
  const showToast = options?.showToast ?? ((_msg: string) => "");

  /**
   * CSV 내보내기 (다운로드)
   */
  const exportToCSV = useCallback(
    (results: SearchResult[], query: string) => {
      if (results.length === 0) {
        showToast("내보낼 결과가 없습니다", "error");
        return;
      }

      setIsExporting(true);

      try {
        // CSV 헤더
        const headers = ["파일명", "경로", "위치", "매칭내용", "점수"];

        // CSV 행 생성
        const rows = results.map((r) => [
          escapeCSV(r.file_name),
          escapeCSV(r.file_path),
          escapeCSV(r.location_hint || `청크 ${r.chunk_index}`),
          escapeCSV(r.content_preview.replace(/\n/g, " ")),
          r.score.toFixed(2),
        ]);

        // CSV 문자열 생성 (BOM 포함 - 한글 엑셀 호환)
        const BOM = "\uFEFF";
        const csvContent =
          BOM +
          [headers.join(","), ...rows.map((row) => row.join(","))].join("\n");

        // 다운로드
        const blob = new Blob([csvContent], {
          type: "text/csv;charset=utf-8;",
        });
        const url = URL.createObjectURL(blob);
        const link = document.createElement("a");
        link.href = url;

        const timestamp = new Date().toISOString().slice(0, 10);
        const safeQuery = query.replace(/[^a-zA-Z0-9가-힣]/g, "_").slice(0, 20);
        link.download = `Anything_${safeQuery}_${timestamp}.csv`;

        document.body.appendChild(link);
        link.click();
        document.body.removeChild(link);
        URL.revokeObjectURL(url);

        showToast(`${results.length}건 CSV 내보내기 완료`, "success");
      } catch (err) {
        console.error("CSV export failed:", err);
        showToast("CSV 내보내기 실패", "error");
      } finally {
        setIsExporting(false);
      }
    },
    [showToast]
  );

  /**
   * 클립보드 복사
   */
  const copyToClipboard = useCallback(
    async (results: SearchResult[], query: string) => {
      if (results.length === 0) {
        showToast("복사할 결과가 없습니다", "error");
        return;
      }

      setIsExporting(true);

      try {
        const lines: string[] = [
          "[Anything 검색 결과]",
          `검색어: "${query}"`,
          `결과: ${results.length}건`,
          "",
        ];

        results.forEach((r, i) => {
          const location = r.location_hint || `청크 ${r.chunk_index}`;
          lines.push(`${i + 1}. ${r.file_name} (${location})`);
          lines.push(`   경로: ${r.file_path}`);
          lines.push(
            `   내용: "${r.content_preview.slice(0, 100).replace(/\n/g, " ")}..."`
          );
          lines.push("");
        });

        await navigator.clipboard.writeText(lines.join("\n"));
        showToast(`${results.length}건 클립보드에 복사됨`, "success");
      } catch (err) {
        console.error("Clipboard copy failed:", err);
        showToast("클립보드 복사 실패", "error");
      } finally {
        setIsExporting(false);
      }
    },
    [showToast]
  );

  return {
    exportToCSV,
    copyToClipboard,
    isExporting,
  };
}

/**
 * CSV 이스케이프 (쌍따옴표, 쉼표, 줄바꿈 처리)
 */
function escapeCSV(value: string): string {
  if (value.includes(",") || value.includes('"') || value.includes("\n")) {
    return `"${value.replace(/"/g, '""')}"`;
  }
  return value;
}
