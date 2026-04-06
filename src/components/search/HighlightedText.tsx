import { memo, useMemo } from "react";
import { extractSearchKeywords, findKeywordRanges, parseSnippetHighlights } from "./searchTextUtils";

interface HighlightedTextProps {
  text: string;
  ranges: [number, number][];
  /** FTS5 snippet (마커 포함) - 있으면 ranges 대신 이걸로 파싱 */
  snippet?: string;
  /** 결과 내 검색 키워드 (추가 하이라이트) */
  refineKeywords?: string[];
  /** 검색 쿼리 (snippet 없을 때 폴백 하이라이트) */
  searchQuery?: string;
  /** 미리보기/확장 뷰에 따른 포맷 모드 */
  formatMode?: "preview" | "full";
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

/**
 * 텍스트를 가독성 좋게 줄바꿈 처리하여 React 노드로 변환
 * - `...` (FTS5 생략 기호)
 * - `. ` 뒤 한글/대문자 시작 (문장 끝)
 * - 목록 구분자 (○, ●)
 */
/**
 * Preview 모드용 텍스트 정제 (마크다운/서식 아티팩트 제거)
 */
function cleanForPreview(text: string): string {
  return text
    .replace(/#{1,6}\s*/g, "")           // ## 헤더 → 제거
    .replace(/\*\*([^*]*)\*\*/g, "$1")   // **bold** → bold
    .replace(/\*\*/g, "")               // 남은 ** 마커
    .replace(/(^|\s)\*\s+/g, "$1")      // * 불릿 마커
    .replace(/\s\|\s/g, " ")            // | 표 구분자
    .replace(/&\d{4,5};/g, " ")         // raw HTML entities (&8228; 등)
    .replace(/\s{2,}/g, " ")            // 다중 공백 축소
    .trim();
}

/**
 * Preview 모드: ... 세퍼레이터를 시각적 구분자(·)로 변환하여 렌더링
 */
function renderPreviewSegments(text: string, keyPrefix: string): React.ReactNode[] {
  const cleaned = cleanForPreview(text);
  const segments = cleaned.split(/\.{3,}/);
  const result: React.ReactNode[] = [];

  segments.forEach((seg, idx) => {
    const trimmed = seg.trim();
    if (!trimmed) return;
    if (result.length > 0) {
      result.push(
        <span key={`${keyPrefix}-sep-${idx}`} className="snippet-sep" aria-hidden="true"> · </span>
      );
    }
    result.push(trimmed);
  });

  return result.length > 0 ? result : cleaned ? [cleaned] : [];
}

function formatAndRender(text: string, keyPrefix: string, mode: "preview" | "full"): React.ReactNode[] {
  if (mode === "preview") {
    return renderPreviewSegments(text, keyPrefix);
  }

  const formatted = text
    // 문장 끝: `. ` 뒤에 한글/대문자 시작이면 줄바꿈 (길이 유지)
    .replace(/\. +(?=[가-힣A-Z○●■□▶▷])/g, ".\n")
    // 목록 구분자 앞에서 줄바꿈 (길이 유지)
    .replace(/ ○ /g, "\n○ ")
    .replace(/ ● /g, "\n● ");

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
export const HighlightedText = memo(function HighlightedText({
  text,
  ranges,
  snippet,
  refineKeywords,
  searchQuery,
  formatMode = "full",
}: HighlightedTextProps) {
  // snippet 파싱 + 범위 계산을 메모이제이션
  const { actualText, effectiveSearchRanges, refineRanges } = useMemo(() => {
    // snippet에 [[HL]] 마커가 있을 때만 snippet 파싱, 없으면 text 사용
    const useSnippet = snippet && snippet.includes('[[HL]]');
    const parsed = useSnippet
      ? parseSnippetHighlights(snippet)
      : { text: text ?? '', ranges: ranges ?? [] };
    // preview 모드: 줄바꿈→공백 (1:1 치환으로 range 오프셋 유지)
    // line-clamp 안에서 하이라이트가 잘리는 문제 방지
    const at = formatMode === "preview"
      ? parsed.text.replace(/[\r\n]/g, " ")
      : parsed.text;
    const ar = parsed.ranges;

    const keywords = searchQuery ? extractSearchKeywords(searchQuery) : [];
    const searchQueryRanges = keywords.length > 0
      ? findKeywordRanges(at, keywords)
      : [];

    const rr = refineKeywords && refineKeywords.length > 0
      ? findKeywordRanges(at, refineKeywords)
      : [];

    const esr = searchQueryRanges.length > 0
      ? mergeOverlappingRanges(searchQueryRanges)
      : mergeOverlappingRanges(ar);

    return { actualText: at, effectiveSearchRanges: esr, refineRanges: rr };
  }, [text, ranges, snippet, refineKeywords, searchQuery, formatMode]);

  // 하이라이트 없으면 포매팅만 적용
  if (effectiveSearchRanges.length === 0 && refineRanges.length === 0) {
    return <>{formatAndRender(actualText, "plain", formatMode)}</>;
  }

  // 두 범위 병합
  const mergedRanges = mergeRanges(effectiveSearchRanges, refineRanges);
  const parts: React.ReactNode[] = [];
  let lastIndex = 0;

  mergedRanges.forEach(({ start, end, type }, i) => {
    // 하이라이트 전 텍스트 (포매팅 적용)
    if (start > lastIndex) {
      parts.push(...formatAndRender(actualText.slice(lastIndex, start), `pre-${i}`, formatMode));
    }
    // 하이라이트 텍스트 - 타입에 따라 클래스 다르게
    const hlClass = type === 'both' ? 'hl-both' : type === 'refine' ? 'hl-refine' : 'hl-search';

    parts.push(
      <mark key={`mark-${i}`} className={hlClass}>
        {actualText.slice(start, end)}
      </mark>
    );
    lastIndex = end;
  });

  // 마지막 남은 텍스트 (포매팅 적용)
  if (lastIndex < actualText.length) {
    parts.push(...formatAndRender(actualText.slice(lastIndex), "post", formatMode));
  }

  return <>{parts}</>;
});
