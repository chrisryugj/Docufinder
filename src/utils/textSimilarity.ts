/**
 * 문자 바이그램 기반 Jaccard 유사도 유틸.
 * - 한국어/영문 혼용 텍스트에서 공백/토큰화 없이 동작
 * - 스니펫 중복 청크 병합에 사용
 */

/** 문자열 → 2글자 bigram Set (공백/대소문자 무시) */
export function charBigrams(text: string): Set<string> {
  const normalized = text.replace(/\s+/g, "").toLowerCase();
  const set = new Set<string>();
  if (normalized.length < 2) {
    if (normalized.length === 1) set.add(normalized);
    return set;
  }
  for (let i = 0; i < normalized.length - 1; i++) {
    set.add(normalized.slice(i, i + 2));
  }
  return set;
}

/** 두 Set의 Jaccard 유사도 (0~1) */
export function jaccard(a: Set<string>, b: Set<string>): number {
  if (a.size === 0 && b.size === 0) return 1;
  let intersection = 0;
  const [smaller, larger] = a.size <= b.size ? [a, b] : [b, a];
  for (const x of smaller) if (larger.has(x)) intersection++;
  const union = a.size + b.size - intersection;
  return union > 0 ? intersection / union : 0;
}

/** 두 텍스트의 bigram Jaccard 유사도 */
export function textSimilarity(a: string, b: string): number {
  return jaccard(charBigrams(a), charBigrams(b));
}
