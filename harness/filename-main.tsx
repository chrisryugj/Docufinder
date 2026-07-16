import { useState } from "react";
import ReactDOM from "react-dom/client";
import "./harness.css";
import { SearchResultList } from "../src/components/search/SearchResultList";
import type { SearchResult } from "../src/types/search";

// ─── 파일명 검색 컬럼 뷰 하니스 ─────────────────────────────────────────
// Everything식 컬럼(useFilenameColumns + FilenameColumnHeader + SearchResultItem
// 파일명 모드)의 창 폭 축소/재확대 레이아웃 검증용. 컨테이너 폭을 프로그래매틱하게
// 조절(window.__setStageWidth)하며 셀 bounding box 비중첩·선호 폭 복원을 계측한다.
// 검증 스크립트: harness/verify-filename-columns.mjs

declare global {
  interface Window {
    __setStageWidth: (px: number) => void;
    __calls: Array<{ type: string; [k: string]: unknown }>;
  }
}

const DIRS = [
  "/Users/me/Documents/업무자료/2026년도/예산편성/부서별지침/최종확정본",
  "/Users/me/Downloads/장기보관용자료함/아주긴하위폴더이름입니다정말로/한번더깊게",
  "C:\\문서\\아주긴상위폴더명입니다정말로\\예산관리\\분기보고",
  "/Users/me/Desktop/백업",
  "/Users/me/Documents/공유",
];
const NAMES = [
  "예산지침.hwpx",
  "2026년도_부서별_예산요구서_작성지침_최종확정본_v3.hwpx",
  "최종보고서.pdf",
  "분기실적.xlsx",
  "회의록.docx",
  "메모.txt",
  "발표자료.pptx",
  "예산안.xlsx",
];

function fakeFilenameResult(i: number): SearchResult {
  return {
    file_path: `${DIRS[i % DIRS.length]}/${i}_${NAMES[i % NAMES.length]}`,
    file_name: `${i}_${NAMES[i % NAMES.length]}`,
    chunk_index: 0,
    content_preview: "",
    score: 1,
    confidence: 0,
    match_type: "filename",
    highlight_ranges: [],
    page_number: null,
    start_offset: 0,
    location_hint: null,
    modified_at: 1750000000 - i * 86400,
    size: 1024 * (i + 1) * 37,
  };
}

const RESULTS: SearchResult[] = Array.from({ length: 20 }, (_, i) => fakeFilenameResult(i));

function FilenameHarness() {
  const [stageWidth, setStageWidth] = useState(1000);
  window.__setStageWidth = setStageWidth;
  return (
    <div id="stage" style={{ width: stageWidth }}>
      <SearchResultList
        results={RESULTS}
        viewMode="flat"
        query="예산"
        isLoading={false}
        searchMode="filename"
        onOpenFile={(p, page) => window.__calls.push({ type: "open", p, page })}
        onCopyPath={(p) => window.__calls.push({ type: "copy", p })}
        onOpenFolder={(p) => window.__calls.push({ type: "folder", p })}
        onSelectResult={(i) => window.__calls.push({ type: "select", i })}
      />
    </div>
  );
}

window.__calls = [];
ReactDOM.createRoot(document.getElementById("root")!).render(<FilenameHarness />);
