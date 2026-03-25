import type { SearchParadigm } from "../../types/search";

interface Props {
  paradigm: SearchParadigm;
  onChange: (p: SearchParadigm) => void;
}

const modes: { value: SearchParadigm; label: string; icon: string }[] = [
  { value: "instant", label: "즉시", icon: "🔍" },
  { value: "natural", label: "자연어", icon: "💬" },
];

export default function SearchParadigmToggle({ paradigm, onChange }: Props) {
  return (
    <div className="inline-flex rounded-md bg-[var(--color-bg-tertiary)] p-0.5 flex-shrink-0">
      {modes.map((m) => (
        <button
          key={m.value}
          onClick={() => onChange(m.value)}
          className={`
            px-2 py-0.5 text-[11px] font-medium rounded transition-all duration-150
            ${paradigm === m.value
              ? "bg-[var(--color-accent)] text-white shadow-sm"
              : "text-[var(--color-text-muted)] hover:text-[var(--color-text-primary)]"
            }
          `}
          title={m.value === "instant" ? "실시간 키워드 검색" : "자연어로 질문하여 검색"}
        >
          <span className="mr-0.5">{m.icon}</span>
          {m.label}
        </button>
      ))}
    </div>
  );
}
