import { memo } from "react";

interface HighlightedFilenameProps {
  filename: string;
  query: string;
  /** 넘치면 가운데를 줄인다 — 끝(확장자와 그 앞 몇 글자)은 늘 보인다.
   *  부모가 폭을 제한해야 한다(min-w-0·overflow hidden). */
  middle?: boolean;
}

interface Segment {
  text: string;
  hl: boolean;
}

/** 검색어 토큰이 걸린 구간을 병합해 [일반/강조] 조각으로 나눈다 */
function toSegments(filename: string, query: string): Segment[] {
  // 검색어를 공백으로 분리하여 각 토큰 처리
  const tokens = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
  if (tokens.length === 0) return [{ text: filename, hl: false }];

  // 모든 매칭 범위 찾기
  const ranges: [number, number][] = [];
  const lowerFilename = filename.toLowerCase();
  for (const token of tokens) {
    let index = 0;
    while ((index = lowerFilename.indexOf(token, index)) !== -1) {
      ranges.push([index, index + token.length]);
      index += token.length;
    }
  }
  if (ranges.length === 0) return [{ text: filename, hl: false }];

  // 범위 정렬 및 병합
  ranges.sort((a, b) => a[0] - b[0]);
  const merged: [number, number][] = [];
  for (const [start, end] of ranges) {
    if (merged.length === 0 || start > merged[merged.length - 1][1]) {
      merged.push([start, end]);
    } else {
      merged[merged.length - 1][1] = Math.max(merged[merged.length - 1][1], end);
    }
  }

  const segments: Segment[] = [];
  let lastIndex = 0;
  for (const [start, end] of merged) {
    if (start > lastIndex) segments.push({ text: filename.slice(lastIndex, start), hl: false });
    segments.push({ text: filename.slice(start, end), hl: true });
    lastIndex = end;
  }
  if (lastIndex < filename.length) segments.push({ text: filename.slice(lastIndex), hl: false });
  return segments;
}

/** 가운데 줄임의 자르는 위치. 끝에 남길 꼬리는 확장자 + 4~14글자이고, 그 안의 구분 문자
 *  ( _-.([ 공백) 중 꼬리가 가장 짧아지는 것 앞에서 자른다 — 좁은 칸에도 꼬리가 들어가야 하고,
 *  구분 문자는 꼬리 쪽에 두어야 공백이 줄 끝에서 사라지지 않는다. 짧은 이름은 자르지 않는다(-1). */
function middleCutIndex(filename: string): number {
  const dot = filename.lastIndexOf(".");
  const extLen = dot > 0 && filename.length - dot <= 6 ? filename.length - dot : 0;
  const len = filename.length;
  if (len <= extLen + 18) return -1;
  const from = len - extLen - 14;
  const to = len - extLen - 4;
  for (let i = to; i >= from; i--) {
    if (" _-.([".includes(filename[i])) return i;
  }
  return len - extLen - 8;
}

function splitSegments(segments: Segment[], cut: number): [Segment[], Segment[]] {
  const head: Segment[] = [];
  const tail: Segment[] = [];
  let pos = 0;
  for (const seg of segments) {
    const end = pos + seg.text.length;
    if (end <= cut) head.push(seg);
    else if (pos >= cut) tail.push(seg);
    else {
      head.push({ text: seg.text.slice(0, cut - pos), hl: seg.hl });
      tail.push({ text: seg.text.slice(cut - pos), hl: seg.hl });
    }
    pos = end;
  }
  return [head, tail];
}

function renderSegments(segments: Segment[], keyPrefix: string): React.ReactNode[] {
  return segments.map((seg, i) =>
    seg.hl ? (
      <mark key={`${keyPrefix}${i}`} className="hl-filename">
        {seg.text}
      </mark>
    ) : (
      seg.text
    )
  );
}

/**
 * 파일명에서 검색어 매칭 부분을 하이라이트
 * Everything 스타일 실시간 하이라이트
 *
 * memo() 적용: 결과당 regex 실행하므로 동일 props 시 재계산 방지
 */
export const HighlightedFilename = memo(function HighlightedFilename({ filename, query, middle = false }: HighlightedFilenameProps) {
  const segments = toSegments(filename, query);
  if (!middle) return <>{renderSegments(segments, "m")}</>;
  const cut = middleCutIndex(filename);
  // 짧은 이름은 가운데 줄임 없이 끝에서만 말줄임 (부모가 truncate 를 맡기지 않으므로 여기서)
  if (cut < 0) return <span className="block truncate">{renderSegments(segments, "m")}</span>;

  // "…_최종_수정본.hwpx" 처럼 끝이 구분점인 이름이 많아, 넘칠 때 앞을 줄이고 끝을 남긴다
  const [head, tail] = splitSegments(segments, cut);
  return (
    <span className="flex min-w-0">
      {/* 앞(shrink 1000)이 먼저 줄고, 다 줄어든 뒤에야 꼬리(shrink 1)가 줄어든다.
          꼬리 비율을 1 미만으로 두면 flexbox 규칙상 남은 공간을 그 비율만큼만 흡수해 칸을 넘친다 */}
      <span className="truncate" style={{ flexShrink: 1000 }}>{renderSegments(head, "h")}</span>
      <span className="min-w-0 overflow-hidden text-ellipsis whitespace-pre">
        {renderSegments(tail, "t")}
      </span>
    </span>
  );
});
