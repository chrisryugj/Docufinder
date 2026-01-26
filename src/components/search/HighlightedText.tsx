interface HighlightedTextProps {
  text: string;
  ranges: [number, number][];
  /** FTS5 snippet (마커 포함) - 있으면 ranges 대신 이걸로 파싱 */
  snippet?: string;
  /** 결과 내 검색 키워드 (추가 하이라이트) */
  refineKeywords?: string[];
  /** 검색 쿼리 (snippet 없을 때 폴백 하이라이트) */
  searchQuery?: string;
}

/**
 * 검색 쿼리에서 키워드 추출 (공백 분리, 빈 문자열 제외)
 */
function extractSearchKeywords(query: string): string[] {
  return query
    .split(/\s+/)
    .map(s => s.trim())
    .filter(s => s.length > 0);
}

/**
 * FTS5 snippet에서 하이라이트 마커 파싱 (JavaScript 문자열 인덱스 기준)
 * [[HL]]매칭[[/HL]] 형식 → { text, ranges }
 */
/**
 * 텍스트에서 키워드 위치 찾기 (대소문자 무시)
 */
function findKeywordRanges(text: string, keywords: string[]): [number, number][] {
  const ranges: [number, number][] = [];
  const lowerText = text.toLowerCase();

  for (const keyword of keywords) {
    const lowerKeyword = keyword.toLowerCase();
    let index = 0;
    while ((index = lowerText.indexOf(lowerKeyword, index)) !== -1) {
      ranges.push([index, index + keyword.length]);
      index += keyword.length;
    }
  }

  // 시작 위치로 정렬
  return ranges.sort((a, b) => a[0] - b[0]);
}

/**
 * 겹치는 범위 병합 (FTS 마커 + searchQuery 결과 병합용)
 */
function mergeOverlappingRanges(ranges: [number, number][]): [number, number][] {
  if (ranges.length === 0) return [];

  // 시작 위치로 정렬
  const sorted = [...ranges].sort((a, b) => a[0] - b[0]);
  const merged: [number, number][] = [sorted[0]];

  for (let i = 1; i < sorted.length; i++) {
    const last = merged[merged.length - 1];
    const current = sorted[i];

    // 겹치거나 인접하면 병합
    if (current[0] <= last[1]) {
      last[1] = Math.max(last[1], current[1]);
    } else {
      merged.push(current);
    }
  }

  return merged;
}

function parseSnippetHighlights(snippet: string): { text: string; ranges: [number, number][] } {
  // 세그먼트별로 분리 후 하이라이트 포함 세그먼트 우선 정렬
  const segments = snippet.split('...');

  // 하이라이트 포함 여부로 분류
  const withHighlight: string[] = [];
  const withoutHighlight: string[] = [];

  for (const seg of segments) {
    const trimmed = seg.trim();
    if (!trimmed) continue;
    if (trimmed.includes('[[HL]]')) {
      withHighlight.push(trimmed);
    } else {
      withoutHighlight.push(trimmed);
    }
  }

  // 하이라이트 있는 세그먼트 먼저, 그 다음 나머지
  const reordered = [...withHighlight, ...withoutHighlight];
  const joinedSnippet = reordered.join('...');

  // 기존 파싱 로직
  const ranges: [number, number][] = [];
  let text = '';
  let i = 0;

  while (i < joinedSnippet.length) {
    if (joinedSnippet.slice(i, i + 6) === '[[HL]]') {
      const start = text.length;
      i += 6; // [[HL]] 건너뛰기

      // [[/HL]] 찾기
      const endMarker = joinedSnippet.indexOf('[[/HL]]', i);
      if (endMarker !== -1) {
        text += joinedSnippet.slice(i, endMarker);
        ranges.push([start, text.length]);
        i = endMarker + 7; // [[/HL]] 건너뛰기
      } else {
        // 닫는 마커 없으면 나머지 전부 하이라이트
        text += joinedSnippet.slice(i);
        ranges.push([start, text.length]);
        break;
      }
    } else {
      text += joinedSnippet[i];
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
 * 두 범위 배열 병합 (겹치는 부분은 타입 표시)
 * 반환: { start, end, type: 'search' | 'refine' | 'both' }[]
 */
function mergeRanges(
  searchRanges: [number, number][],
  refineRanges: [number, number][]
): { start: number; end: number; type: 'search' | 'refine' | 'both' }[] {
  const events: { pos: number; delta: number; source: 'search' | 'refine' }[] = [];

  for (const [s, e] of searchRanges) {
    events.push({ pos: s, delta: 1, source: 'search' });
    events.push({ pos: e, delta: -1, source: 'search' });
  }
  for (const [s, e] of refineRanges) {
    events.push({ pos: s, delta: 1, source: 'refine' });
    events.push({ pos: e, delta: -1, source: 'refine' });
  }

  // 위치순, 같으면 시작(delta=1)이 먼저
  events.sort((a, b) => a.pos - b.pos || b.delta - a.delta);

  const result: { start: number; end: number; type: 'search' | 'refine' | 'both' }[] = [];
  let searchCount = 0;
  let refineCount = 0;
  let lastPos = 0;

  for (const { pos, delta, source } of events) {
    if (pos > lastPos && (searchCount > 0 || refineCount > 0)) {
      const type = searchCount > 0 && refineCount > 0 ? 'both' :
                   searchCount > 0 ? 'search' : 'refine';
      result.push({ start: lastPos, end: pos, type });
    }
    if (source === 'search') searchCount += delta;
    else refineCount += delta;
    lastPos = pos;
  }

  return result;
}

/**
 * 하이라이트 범위가 적용된 텍스트 렌더링
 * - 가독성을 위한 줄바꿈 처리 포함
 * - snippet이 있으면 JavaScript에서 직접 파싱 (Rust 바이트 인덱스 버그 회피)
 * - refineKeywords가 있으면 추가 하이라이트 (다른 색상)
 */
export function HighlightedText({ text, ranges, snippet, refineKeywords, searchQuery }: HighlightedTextProps) {
  // snippet이 있으면 JavaScript에서 직접 파싱 (더 정확함)
  const { actualText, actualRanges } = snippet
    ? (() => {
        const parsed = parseSnippetHighlights(snippet);
        return { actualText: parsed.text, actualRanges: parsed.ranges };
      })()
    : { actualText: text, actualRanges: ranges };

  // searchQuery로 추가 하이라이트 범위 계산
  // FTS5 snippet 마커가 누락될 수 있으므로 항상 searchQuery로도 찾기
  const searchQueryRanges = searchQuery
    ? findKeywordRanges(actualText, extractSearchKeywords(searchQuery))
    : [];

  // 결과 내 검색 키워드 범위 계산
  const refineRanges = refineKeywords && refineKeywords.length > 0
    ? findKeywordRanges(actualText, refineKeywords)
    : [];

  // 검색 하이라이트 = FTS ranges + searchQuery 병합 (FTS 마커 누락 보완)
  const effectiveSearchRanges = mergeOverlappingRanges([...actualRanges, ...searchQueryRanges]);

  // 하이라이트 없으면 포매팅만 적용
  if (effectiveSearchRanges.length === 0 && refineRanges.length === 0) {
    return <>{formatAndRender(actualText, "plain")}</>;
  }

  // 두 범위 병합
  const mergedRanges = mergeRanges(effectiveSearchRanges, refineRanges);
  const parts: React.ReactNode[] = [];
  let lastIndex = 0;

  mergedRanges.forEach(({ start, end, type }, i) => {
    // 하이라이트 전 텍스트 (포매팅 적용)
    if (start > lastIndex) {
      parts.push(...formatAndRender(actualText.slice(lastIndex, start), `pre-${i}`));
    }
    // 하이라이트 텍스트 - 타입에 따라 색상 다르게
    const bgColor = type === 'refine' || type === 'both'
      ? "var(--color-highlight-refine-bg)"
      : "var(--color-highlight-bg)";
    const textColor = type === 'refine' || type === 'both'
      ? "var(--color-highlight-refine-text)"
      : "var(--color-highlight-text)";

    parts.push(
      <mark
        key={`mark-${i}`}
        style={{
          backgroundColor: bgColor,
          color: textColor,
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
