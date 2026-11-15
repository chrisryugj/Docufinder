/**
 * 창 불러오기 전역 단축키 녹화 (#52).
 *
 * 저장 형식은 백엔드 global-hotkey 파서 형식 "Ctrl+Alt+KeyK" — 키 이름은 KeyboardEvent.code 를 그대로 쓴다
 * (파서가 KEYK·DIGIT5·BACKQUOTE·ARROWUP·NUMPAD7·F13 같은 code 이름을 대소문자 무시로 받는다).
 */

// 파서가 받는 키. Esc 는 녹화 취소 키라 뺀다. 한/영·한자(Lang1/2)·CapsLock 등은 파서가 모른다.
const ACCEPTED_CODE =
  /^(Key[A-Z]|Digit[0-9]|F([1-9]|1[0-9]|2[0-4])|Numpad([0-9]|Add|Decimal|Divide|Enter|Equal|Multiply|Subtract)|Arrow(Up|Down|Left|Right)|Space|Enter|Tab|Backspace|Delete|Insert|Home|End|PageUp|PageDown|Backquote|Backslash|BracketLeft|BracketRight|Comma|Period|Slash|Semicolon|Quote|Minus|Equal|Pause|PrintScreen|ScrollLock)$/;
const MODIFIER_CODE = /^(Control|Alt|Shift|Meta|OS)(Left|Right)?$/;

export type ShortcutKey = Pick<KeyboardEvent, "code" | "ctrlKey" | "altKey" | "shiftKey" | "metaKey">;

export type RecordResult =
  /** 수식키만 눌렀다 — 계속 기다린다. held 는 지금 누른 수식키("Ctrl+Alt"), 없으면 "" */
  | { kind: "modifier"; held: string }
  | { kind: "invalid"; reason: string }
  | { kind: "ok"; accelerator: string };

export function recordShortcut(e: ShortcutKey): RecordResult {
  const mods = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift", e.metaKey && "Super"].filter(
    (m): m is string => Boolean(m),
  );
  if (MODIFIER_CODE.test(e.code)) return { kind: "modifier", held: mods.join("+") };
  if (!ACCEPTED_CODE.test(e.code)) return { kind: "invalid", reason: "이 키는 단축키로 쓸 수 없습니다" };
  // 수식키 없이(또는 Shift 만) 걸면 어느 프로그램에서도 그 글자를 칠 수 없게 된다.
  if (!e.ctrlKey && !e.altKey && !e.metaKey) {
    return { kind: "invalid", reason: "Ctrl 이나 Alt 를 함께 눌러 주세요" };
  }
  return { kind: "ok", accelerator: [...mods, e.code].join("+") };
}

const KEY_LABEL: Record<string, string> = {
  Backquote: "`", Minus: "-", Equal: "=", BracketLeft: "[", BracketRight: "]", Backslash: "\\",
  Semicolon: ";", Quote: "'", Comma: ",", Period: ".", Slash: "/",
  ArrowUp: "↑", ArrowDown: "↓", ArrowLeft: "←", ArrowRight: "→",
};

/** "Ctrl+Alt+KeyK" → "Ctrl + Alt + K" (화면 표시용) */
export function formatShortcut(accelerator: string, mac: boolean): string {
  return accelerator
    .split("+")
    .map((t) => {
      if (t === "Super") return mac ? "⌘" : "Win";
      if (t === "Alt" && mac) return "Option";
      const letter = /^(?:Key|Digit)(.)$/.exec(t);
      if (letter) return letter[1];
      const numpad = /^Numpad(.+)$/.exec(t);
      if (numpad) return `Num ${numpad[1]}`;
      return KEY_LABEL[t] ?? t;
    })
    .join(" + ");
}
