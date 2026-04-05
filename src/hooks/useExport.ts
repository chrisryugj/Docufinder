import { useCallback, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import type { SearchResult } from "../types/search";

type ShowToastFn = (message: string, type: "success" | "error" | "loading" | "info") => string;

interface UseExportOptions {
  showToast?: ShowToastFn;
}

interface UseExportReturn {
  exportToCSV: (results: SearchResult[], query: string) => Promise<void>;
  copyToClipboard: (results: SearchResult[], query: string) => Promise<void>;
  isExporting: boolean;
}

export function useExport(options?: UseExportOptions): UseExportReturn {
  const [isExporting, setIsExporting] = useState(false);
  const showToast = options?.showToast ?? ((_msg: string) => "");

  const exportToCSV = useCallback(
    async (results: SearchResult[], query: string) => {
      if (results.length === 0) {
        showToast("내보낼 결과가 없습니다", "error");
        return;
      }

      const timestamp = new Date().toISOString().slice(0, 10);
      const safeQuery = query.replace(/[^a-zA-Z0-9가-힣]/g, "_").slice(0, 20);

      const outputPath = await save({
        defaultPath: `Anything_${safeQuery}_${timestamp}.csv`,
        filters: [{ name: "CSV", extensions: ["csv"] }],
      });

      if (!outputPath) return;

      setIsExporting(true);
      try {
        const rows = results.map((r) => ({
          file_name: r.file_name,
          file_path: r.file_path,
          location_hint: r.location_hint || `청크 ${r.chunk_index}`,
          content_preview: r.content_preview.replace(/\n/g, " "),
          score: r.score,
          modified_at: r.modified_at ?? null,
        }));

        await invoke("export_csv", { rows, query, outputPath });
        showToast(`${results.length}건 CSV 내보내기 완료`, "success");
      } catch (e) {
        showToast(`CSV 내보내기 실패: ${e}`, "error");
      } finally {
        setIsExporting(false);
      }
    },
    [showToast]
  );

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
      } catch {
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
