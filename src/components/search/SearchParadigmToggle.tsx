import type { SearchParadigm } from "../../types/search";

interface Props {
  paradigm: SearchParadigm;
  onChange: (p: SearchParadigm) => void;
}

// SVG 아이콘 — 이모지 대체 (플랫폼 일관성 + 디자인 토큰 제어)
const InstantIcon = () => (
  <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <circle cx="11" cy="11" r="8" />
    <path d="m21 21-4.35-4.35" />
  </svg>
);

const NaturalIcon = () => (
  <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M15 4V2M15 16v-2M8 9h2M20 9h2M17.8 11.8l1.4 1.4M17.8 6.2l1.4-1.4M12.2 6.2l-1.4-1.4M3 21l9-9" />
  </svg>
);

// Anything 아이콘 (별 + 스파클)
const AnythingIcon = () => (
  <svg width="11" height="11" viewBox="0 0 24 24" fill="currentColor" stroke="none">
    <path d="M12 2l2.4 6.4L21 11l-6.6 2.4L12 21l-2.4-7.6L3 11l6.6-2.4L12 2z" />
  </svg>
);

const modes: { value: SearchParadigm; label: string; Icon: React.ComponentType }[] = [
  { value: "instant", label: "검색", Icon: InstantIcon },
  { value: "natural", label: "스마트", Icon: NaturalIcon },
  { value: "question", label: "Anything", Icon: AnythingIcon },
];

export default function SearchParadigmToggle({ paradigm, onChange }: Props) {
  return (
    <div
      className="inline-flex rounded-md bg-[var(--color-bg-tertiary)] p-0.5 flex-shrink-0"
      role="radiogroup"
      aria-label="검색 패러다임 선택"
    >
      {modes.map((m) => {
        const isActive = paradigm === m.value;
        const isNaturalActive = m.value === "natural" && isActive;
        const isQuestionActive = m.value === "question" && isActive;
        const hasGradient = isNaturalActive || isQuestionActive;
        const desc = m.value === "instant"
          ? "키워드 검색"
          : m.value === "natural"
            ? "자연어 스마트 검색"
            : "AI 문서 분석";
        return (
          <button
            key={m.value}
            onClick={() => onChange(m.value)}
            role="radio"
            aria-checked={isActive}
            aria-label={`${m.label} — ${desc}`}
            className={`
              flex items-center gap-1 px-2 py-0.5 text-[11px] font-medium rounded transition-all duration-150
              ${isActive
                ? hasGradient
                  ? "text-white shadow-sm"
                  : "bg-[var(--color-accent)] text-white shadow-sm"
                : "text-[var(--color-text-muted)] hover:text-[var(--color-text-primary)]"
              }
            `}
            style={
              isNaturalActive ? {
                background: "linear-gradient(135deg, var(--color-accent) 0%, #059669 100%)",
                boxShadow: "0 1px 4px var(--color-accent-glow)",
              } : isQuestionActive ? {
                background: "linear-gradient(135deg, var(--color-accent-ai) 0%, var(--color-accent-ai-hover) 100%)",
                boxShadow: "0 1px 4px var(--color-accent-ai-glow)",
              } : undefined
            }
            title={desc}
          >
            <m.Icon />
            {m.label}
          </button>
        );
      })}
    </div>
  );
}
