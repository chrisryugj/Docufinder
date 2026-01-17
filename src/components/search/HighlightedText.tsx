interface HighlightedTextProps {
  text: string;
  ranges: [number, number][];
  /** FTS5 snippet (마커 포함) - 있으면 ranges 대신 이걸로 파싱 */
  snippet?: string;
}

/**
 * FTS5 snippet에서 하이라이트 마커 파싱 (JavaScript 문자열 인덱스 기준)
 * [[HL]]매칭[[/HL]] 형식 → { text, ranges }
 */
function parseSnippetHighlights(snippet: string): { text: string; ranges: [number, number][] } {
  const ranges: [number, number][] = [];
  let text = '';
  let i = 0;

  while (i < snippet.length) {
    if (snippet.slice(i, i + 6) === '[[HL]]') {
      const start = text.length;
      i += 6; // [[HL]] 건너뛰기

      // [[/HL]] 찾기
      const endMarker = snippet.indexOf('[[/HL]]', i);
      if (endMarker !== -1) {
        text += snippet.slice(i, endMarker);
        ranges.push([start, text.length]);
        i = endMarker + 7; // [[/HL]] 건너뛰기
      } else {
        // 닫는 마커 없으면 나머지 전부 하이라이트
        text += snippet.slice(i);
        ranges.push([start, text.length]);
        break;
      }
    } else {
      text += snippet[i];
      i++;
    }
  }

  return { text, ranges };
}

/**
 * 텍스트를 가독성 좋게 줄바꿈 처리하여 React 노드로 변환
 * - `...` (FTS5 생략 기호)
 * - `. ` 뒤 한글/대문자 시작 (문장 끝)
 * - 목록 구분자 (○, ●)
 */
function formatAndRender(text: string, keyPrefix: string): React.ReactNode[] {
  // 줄바꿈 패턴 적용
  const formatted = text
    // FTS5 생략 기호
    .replace(/\.{3}/g, "\n")
    // 문장 끝: `. ` 뒤에 한글이나 대문자가 오면 줄바꿈
    .replace(/\. +(?=[가-힣A-Z○●■□▶▷])/g, ".\n")
    // 목록 구분자 앞에서 줄바꿈
    .replace(/ ○ /g, "\n○ ")
    .replace(/ ● /g, "\n● ")
    // 괄호 닫고 화살표 패턴 (예: "예산) ->")
    .replace(/\) *-> */g, ")\n→ ")
    // 연속 공백 정리
    .replace(/ {2,}/g, " ");

  // 줄바꿈으로 분리하여 React 노드로 변환
  const lines = formatted.split("\n");
  const result: React.ReactNode[] = [];

  lines.forEach((line, idx) => {
    if (idx > 0) {
      result.push(<br key={`${keyPrefix}-br-${idx}`} />);
    }
    if (line) {
      result.push(line);
    }
  });

  return result;
}

/**
 * 하이라이트 범위가 적용된 텍스트 렌더링
 * - 가독성을 위한 줄바꿈 처리 포함
 * - snippet이 있으면 JavaScript에서 직접 파싱 (Rust 바이트 인덱스 버그 회피)
 */
export function HighlightedText({ text, ranges, snippet }: HighlightedTextProps) {
  // snippet이 있으면 JavaScript에서 직접 파싱 (더 정확함)
  const { actualText, actualRanges } = snippet
    ? (() => {
        const parsed = parseSnippetHighlights(snippet);
        return { actualText: parsed.text, actualRanges: parsed.ranges };
      })()
    : { actualText: text, actualRanges: ranges };

  // 하이라이트 없으면 포매팅만 적용
  if (!actualRanges || actualRanges.length === 0) {
    return <>{formatAndRender(actualText, "plain")}</>;
  }

  // 범위 정렬 (시작 위치 기준)
  const sortedRanges = [...actualRanges].sort((a, b) => a[0] - b[0]);
  const parts: React.ReactNode[] = [];
  let lastIndex = 0;

  sortedRanges.forEach(([start, end], i) => {
    // 하이라이트 전 텍스트 (포매팅 적용)
    if (start > lastIndex) {
      parts.push(...formatAndRender(actualText.slice(lastIndex, start), `pre-${i}`));
    }
    // 하이라이트 텍스트 - CSS 변수 기반 테마 대응
    parts.push(
      <mark
        key={`mark-${i}`}
        style={{
          backgroundColor: "var(--color-highlight-bg)",
          color: "var(--color-highlight-text)",
          borderRadius: "2px",
          padding: "0 2px",
        }}
      >
        {actualText.slice(start, end)}
      </mark>
    );
    lastIndex = end;
  });

  // 마지막 남은 텍스트 (포매팅 적용)
  if (lastIndex < actualText.length) {
    parts.push(...formatAndRender(actualText.slice(lastIndex), "post"));
  }

  return <>{parts}</>;
}
