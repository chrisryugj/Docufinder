/**
 * SearchResultItem에서 분리된 순수 텍스트 유틸 함수들
 */

const EXPANDED_CONTEXT_BEFORE_CHARS = 300;
const EXPANDED_CONTEXT_AFTER_CHARS = 300;

export function formatPathSegments(path: string): { label: string; fullPath: string }[] {
  const cleanPath = path.replace(/^\\\\\?\\/, "").replace(/^\/\/\?\//, "");
  const parts = cleanPath.split(/[/\\]/).filter(Boolean);

  const segments = parts.map((part, i) => ({
    label: part,
    fullPath: parts.slice(0, i + 1).join("\\"),
  }));

  if (segments.length > 10) {
    return [
      ...segments.slice(0, 3),
      { label: "\u2026", fullPath: "" },
      ...segments.slice(-4),
    ];
  }
  return segments;
}

export function buildExpandedContext(
  fullText: string,
  ranges: [number, number][],
  snippet?: string
): { text: string; ranges: [number, number][] } {
  const anchor = snippet
    ? findSnippetAnchor(fullText, snippet, ranges)
    : findFirstRangeAnchor(ranges);
  const effectiveAnchor = anchor ?? findFirstRangeAnchor(ranges);
  if (!effectiveAnchor) {
    const limitedText = fullText.slice(0, 600);
    return {
      text: limitedText + (fullText.length > 600 ? "..." : ""),
      ranges,
    };
  }

  const startOffset = Math.max(0, effectiveAnchor.start - EXPANDED_CONTEXT_BEFORE_CHARS);
  const endOffset = Math.min(fullText.length, effectiveAnchor.end + EXPANDED_CONTEXT_AFTER_CHARS);

  const trimmedText = fullText.slice(startOffset, endOffset);
  const trimmedRanges = ranges
    .filter(([start, end]) => end > startOffset && start < endOffset)
    .map(([start, end]) => {
      const clippedStart = Math.max(0, start - startOffset);
      const clippedEnd = Math.min(trimmedText.length, end - startOffset);
      return [clippedStart, clippedEnd] as [number, number];
    });

  const prefix = startOffset > 0 ? "..." : "";
  const suffix = endOffset < fullText.length ? "..." : "";
  const finalText = prefix + trimmedText + suffix;

  const offsetAdjust = prefix.length;
  const adjustedRanges = trimmedRanges.map(
    ([start, end]) => [start + offsetAdjust, end + offsetAdjust] as [number, number]
  );

  return { text: finalText, ranges: adjustedRanges };
}

export function findSnippetAnchor(
  fullText: string,
  snippet: string,
  ranges: [number, number][]
): { start: number; end: number } | null {
  const segments = snippet.split("...");
  let fallback: { start: number; end: number } | null = null;

  for (const segment of segments) {
    if (!segment.includes("[[HL]]")) continue;
    const parsed = parseSnippetSegment(segment);
    if (!parsed.text.trim()) continue;

    let searchStart = 0;
    while (true) {
      const index = fullText.indexOf(parsed.text, searchStart);
      if (index === -1) break;

      const candidate = parsed.ranges.length
        ? { start: index + parsed.ranges[0][0], end: index + parsed.ranges[0][1] }
        : { start: index, end: index + parsed.text.length };

      if (!fallback) fallback = candidate;

      if (ranges.some(([rangeStart, rangeEnd]) => candidate.start >= rangeStart && candidate.end <= rangeEnd)) {
        return candidate;
      }
      searchStart = index + parsed.text.length;
    }
  }
  return fallback;
}

export function parseSnippetSegment(segment: string): { text: string; ranges: [number, number][] } {
  const ranges: [number, number][] = [];
  let text = "";
  let i = 0;

  while (i < segment.length) {
    if (segment.slice(i, i + 6) === "[[HL]]") {
      const start = text.length;
      i += 6;
      const endMarker = segment.indexOf("[[/HL]]", i);
      if (endMarker !== -1) {
        text += segment.slice(i, endMarker);
        ranges.push([start, text.length]);
        i = endMarker + 7;
      } else {
        text += segment.slice(i);
        ranges.push([start, text.length]);
        break;
      }
    } else {
      text += segment[i];
      i += 1;
    }
  }
  return { text, ranges };
}

function findFirstRangeAnchor(ranges: [number, number][]): { start: number; end: number } | null {
  if (ranges.length === 0) return null;
  let [start, end] = ranges[0];
  for (const [rangeStart, rangeEnd] of ranges) {
    if (rangeStart < start) { start = rangeStart; end = rangeEnd; }
  }
  return { start, end };
}
