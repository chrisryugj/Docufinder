interface HighlightedTextProps {
  text: string;
  ranges: [number, number][];
}

/**
 * 하이라이트 범위가 적용된 텍스트 렌더링
 */
export function HighlightedText({ text, ranges }: HighlightedTextProps) {
  if (!ranges || ranges.length === 0) {
    return <>{text}</>;
  }

  // 범위 정렬 (시작 위치 기준)
  const sortedRanges = [...ranges].sort((a, b) => a[0] - b[0]);
  const parts: React.ReactNode[] = [];
  let lastIndex = 0;

  sortedRanges.forEach(([start, end], i) => {
    // 하이라이트 전 텍스트
    if (start > lastIndex) {
      parts.push(text.slice(lastIndex, start));
    }
    // 하이라이트 텍스트
    parts.push(
      <mark
        key={i}
        className="bg-yellow-500/30 text-yellow-200 rounded px-0.5"
      >
        {text.slice(start, end)}
      </mark>
    );
    lastIndex = end;
  });

  // 마지막 남은 텍스트
  if (lastIndex < text.length) {
    parts.push(text.slice(lastIndex));
  }

  return <>{parts}</>;
}
