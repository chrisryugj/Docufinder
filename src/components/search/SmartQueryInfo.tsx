import type { ParsedQueryInfo } from "../../types/search";

interface Props {
  parsed: ParsedQueryInfo;
  onClear: () => void;
}

/** 자연어 검색 파싱 결과를 칩으로 표시 */
export default function SmartQueryInfo({ parsed, onClear }: Props) {
  // 파싱된 게 없으면 (keywords === original) 표시 안 함
  const hasFilters =
    parsed.date_filter !== null ||
    parsed.file_type !== null ||
    parsed.exclude_keywords.length > 0;

  if (!hasFilters) return null;

  const chips: { label: string; icon: string }[] = [];

  if (parsed.date_filter) {
    const dateLabels: Record<string, string> = {
      Today: "오늘",
      ThisWeek: "이번 주",
      LastWeek: "지난 주",
      ThisMonth: "이번 달",
      LastMonth: "지난 달",
      ThisYear: "올해",
      Year: `${parsed.date_filter.value}년`,
      RecentDays: `최근 ${parsed.date_filter.value}일`,
    };
    chips.push({
      label: dateLabels[parsed.date_filter.type] || parsed.date_filter.type,
      icon: "📅",
    });
  }

  if (parsed.file_type) {
    const typeLabels: Record<string, string> = {
      hwpx: "한글",
      docx: "워드",
      xlsx: "엑셀",
      pdf: "PDF",
      txt: "텍스트",
      pptx: "파워포인트",
    };
    chips.push({
      label: typeLabels[parsed.file_type] || parsed.file_type,
      icon: "📄",
    });
  }

  for (const ex of parsed.exclude_keywords) {
    chips.push({ label: `제외: ${ex}`, icon: "🚫" });
  }

  return (
    <div className="flex items-center gap-2 flex-wrap px-1 py-1.5 text-xs">
      {parsed.keywords && (
        <span className="text-[var(--color-text-secondary)]">
          검색어: <span className="font-medium text-[var(--color-text-primary)]">{parsed.keywords}</span>
        </span>
      )}
      {chips.map((chip, i) => (
        <span
          key={i}
          className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-[var(--color-accent)]/10 text-[var(--color-accent)] border border-[var(--color-accent)]/20"
        >
          {chip.icon} {chip.label}
        </span>
      ))}
      <button
        onClick={onClear}
        className="text-[var(--color-text-tertiary)] hover:text-[var(--color-text-secondary)] ml-1"
        title="필터 초기화"
      >
        ✕
      </button>
    </div>
  );
}
