import { Lock } from "lucide-react";

interface PasswordGateProps {
  value: string;
  onChange: (value: string) => void;
  onSubmit: () => void;
  unlocking: boolean;
  /** 입력한 암호가 틀렸다 (한 번이라도 시도한 뒤) */
  wrong: boolean;
}

/** 열기 암호 입력: 암호로 보호된 문서 (kordoc v4.4.0+, HWPX·HWP3·HWP5) */
export function PasswordGate({ value, onChange, onSubmit, unlocking, wrong }: PasswordGateProps) {
  return (
    <div className="p-4 flex flex-col items-center gap-3">
      <Lock size={20} className="opacity-60 text-[var(--color-text-muted)]" />
      <p className="text-sm text-center text-[var(--color-text-secondary)]">
        암호로 보호된 문서입니다.
        <br />
        열기 암호를 넣으면 내용을 볼 수 있어요.
      </p>
      <form
        className="flex items-center gap-1.5 w-full max-w-xs"
        onSubmit={(e) => {
          e.preventDefault();
          onSubmit();
        }}
      >
        <input
          type="password"
          value={value}
          autoFocus
          disabled={unlocking}
          onChange={(e) => onChange(e.target.value)}
          placeholder="열기 암호"
          aria-label="문서 열기 암호"
          aria-invalid={wrong}
          className="flex-1 px-2 py-1.5 text-sm rounded outline-none bg-[var(--color-bg-primary)] text-[var(--color-text-primary)] border"
          style={{ borderColor: wrong ? "var(--color-error)" : "var(--color-border)" }}
        />
        <button
          type="submit"
          disabled={!value || unlocking}
          className="px-3 py-1.5 text-sm rounded shrink-0 bg-[var(--color-accent)] text-[var(--color-on-accent)] disabled:opacity-50"
        >
          {unlocking ? "여는 중" : "열기"}
        </button>
      </form>
      {wrong && (
        <p className="text-xs text-[var(--color-error)]" role="alert">암호가 맞지 않습니다. 다시 확인해 주세요.</p>
      )}
      <p className="text-xs text-[var(--color-text-muted)]">
        암호는 이 문서를 여는 데만 쓰고 저장하지 않습니다.
      </p>
    </div>
  );
}
