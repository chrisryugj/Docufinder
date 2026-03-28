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
    <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
  </svg>
);

const modes: { value: SearchParadigm; label: string; Icon: React.ComponentType }[] = [
  { value: "instant", label: "즉시", Icon: InstantIcon },
  { value: "natural", label: "자연어", Icon: NaturalIcon },
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
        const desc = m.value === "instant" ? "실시간 키워드 검색" : "자연어로 질문하여 검색";
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
                ? isNaturalActive
                  ? "text-white shadow-sm"
                  : "bg-[var(--color-accent)] text-white shadow-sm"
                : "text-[var(--color-text-muted)] hover:text-[var(--color-text-primary)]"
              }
            `}
            style={isNaturalActive ? {
              background: "linear-gradient(135deg, var(--color-accent) 0%, #059669 100%)",
              boxShadow: "0 1px 4px var(--color-accent-glow)",
            } : undefined}
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
