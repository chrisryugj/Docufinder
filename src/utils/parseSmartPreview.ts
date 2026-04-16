/**
 * 스마트 검색 입력 미리보기 파서 (프론트엔드 전용)
 * 백엔드 NlQueryParser와 100% 일치할 필요 없음 — UX 힌트용
 */

export interface SmartPreview {
  keywords: string;
  dateLabel: string | null;
  fileType: string | null;
  filenameFilter: string | null;
  excludeKeywords: string[];
}

const DATE_PATTERNS: [RegExp, string][] = [
  [/오늘/, "오늘"],
  [/이번\s*주/, "이번 주"],
  [/지난\s*주/, "지난 주"],
  [/이번\s*달/, "이번 달"],
  [/지난\s*달/, "지난 달"],
  [/올해/, "올해"],
  [/작년|지난\s*해/, "작년"],
  [/최근\s*(\d+)\s*일/, "최근 $1일"],
  [/(\d{4})\s*년/, "$1년"],
  [/(\d{1,2})\s*월/, "$1월"],
];

const FILE_TYPE_PATTERNS: [RegExp, string][] = [
  [/한글\s*(문서|파일)?|hwpx?(\s*파일)?/i, "한글"],
  [/워드|word|docx?(\s*파일)?/i, "워드"],
  [/엑셀|excel|xlsx?(\s*파일)?/i, "엑셀"],
  [/pdf|피디에프(\s*파일)?/i, "PDF"],
  [/텍스트\s*파일|txt(\s*파일)?/i, "텍스트"],
  [/파워포인트|피피티|pptx?(\s*파일)?/i, "PPT"],
];

const FILENAME_PATTERNS: [RegExp, number][] = [
  [/(제목|이름|파일명|파일\s*이름)(이|에)\s+(\S+?)(인|포함된|들어간|포함|있는)/, 3],
  [/(제목|이름|파일명|파일\s*이름)(에)\s+(\S+)\s+(포함된|들어간|포함|있는)/, 3],
];

const EXCLUDE_PATTERN = /(\S+)\s*(아닌|빼고|제외|말고|없는|않은)/g;

const INTENT_SUFFIXES = /\s*(찾아줘|보여줘|검색해줘?|알려줘|있어\??|해줘)\s*$/;

export function parseSmartPreview(query: string): SmartPreview | null {
  if (!query.trim()) return null;

  let remaining = query.trim();

  // intent 제거
  remaining = remaining.replace(INTENT_SUFFIXES, "");

  // 제외 키워드 추출
  const excludeKeywords: string[] = [];
  remaining = remaining.replace(EXCLUDE_PATTERN, (_, word) => {
    excludeKeywords.push(word);
    return "";
  });

  // 파일명 필터 추출
  let filenameFilter: string | null = null;
  for (const [pattern, groupIdx] of FILENAME_PATTERNS) {
    const match = remaining.match(pattern);
    if (match && match[groupIdx]) {
      filenameFilter = match[groupIdx];
      remaining = remaining.replace(match[0], "");
      break;
    }
  }

  // 날짜 추출
  let dateLabel: string | null = null;
  for (const [pattern, label] of DATE_PATTERNS) {
    const match = remaining.match(pattern);
    if (match) {
      dateLabel = label.replace("$1", match[1] || "");
      remaining = remaining.replace(match[0], "");
      break;
    }
  }

  // 파일 타입 추출
  let fileType: string | null = null;
  for (const [pattern, label] of FILE_TYPE_PATTERNS) {
    const match = remaining.match(pattern);
    if (match) {
      fileType = label;
      remaining = remaining.replace(match[0], "");
      break;
    }
  }

  // 키워드 정리
  const keywords = remaining.replace(/\s+/g, " ").trim();

  // 아무것도 파싱되지 않았으면 null
  if (!dateLabel && !fileType && !filenameFilter && excludeKeywords.length === 0) return null;

  return { keywords, dateLabel, fileType, filenameFilter, excludeKeywords };
}
