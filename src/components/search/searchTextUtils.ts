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
  const highlightRanges = input.highlightRanges ?? [];
  const contextBefore = input.contextBefore ?? DEFAULT_CONTEXT_BEFORE;
  const contextAfter = input.contextAfter ?? DEFAULT_CONTEXT_AFTER;

  if (input.snippet && input.snippet.includes("[[HL]]")) {
    const parsed = parseSnippetHighlights(input.snippet);
    return buildContextWindow(parsed.text, parsed.ranges, contextBefore, contextAfter);
  }

  if (fullText && highlightRanges.length > 0) {
    return buildContextWindow(fullText, highlightRanges, contextBefore, contextAfter);
  }

  const fallbackText = getPreviewWithKeyword(previewText, fullText, input.query ?? "", contextBefore, contextAfter);
  const keywords = input.query ? extractSearchKeywords(input.query) : [];
  const fallbackRanges = keywords.length > 0 ? findKeywordRanges(fallbackText, keywords) : [];
  return { text: fallbackText, ranges: fallbackRanges };
}
