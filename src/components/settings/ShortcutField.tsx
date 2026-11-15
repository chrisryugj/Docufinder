import { useEffect, useRef, useState } from "react";
import { Dropdown } from "../ui/Dropdown";
import { Button } from "../ui/Button";
import { GLOBAL_SHORTCUT_OPTIONS } from "./tabs/types";
import { formatShortcut, recordShortcut } from "../../utils/shortcut";
import { isMac } from "../../utils/platform";

interface ShortcutFieldProps {
  value: string | null | undefined;
  onChange: (value: string | null) => void;
}

/** 창 불러오기 전역 단축키 — 프리셋 고르기 + 키를 눌러 직접 지정(녹화) (#52) */
export function ShortcutField({ value, onChange }: ShortcutFieldProps) {
  const [recording, setRecording] = useState(false);
  const [hint, setHint] = useState<string | null>(null);
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;

  const current = value ?? "";
  const options =
    current && !GLOBAL_SHORTCUT_OPTIONS.some((o) => o.value === current)
      ? [...GLOBAL_SHORTCUT_OPTIONS, { value: current, label: `${formatShortcut(current, isMac)} (직접 지정)` }]
      : GLOBAL_SHORTCUT_OPTIONS;

  useEffect(() => {
    if (!recording) return;
    // 캡처 단계에서 가로채 녹화 중엔 앱 단축키(Ctrl+K 등)와 Esc 설정 창 닫기가 먹지 않게 한다.
    const onKeyDown = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopImmediatePropagation();
      if (e.code === "Escape") {
        setRecording(false);
        return;
      }
      const r = recordShortcut(e);
      if (r.kind === "ok") {
        setRecording(false);
        onChangeRef.current(r.accelerator);
      } else if (r.kind === "modifier") {
        setHint(r.held ? `${formatShortcut(r.held, isMac)} + …` : null);
      } else {
        setHint(r.reason);
      }
    };
    const cancel = () => setRecording(false);
    window.addEventListener("keydown", onKeyDown, true);
    window.addEventListener("blur", cancel);
    return () => {
      window.removeEventListener("keydown", onKeyDown, true);
      window.removeEventListener("blur", cancel);
      setHint(null);
    };
  }, [recording]);

  return (
    <div>
      <label className="block text-sm font-medium mb-1" style={{ color: "var(--color-text-secondary)" }}>
        창 불러오기 단축키
      </label>
      <div className="flex gap-2">
        <div className="flex-1 min-w-0">
          <Dropdown
            options={options}
            value={current}
            onChange={(v) => onChange(v || null)}
            placeholder="단축키 선택"
          />
        </div>
        <Button variant="secondary" size="sm" onClick={() => setRecording((r) => !r)} aria-pressed={recording}>
          {recording ? "취소" : "직접 지정"}
        </Button>
      </div>
      {recording && (
        <p className="text-xs mt-1" role="status" style={{ color: "var(--color-accent)" }}>
          {hint ?? "쓰고 싶은 조합을 누르세요 (Esc: 취소)"}
        </p>
      )}
    </div>
  );
}
