export type TextRange = [number, number];

const DEFAULT_CONTEXT_BEFORE = 40;
const DEFAULT_CONTEXT_AFTER = 140;

export function extractSearchKeywords(query: string): string[] {
  return query
    .split(/\s+/)
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
}

export function findKeywordRanges(text: string, keywords: string[]): TextRange[] {
  if (!text || keywords.length === 0) return [];
  const ranges: TextRange[] = [];
  const lowerText = text.toLowerCase();

  for (const keyword of keywords) {
    const lowerKeyword = keyword.toLowerCase();
    let index = 0;
    while ((index = lowerText.indexOf(lowerKeyword, index)) !== -1) {
      ranges.push([index, index + keyword.length]);
      index += keyword.length;
    }
  }

  return ranges.sort((a, b) => a[0] - b[0]);
}

export function parseSnippetHighlights(snippet: string): { text: string; ranges: TextRange[] } {
  const segments = snippet.split("...");
  const withHighlight: string[] = [];
  const withoutHighlight: string[] = [];

  for (const seg of segments) {
    const trimmed = seg.trim();
    if (!trimmed) continue;
    if (trimmed.includes("[[HL]]")) {
      withHighlight.push(trimmed);
    } else {
      withoutHighlight.push(trimmed);
    }
  }

  const joinedSnippet = [...withHighlight, ...withoutHighlight].join("...");
  const ranges: TextRange[] = [];
  let text = "";
  let i = 0;

  while (i < joinedSnippet.length) {
    if (joinedSnippet.slice(i, i + 6) === "[[HL]]") {
      const start = text.length;
      i += 6;
      const endMarker = joinedSnippet.indexOf("[[/HL]]", i);
      if (endMarker !== -1) {
        text += joinedSnippet.slice(i, endMarker);
        ranges.push([start, text.length]);
        i = endMarker + 7;
      } else {
        text += joinedSnippet.slice(i);
        ranges.push([start, text.length]);
        break;
      }
    } else {
      text += joinedSnippet[i];
      i += 1;
    }
  }

  return { text, ranges };
}

function normalizeRanges(ranges: TextRange[], textLength: number): TextRange[] {
  return ranges
    .map(([start, end]) => [Math.max(0, start), Math.min(textLength, end)] as TextRange)
    .filter(([start, end]) => start < end && start < textLength)
    .sort((a, b) => a[0] - b[0]);
}

function buildContextWindow(
  text: string,
  ranges: TextRange[],
  contextBefore: number,
  contextAfter: number
): { text: string; ranges: TextRange[] } {
  if (!text) return { text: "", ranges: [] };
  const normalized = normalizeRanges(ranges, text.length);
  if (normalized.length === 0) {
    return { text, ranges: [] };
  }

  const [anchorStart, anchorEnd] = normalized[0];
  const start = Math.max(0, anchorStart - contextBefore);
  const end = Math.min(text.length, anchorEnd + contextAfter);

  const clippedText = text.slice(start, end);
  const prefix = start > 0 ? "..." : "";
  const suffix = end < text.length ? "..." : "";
  const prefixLen = prefix.length;

  const adjustedRanges = normalized
    .filter(([s, e]) => e > start && s < end)
    .map(([s, e]) => [
      Math.max(s, start) - start + prefixLen,
      Math.min(e, end) - start + prefixLen,
    ] as TextRange);

  return { text: prefix + clippedText + suffix, ranges: adjustedRanges };
}

export function getPreviewWithKeyword(
  preview: string,
  fullContent: string,
  query: string,
  contextBefore = DEFAULT_CONTEXT_BEFORE,
  contextAfter = DEFAULT_CONTEXT_AFTER
): string {
  if (!query || !fullContent) return preview ?? "";
  const keywords = extractSearchKeywords(query);
  if (keywords.length === 0) return preview ?? "";

  const lowerPreview = (preview ?? "").toLowerCase();
  if (keywords.some((kw) => lowerPreview.includes(kw.toLowerCase()))) {
    return preview;
  }

  const lowerFull = fullContent.toLowerCase();
  let matchIdx = -1;
  let matchLen = 0;
  for (const kw of keywords) {
    const idx = lowerFull.indexOf(kw.toLowerCase());
    if (idx !== -1 && (matchIdx === -1 || idx < matchIdx)) {
      matchIdx = idx;
      matchLen = kw.length;
    }
  }

  if (matchIdx === -1) return preview ?? "";

  const start = Math.max(0, matchIdx - contextBefore);
  const end = Math.min(fullContent.length, matchIdx + matchLen + contextAfter);
  const excerpt = fullContent.slice(start, end);

  return (start > 0 ? "..." : "") + excerpt + (end < fullContent.length ? "..." : "");
}

export function buildPreviewContext(input: {
  previewText?: string;
  fullText?: string;
  highlightRanges?: TextRange[];
  snippet?: string | null;
  query?: string;
  contextBefore?: number;
  contextAfter?: number;
}): { text: string; ranges: TextRange[] } {
  const previewText = input.previewText ?? "";
  const fullText = input.fullText ?? previewText;
  const contextBefore = input.contextBefore ?? DEFAULT_CONTEXT_BEFORE;
  const contextAfter = input.contextAfter ?? DEFAULT_CONTEXT_AFTER;
  const keywords = input.query ? extractSearchKeywords(input.query) : [];

  // snippet 파싱 (있으면)
  const parsed = input.snippet?.includes("[[HL]]")
    ? parseSnippetHighlights(input.snippet)
    : null;

  // 1) snippet 텍스트에서 검색어 직접 찾기 (최우선 - 가장 정확한 앵커링)
  if (parsed && keywords.length > 0) {
    const kwRanges = findKeywordRanges(parsed.text, keywords);
    if (kwRanges.length > 0) {
      return buildContextWindow(parsed.text, kwRanges, contextBefore, contextAfter);
    }
  }

  // 2) fullText에서 검색어 직접 찾기 (snippet에 없지만 원본에는 있는 경우)
  if (keywords.length > 0 && fullText) {
    const kwRanges = findKeywordRanges(fullText, keywords);
    if (kwRanges.length > 0) {
      return buildContextWindow(fullText, kwRanges, contextBefore, contextAfter);
    }
  }

  // 3) snippet 하이라이트 범위 사용 (검색어가 연속 문자열이 아닌 경우 - FTS5 토큰 매칭)
  //    예: "김하늘" 검색 → "김" + "하늘" 개별 토큰 매칭 → 개별 토큰이라도 하이라이트
  if (parsed && parsed.ranges.length > 0) {
    return buildContextWindow(parsed.text, parsed.ranges, contextBefore, contextAfter);
  }

  // 4) highlight_ranges 폴백 (snippet 없는 시맨틱/하이브리드 결과용)
  if (fullText && input.highlightRanges && input.highlightRanges.length > 0) {
    return buildContextWindow(fullText, input.highlightRanges, contextBefore, contextAfter);
  }

  // 5) Final fallback - 키워드 기반 컨텍스트 추출
  const fallbackText = getPreviewWithKeyword(previewText, fullText, input.query ?? "", contextBefore, contextAfter);
  const fallbackRanges = keywords.length > 0 ? findKeywordRanges(fallbackText, keywords) : [];
  return { text: fallbackText, ranges: fallbackRanges };
}
