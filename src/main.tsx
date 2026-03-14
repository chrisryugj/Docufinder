import React, { useState } from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import AppV2 from "./AppV2";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { useTheme } from "./hooks/useTheme";
import "./index.css";

// V1/V2 토글을 포함한 Root 컴포넌트
function Root() {
  const [isV2, setIsV2] = useState(true); // 기본적으로 V2를 띄워 테스트 시작
  useTheme(); // V1/V2 전환 시 상태가 언마운트되더라도 루트에서 테마 클래스를 확실하게 관리하도록 호출

  return (
    <>
      {/* V1/V2 전환 플로팅 스위치 (운영 시 제거 예정) */}
      <div className="fixed bottom-4 right-4 z-50 flex items-center gap-2 px-3 py-2 bg-slate-800/80 backdrop-blur-md rounded-full shadow-lg border border-slate-700/50">
        <span className="text-xs font-medium text-slate-300">UI Version</span>
        <button
          onClick={() => setIsV2(false)}
          className={`px-3 py-1 text-xs font-bold rounded-full transition-all ${
            !isV2 
              ? "bg-slate-200 text-slate-900 shadow-sm" 
              : "text-slate-400 hover:text-white hover:bg-slate-700"
          }`}
        >
          V1
        </button>
        <button
          onClick={() => setIsV2(true)}
          className={`px-3 py-1 text-xs font-bold rounded-full transition-all ${
            isV2 
              ? "bg-blue-500 text-white shadow-sm shadow-blue-500/20" 
              : "text-slate-400 hover:text-white hover:bg-slate-700"
          }`}
        >
          V2 (New)
        </button>
      </div>

      <ErrorBoundary>
        {isV2 ? <AppV2 /> : <App />}
      </ErrorBoundary>
    </>
  );
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <Root />
  </React.StrictMode>
);
