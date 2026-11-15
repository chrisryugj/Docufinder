import { useEffect, useState } from "react";
import ReactDOM from "react-dom/client";
import "./harness.css";
import { ShortcutField } from "../src/components/settings/ShortcutField";

// ─── 창 불러오기 단축키 녹화 하니스 (#52) ─────────────────────────────────
// 설정 → 시스템의 ShortcutField 만 띄워 '직접 지정' 녹화를 실구동한다.
// 검증 스크립트: harness/verify-shortcut.mjs

declare global {
  interface Window {
    __value: string | null;
    __appKeys: number;
  }
}

// 앱 전역 키 처리(useKeyboardShortcuts — window keydown)를 흉내낸다. 녹화 중엔 한 번도 불리면 안 된다.
window.__appKeys = 0;
window.addEventListener("keydown", () => {
  window.__appKeys++;
});

function App() {
  const [value, setValue] = useState<string | null>("Ctrl+Alt+Space");
  useEffect(() => {
    window.__value = value;
  }, [value]);
  return (
    <div style={{ padding: 24, width: 460 }}>
      <ShortcutField value={value} onChange={setValue} />
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("root")!).render(<App />);
