/**
 * 검색창 인라인 연산자 미리보기 파서 (프론트엔드 표시 전용)
 *
 * 백엔드 src-tauri/src/search/query_syntax.rs::parse_operators 와 의미론 일치:
 * - `"정확 구문"`            : 구문 검색 (닫는 따옴표 없으면 나머지 전체가 구문)
 * - `-제외어` / `-"제외 구문"` : 공백 뒤 `-` 만 제외 연산자 (단어 중간 하이픈은 일반 문자)
 * - `ext:hwp` / `ext:hwp,pdf` : 확장자 필터 (점/대문자 허용, 레거시 그룹 자동 확장)
 * - `path:단어`               : 파일 경로 부분 일치
 * - `after:` / `before:`      : 수정일 범위 YYYY[-MM[-DD]] (`.`/`/` 구분자 허용, 파싱 실패 시 일반 단어)
 * - `~N`                      : 근접 검색 (N 생략 시 10, 1~100 클램프, 숫자 아니면 일반 단어)
 *
 * 표시 전용이므로 날짜는 타임스탬프 변환 없이 원문 라벨로 유지한다.
 */

/** 근접 검색(`~`)에 숫자가 없을 때 기본 토큰 거리 (백엔드 DEFAULT_NEAR_DISTANCE) */
export const DEFAULT_NEAR_DISTANCE = 10;

export interface OperatorPreview {
  /** 연산자를 제거하고 남은 일반 검색어 (공백 구분) */
  terms: string;
  /** `"..."` 정확 구문 목록 */
  phrases: string[];
  /** `-단어` 제외어 목록 */
  excludes: string[];
  /** `ext:` 확장자 목록 (소문자, 점 없음, 그룹 확장 완료) */
  extFilters: string[];
  /** `path:` 경로 부분 일치 토큰 (소문자) */
  pathFilters: string[];
  /** `after:` 원문 날짜 라벨 (유효한 날짜일 때만) */
  afterLabel: string | null;
  /** `before:` 원문 날짜 라벨 (유효한 날짜일 때만) */
  beforeLabel: string | null;
  /** `~N` 근접 검색 토큰 거리 */
  near: number | null;
}

type QueryToken =
  | { kind: "word"; value: string }
  | { kind: "phrase"; value: string }
  | { kind: "exclude"; value: string };

/** 레거시 확장자 그룹 확장 — 백엔드 expand_ext_group 미러 */
const EXT_GROUPS: Record<string, string[]> = {
  hwp: ["hwp", "hwpx"],
  hwpx: ["hwp", "hwpx"],
  doc: ["doc", "docx"],
  docx: ["doc", "docx"],
  xls: ["xls", "xlsx"],
  xlsx: ["xls", "xlsx"],
  ppt: ["ppt", "pptx"],
  pptx: ["ppt", "pptx"],
};

const U32_MAX = 4294967295;

function isWhitespace(ch: string): boolean {
  return /\s/.test(ch);
}

/** 인용부호를 존중하며 토큰화 — 백엔드 tokenize_respecting_quotes 미러 */
function tokenizeRespectingQuotes(raw: string): QueryToken[] {
  const tokens: QueryToken[] = [];
  const chars = Array.from(raw);
  let i = 0;

  while (i < chars.length) {
    while (i < chars.length && isWhitespace(chars[i])) i++;
    if (i >= chars.length) break;

    const negated = chars[i] === "-" && i + 1 < chars.length && !isWhitespace(chars[i + 1]);
    if (negated) i++;

    if (chars[i] === '"') {
      // 인용 구문: 닫는 따옴표까지 (미폐쇄 시 끝까지)
      i++;
      const start = i;
      while (i < chars.length && chars[i] !== '"') i++;
      const phrase = chars.slice(start, i).join("");
      if (i < chars.length) i++; // 닫는 따옴표 소비
      tokens.push({ kind: negated ? "exclude" : "phrase", value: phrase });
    } else {
      const start = i;
      while (i < chars.length && !isWhitespace(chars[i])) i++;
      const word = chars.slice(start, i).join("");
      tokens.push({ kind: negated ? "exclude" : "word", value: word });
    }
  }

  return tokens;
}

/** 대소문자 무시 prefix 제거 — 백엔드 strip_prefix_ci 미러 */
function stripPrefixCI(s: string, prefix: string): string | null {
  if (s.length < prefix.length) return null;
  return s.slice(0, prefix.length).toLowerCase() === prefix ? s.slice(prefix.length) : null;
}

/** `YYYY[-MM[-DD]]` (구분자 `-`/`.`/`/`) 유효성 — 백엔드 parse_date_token 미러 (표시 전용) */
function isValidDateToken(s: string): boolean {
  const normalized = s.trim().replace(/[./]/g, "-");
  const parts = normalized.split("-").filter((p) => p.length > 0);
  if (parts.length < 1 || parts.length > 3) return false;
  if (!parts.every((p) => /^\d+$/.test(p))) return false;

  const year = parseInt(parts[0], 10);
  if (year < 1970 || year > 2100) return false;
  if (parts.length >= 2) {
    const month = parseInt(parts[1], 10);
    if (month < 1 || month > 12) return false;
    if (parts.length === 3) {
      const day = parseInt(parts[2], 10);
      const daysInMonth = new Date(year, month, 0).getDate();
      if (day < 1 || day > daysInMonth) return false;
    }
  }
  return true;
}

/**
 * 검색어에서 인라인 연산자를 분리한다 (표시용).
 * 연산자가 하나도 없으면 null — 미리보기 비표시 (백엔드 has_operators와 동일 기준).
 */
export function parseOperatorPreview(raw: string): OperatorPreview | null {
  const result: OperatorPreview = {
    terms: "",
    phrases: [],
    excludes: [],
    extFilters: [],
    pathFilters: [],
    afterLabel: null,
    beforeLabel: null,
    near: null,
  };
  const terms: string[] = [];

  for (const token of tokenizeRespectingQuotes(raw)) {
    if (token.kind === "phrase") {
      const p = token.value.trim();
      if (p) result.phrases.push(p);
      continue;
    }
    if (token.kind === "exclude") {
      const e = token.value.trim();
      if (e) result.excludes.push(e);
      continue;
    }

    const w = token.value;
    const extRest = stripPrefixCI(w, "ext:") ?? stripPrefixCI(w, "type:");
    const pathRest = extRest === null ? stripPrefixCI(w, "path:") : null;
    const afterRest = extRest === null && pathRest === null ? stripPrefixCI(w, "after:") : null;
    const beforeRest =
      extRest === null && pathRest === null && afterRest === null
        ? stripPrefixCI(w, "before:")
        : null;

    if (extRest !== null) {
      for (const rawExt of extRest.split(",")) {
        const ext = rawExt.trim().replace(/^\.+/, "").toLowerCase();
        if (!ext) continue;
        for (const expanded of EXT_GROUPS[ext] ?? [ext]) {
          if (!result.extFilters.includes(expanded)) result.extFilters.push(expanded);
        }
      }
    } else if (pathRest !== null) {
      const p = pathRest.trim().toLowerCase();
      if (p) result.pathFilters.push(p);
    } else if (afterRest !== null) {
      if (isValidDateToken(afterRest)) result.afterLabel = afterRest.trim();
      else terms.push(w);
    } else if (beforeRest !== null) {
      if (isValidDateToken(beforeRest)) result.beforeLabel = beforeRest.trim();
      else terms.push(w);
    } else if (w.startsWith("~")) {
      const rest = w.slice(1);
      if (rest === "") {
        result.near = DEFAULT_NEAR_DISTANCE;
      } else if (/^\d+$/.test(rest) && Number(rest) <= U32_MAX) {
        result.near = Math.min(Math.max(Number(rest), 1), 100);
      } else {
        terms.push(w);
      }
    } else {
      terms.push(w);
    }
  }

  result.terms = terms.join(" ");

  const hasOperators =
    result.phrases.length > 0 ||
    result.excludes.length > 0 ||
    result.extFilters.length > 0 ||
    result.pathFilters.length > 0 ||
    result.afterLabel !== null ||
    result.beforeLabel !== null ||
    result.near !== null;

  return hasOperators ? result : null;
}
