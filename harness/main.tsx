import { useState } from "react";
import ReactDOM from "react-dom/client";
import "./harness.css";
import { LayoutView } from "../src/components/search/LayoutView";
import { PdfLayoutView } from "../src/components/search/PdfLayoutView";
import { SearchResultList } from "../src/components/search/SearchResultList";
import type { SearchResult, GroupedSearchResult, ViewMode } from "../src/types/search";
import type { ViewDensity } from "../src/types/settings";
import svgRaw from "./test-render.svg?raw";

// 컴포넌트 핸들러(타깃)가 처리한 뒤 버블 단계에서 defaultPrevented 를 기록 —
// passive onWheel 결함(v3.2.2/이번 PDF 수정)의 실효 검증 프로브.
declare global {
  interface Window {
    __wheelPrevented: boolean[];
    __closed: boolean;
    __setMode: (m: "svg" | "pdf" | "inline") => void;
    __setPdfFile: (p: string) => void;
    __invokeCalls: Array<{ cmd: string; args: unknown }>;
    __calls: Array<{ type: string; [k: string]: unknown }>;
    __setResultsCfg: (c: Partial<ResultsCfg>) => void;
  }
}

// ─── 검색 결과 리스트 하니스 (?view=results) ───────────────────────────
// 클릭 동작(한 번/두 번 클릭 열기)·경로 표시 설정의 실브라우저 검증용.

interface ResultsCfg {
  openOnSingleClick: boolean;
  showResultPath: boolean;
  density: ViewDensity;
  viewMode: ViewMode;
}

function fakeResult(over: Partial<SearchResult> & Pick<SearchResult, "file_path" | "file_name">): SearchResult {
  return {
    chunk_index: 0,
    content_preview:
      "2026년도 예산 편성 지침에 따라 각 부서는 사업별 예산 요구서를 기한 내 제출해야 한다. 예산 심의는 3월에 진행되며 조정 결과는 부서별로 통보된다.",
    score: 1,
    confidence: 88,
    match_type: "keyword",
    highlight_ranges: [[6, 8]],
    page_number: null,
    start_offset: 0,
    location_hint: null,
    modified_at: 1750000000,
    ...over,
  };
}

const RESULT_DEEP = fakeResult({
  file_path: "/Users/me/Documents/업무자료/2026년도/예산편성/부서별지침/최종확정본/예산지침.hwpx",
  file_name: "예산지침.hwpx",
});
const RESULT_WIN = fakeResult({
  file_path: "C:\\문서\\아주긴상위폴더명입니다정말로\\예산관리\\분기보고\\최종보고서.pdf",
  file_name: "최종보고서.pdf",
  page_number: 3,
  chunk_index: 1,
});
// 같은 이름 3곳 — FilenameCopiesBadge(≥3 접기) 경로까지 마운트
const FILENAME_RESULTS: SearchResult[] = [
  "/Users/me/Downloads/장기보관용자료함",
  "/Users/me/Desktop/백업",
  "/Users/me/Documents/공유",
].map((dir) =>
  fakeResult({ file_path: `${dir}/예산안.xlsx`, file_name: "예산안.xlsx", match_type: "filename" })
);
const GROUPED: GroupedSearchResult[] = [
  {
    file_path: RESULT_DEEP.file_path,
    file_name: RESULT_DEEP.file_name,
    chunks: [RESULT_DEEP, fakeResult({ ...RESULT_DEEP, chunk_index: 1, start_offset: 500 })],
    top_confidence: 88,
    total_matches: 2,
  },
  {
    file_path: RESULT_WIN.file_path,
    file_name: RESULT_WIN.file_name,
    chunks: [RESULT_WIN],
    top_confidence: 88,
    total_matches: 1,
  },
];

function ResultsHarness() {
  const [cfg, setCfg] = useState<ResultsCfg>({
    openOnSingleClick: false,
    showResultPath: true,
    density: "normal",
    viewMode: "flat",
  });
  window.__setResultsCfg = (c) => setCfg((prev) => ({ ...prev, ...c }));
  return (
    <div id="stage" style={{ width: 900, padding: 16 }}>
      <SearchResultList
        results={[RESULT_DEEP, RESULT_WIN]}
        filenameResults={FILENAME_RESULTS}
        groupedResults={GROUPED}
        viewMode={cfg.viewMode}
        viewDensity={cfg.density}
        query="예산"
        isLoading={false}
        onOpenFile={(p, page) => window.__calls.push({ type: "open", p, page })}
        onCopyPath={(p) => window.__calls.push({ type: "copy", p })}
        onOpenFolder={(p) => window.__calls.push({ type: "folder", p })}
        onSelectResult={(i) => window.__calls.push({ type: "select", i })}
        onPreviewFile={(p) => window.__calls.push({ type: "preview", p })}
        openOnSingleClick={cfg.openOnSingleClick}
        showResultPath={cfg.showResultPath}
      />
    </div>
  );
}
window.__wheelPrevented = [];
window.__closed = false;
window.__calls = [];
document.addEventListener("wheel", (e) => window.__wheelPrevented.push(e.defaultPrevented));

function App() {
  // 실앱은 팝업 뷰어가 한 번에 하나만 뜬다 — 키보드(window) 리스너 교차 방지 위해 단일 마운트
  const [mode, setMode] = useState<"svg" | "pdf" | "inline">("svg");
  const [pdfFile, setPdfFile] = useState("/fake/a.pdf");
  window.__setMode = setMode;
  window.__setPdfFile = setPdfFile;
  if (mode === "inline") {
    // 실앱 인라인 배치 재현 — PreviewPanel(flex-col h-full) 안에서 헤더·툴바·푸터와
    // 형제로 LayoutView(onExpand=인라인 모드)가 놓인다. 팝업(#stage 직속)과 달리
    // 위아래 크롬이 세로 공간을 나눠 갖는 flex 컨텍스트가 검증 대상.
    return (
      <div id="stage" style={{ width: 778, height: 720 }}>
        <div className="preview-panel flex flex-col h-full border-l">
          <div style={{ height: 37, flexShrink: 0, borderBottom: "1px solid #ddd" }}>헤더</div>
          <div style={{ height: 31, flexShrink: 0, borderBottom: "1px solid #ddd" }}>툴바</div>
          <LayoutView svg={svgRaw} onExpand={() => {}} />
          <div style={{ height: 27, flexShrink: 0, borderTop: "1px solid #ddd" }}>경로</div>
        </div>
      </div>
    );
  }
  return (
    <div id="stage" style={{ width: 800, height: 600 }}>
      {mode === "svg" ? (
        <LayoutView svg={svgRaw} onClose={() => { window.__closed = true; }} />
      ) : (
        <PdfLayoutView filePath={pdfFile} onClose={() => { window.__closed = true; }} />
      )}
    </div>
  );
}

const isResultsView = new URLSearchParams(location.search).get("view") === "results";
ReactDOM.createRoot(document.getElementById("root")!).render(isResultsView ? <ResultsHarness /> : <App />);
