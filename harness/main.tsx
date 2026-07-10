import { useState } from "react";
import ReactDOM from "react-dom/client";
import "./harness.css";
import { LayoutView } from "../src/components/search/LayoutView";
import { PdfLayoutView } from "../src/components/search/PdfLayoutView";
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
  }
}
window.__wheelPrevented = [];
window.__closed = false;
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

ReactDOM.createRoot(document.getElementById("root")!).render(<App />);
