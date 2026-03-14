import { memo } from "react";

interface HighlightedFilenameProps {
  filename: string;
  query: string;
}

/**
 * 파일명에서 검색어 매칭 부분을 하이라이트
 * Everything 스타일 실시간 하이라이트
 *
 * memo() 적용: 결과당 regex 실행하므로 동일 props 시 재계산 방지
 */
export const HighlightedFilename = memo(function HighlightedFilename({ filename, query }: HighlightedFilenameProps) {
  if (!query.trim()) {
    return <>{filename}</>;
  }

  // 검색어를 공백으로 분리하여 각 토큰 처리
  const tokens = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
  if (tokens.length === 0) {
    return <>{filename}</>;
  }

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

  // 범위가 없으면 원본 반환
  if (ranges.length === 0) {
    return <>{filename}</>;
  }

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

  // 렌더링
  const parts: React.ReactNode[] = [];
  let lastIndex = 0;

  merged.forEach(([start, end], i) => {
    // 하이라이트 전 텍스트
    if (start > lastIndex) {
      parts.push(filename.slice(lastIndex, start));
    }
    // 하이라이트 텍스트
    parts.push(
      <mark key={`mark-${i}`} className="hl-filename">
        {filename.slice(start, end)}
      </mark>
    );
    lastIndex = end;
  });

  // 마지막 남은 텍스트
  if (lastIndex < filename.length) {
    parts.push(filename.slice(lastIndex));
  }

  return <>{parts}</>;
});
