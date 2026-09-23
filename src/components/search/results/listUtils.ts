import type { SearchResult } from "../../../types/search";
import type { FilenameSort } from "../../../hooks/useFilenameColumns";

/** 파일명 컬럼 정렬 비교자 (이름/경로/크기/수정일시/유형) */
export function compareFilename(a: SearchResult, b: SearchResult, sort: FilenameSort): number {
  const mul = sort.dir === "asc" ? 1 : -1;
  const ext = (r: SearchResult) => (r.file_name.split(".").pop() || "").toLowerCase();
  switch (sort.field) {
    case "name": return mul * a.file_name.localeCompare(b.file_name, "ko");
    case "path": return mul * a.file_path.localeCompare(b.file_path, "ko");
    case "size": return mul * ((a.size ?? 0) - (b.size ?? 0));
    case "time": return mul * ((a.modified_at ?? 0) - (b.modified_at ?? 0));
    case "type": return mul * (ext(a).localeCompare(ext(b)) || a.file_name.localeCompare(b.file_name, "ko"));
    default: return 0;
  }
}

// 컬럼 자동 맞춤용 텍스트 폭 측정 (canvas 재사용)
let measureCanvas: HTMLCanvasElement | null = null;
export function measureText(text: string, font: string): number {
  if (!measureCanvas) measureCanvas = document.createElement("canvas");
  const ctx = measureCanvas.getContext("2d");
  if (!ctx) return 0;
  ctx.font = font;
  return ctx.measureText(text).width;
}

/** 래퍼 더블클릭 열기에서 제외할 대상 — 자체 클릭 동작(버튼·토글·링크)이 있는 *내부* 요소.
 *  핸들러가 걸린 요소 자신(예: role=button인 파일명 매치 행)은 제외 대상이 아니다. */
export function isInteractiveTarget(e: React.MouseEvent): boolean {
  const hit = (e.target as HTMLElement).closest("button, [role='button'], a");
  return !!hit && hit !== e.currentTarget;
}

export function findScrollContainer(element: HTMLElement | null): HTMLElement | null {
  let current = element?.parentElement ?? null;

  while (current) {
    const { overflowY } = window.getComputedStyle(current);
    if (overflowY === "auto" || overflowY === "scroll") {
      return current;
    }
    current = current.parentElement;
  }

  return null;
}

export function getOffsetTopWithinContainer(element: HTMLElement, container: HTMLElement): number {
  const elementRect = element.getBoundingClientRect();
  const containerRect = container.getBoundingClientRect();
  return elementRect.top - containerRect.top;
}
