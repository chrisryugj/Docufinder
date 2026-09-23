import { Spinner } from "./Spinner";

/** 지연 로드 패널(미리보기·AI 질문)의 첫 로드 동안 빈 화면 대신 보이는 자리 표시 */
export function PanelLoading({ label }: { label: string }) {
  return (
    <div
      className="h-full min-h-[120px] flex items-center justify-center gap-2 text-sm"
      style={{ color: "var(--color-text-muted)", backgroundColor: "var(--color-bg-secondary)" }}
    >
      <Spinner size="sm" />
      <span>{label}</span>
    </div>
  );
}
