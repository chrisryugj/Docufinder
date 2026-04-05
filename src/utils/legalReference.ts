/** 법령 참조 자동 감지 + law.go.kr 링크 생성 */

/** 법령 참조 정보 */
export interface LegalReference {
  /** 원본 매칭 텍스트 */
  text: string;
  /** 법령명 (감지된 경우) */
  lawName?: string;
  /** 조항 번호 */
  article?: string;
  /** 참조 유형 */
  type: "law_article" | "law_number" | "standalone_article";
  /** law.go.kr 검색 URL */
  url: string;
  /** 원본 텍스트 내 시작 위치 */
  start: number;
  /** 원본 텍스트 내 끝 위치 */
  end: number;
}

/** 법령이 아닌 일반 단어 제외 목록 */
const NON_LAW_WORDS = new Set([
  "방법", "문법", "용법", "산법", "기법", "어법", "화법", "수법",
  "서법", "작법", "주법", "타법", "투법", "필법", "합법", "불법",
]);

/** law.go.kr 검색 URL 생성 */
function buildLawUrl(query: string): string {
  return `https://law.go.kr/lsSc.do?query=${encodeURIComponent(query)}`;
}

/**
 * 법령 참조 패턴 정규식 목록
 *
 * 순서 중요: 더 구체적인 패턴이 먼저 와야 함
 */
const PATTERNS: {
  regex: RegExp;
  type: LegalReference["type"];
  extract: (match: RegExpExecArray) => { lawName?: string; article?: string; query: string };
}[] = [
  // 1) "○○법 제N조" or "○○법 제N조의N" — 법령명 + 조항
  {
    regex: /((?:[가-힣]+(?:\s+)?){1,4}법)\s*제(\d+)조(?:의(\d+))?/g,
    type: "law_article",
    extract: (m) => ({
      lawName: m[1],
      article: m[3] ? `제${m[2]}조의${m[3]}` : `제${m[2]}조`,
      query: `${m[1]} ${m[3] ? `제${m[2]}조의${m[3]}` : `제${m[2]}조`}`,
    }),
  },
  // 2) "법률 제NNNNN호" — 법률 번호
  {
    regex: /법률\s*제(\d+)호/g,
    type: "law_number",
    extract: (m) => ({
      article: `법률 제${m[1]}호`,
      query: `법률 제${m[1]}호`,
    }),
  },
  // 3) "제N조" or "제N조의N" (독립) — 문맥 없는 조항 (법령명 미감지)
  {
    regex: /(?<![가-힣])제(\d+)조(?:의(\d+))?(?:\s*제(\d+)항)?/g,
    type: "standalone_article",
    extract: (m) => {
      let article = m[2] ? `제${m[1]}조의${m[2]}` : `제${m[1]}조`;
      if (m[3]) article += ` 제${m[3]}항`;
      return { article, query: article };
    },
  },
];

/**
 * 텍스트에서 법령 참조를 추출
 *
 * @param text 분석할 텍스트
 * @returns 감지된 법령 참조 목록 (위치 순)
 */
export function extractLegalReferences(text: string): LegalReference[] {
  if (!text || text.length < 3) return [];

  const results: LegalReference[] = [];
  // 이미 매칭된 범위 추적 (중복 방지)
  const matched = new Set<string>();

  for (const pattern of PATTERNS) {
    const regex = new RegExp(pattern.regex.source, pattern.regex.flags);
    let match: RegExpExecArray | null;

    while ((match = regex.exec(text)) !== null) {
      const start = match.index;
      const end = start + match[0].length;
      const key = `${start}-${end}`;

      // 이미 더 구체적인 패턴으로 매칭된 범위와 겹치면 스킵
      let overlaps = false;
      for (const k of matched) {
        const [ms, me] = k.split("-").map(Number);
        if (start < me && end > ms) {
          overlaps = true;
          break;
        }
      }
      if (overlaps) continue;

      const extracted = pattern.extract(match);

      // 법령이 아닌 일반 단어 제외 (방법, 문법 등)
      if (extracted.lawName && NON_LAW_WORDS.has(extracted.lawName.trim())) continue;

      matched.add(key);

      results.push({
        text: match[0],
        lawName: extracted.lawName,
        article: extracted.article,
        type: pattern.type,
        url: buildLawUrl(extracted.query),
        start,
        end,
      });
    }
  }

  // 위치 순 정렬
  results.sort((a, b) => a.start - b.start);
  return results;
}
