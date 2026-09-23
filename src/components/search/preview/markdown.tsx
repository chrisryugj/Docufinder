// 미리보기 마크다운 렌더 유틸: 검색어·법령 하이라이트, 찾기(Ctrl+F) Range 수집,
// 인용 점프 위치 탐색, HTML 살균 스키마, 마크다운 커스텀 컴포넌트.
import type { ComponentProps } from "react";
import type ReactMarkdown from "react-markdown";
import { defaultSchema } from "rehype-sanitize";
import { ImageIcon } from "lucide-react";
import { extractLegalReferences } from "../../../utils/legalReference";

// ─── 검색어 하이라이트 + 법령 참조 유틸 ────────────────

function highlightTextWithLegal(
  text: string,
  searchRegex: RegExp | null,
  onOpenUrl: (url: string) => void,
): React.ReactNode {
  const legalRefs = extractLegalReferences(text);

  if (legalRefs.length === 0 && !searchRegex) return text;

  const applySearchHighlight = (str: string, keyBase: string): React.ReactNode[] => {
    if (!searchRegex || !str) return [str];
    const parts = str.split(new RegExp(`(${searchRegex.source})`, "gi"));
    return parts.map((part, i) =>
      i % 2 === 1 ? (
        <mark key={`${keyBase}-h${i}`} className="hl-search">{part}</mark>
      ) : (
        <span key={`${keyBase}-t${i}`}>{part}</span>
      ),
    );
  };

  if (legalRefs.length === 0) {
    return <>{applySearchHighlight(text, "s")}</>;
  }

  const segments: React.ReactNode[] = [];
  let lastEnd = 0;

  for (let li = 0; li < legalRefs.length; li++) {
    const ref = legalRefs[li];
    if (ref.start > lastEnd) {
      segments.push(...applySearchHighlight(text.slice(lastEnd, ref.start), `pre-${li}`));
    }
    segments.push(
      <button
        key={`legal-${li}`}
        onClick={() => onOpenUrl(ref.url)}
        className="preview-inline-link inline underline decoration-dotted underline-offset-2 cursor-pointer"
        style={{ color: "var(--color-accent)" }}
        title={`${ref.lawName ? ref.lawName + " " : ""}${ref.article || ref.text} · 법제처에서 열기`}
      >
        {ref.text}
      </button>,
    );
    lastEnd = ref.end;
  }

  if (lastEnd < text.length) {
    segments.push(...applySearchHighlight(text.slice(lastEnd), "post"));
  }

  return <>{segments}</>;
}

// ─── 찾기 바(Ctrl+F) 하이라이트 — CSS Custom Highlight API (T3-6) ──
// 이전에는 찾기 정규식이 markdown components 를 갈아끼워 확정 검색어마다
// remark/katex 전체 재파싱을 유발했다. 이제 커밋된 DOM 텍스트 노드를
// TreeWalker 로 순회해 Range 를 만들고 CSS.highlights 에 등록만 한다 —
// DOM 을 바꾸지 않으므로 React 재조정과 충돌하지 않고, 재파싱도 없다.

export const FIND_HIGHLIGHT = "docufinder-find";
export const FIND_ACTIVE_HIGHLIGHT = "docufinder-find-active";

/** CSS Custom Highlight API 지원 여부 (WebView2/Safari 17.2+ — 미지원 시 카운트/이동만 동작) */
export const cssHighlightsSupported = (): boolean =>
  typeof CSS !== "undefined" && "highlights" in CSS;

/** 본문 텍스트 노드를 순회하며 찾기 매치 Range 수집 (문서 순서 보장) */
export function collectFindRanges(root: HTMLElement, regex: RegExp): Range[] {
  const ranges: Range[] = [];
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
    // KaTeX 는 시각용 HTML 과 보조기기용 트리에 같은 텍스트가 중복되어
    // 이중 카운트를 유발하므로 제외
    acceptNode: (node) =>
      node.parentElement?.closest(".katex")
        ? NodeFilter.FILTER_REJECT
        : NodeFilter.FILTER_ACCEPT,
  });
  for (let node = walker.nextNode(); node; node = walker.nextNode()) {
    const text = node.nodeValue;
    if (!text) continue;
    regex.lastIndex = 0;
    let m: RegExpExecArray | null;
    while ((m = regex.exec(text)) !== null) {
      if (m[0].length === 0) break; // 빈 매치 무한루프 방어
      const r = document.createRange();
      r.setStart(node, m.index);
      r.setEnd(node, m.index + m[0].length);
      ranges.push(r);
    }
  }
  return ranges;
}

// ─── 인용 점프: 앵커 텍스트 → 미리보기 DOM 위치 탐색 ──

const JUMP_BLOCK_SELECTOR = "p, li, td, th, h1, h2, h3, h4, h5, h6, blockquote";

/** NFC 정규화 + 공백/비가시 문자 + 마크다운 마커 제거 — 앵커(DB 청크, 백엔드에서
 *  normalize_text 적용됨)와 미리보기(kordoc 재파싱, 비정규화)의 차이를 흡수한다.
 *  `\s` 가 NBSP·ideographic space·BOM 을 이미 포함하므로, 그 밖의 zero-width
 *  (U+200B/C/D)·soft-hyphen(U+00AD) 만 추가 제거하면 백엔드 normalize_text 와 정합.
 *  앵커와 DOM textContent 양쪽 모두 이 함수를 거치므로 어느 쪽에 차이가 있든 정렬된다. */
const normForMatch = (s: string): string =>
  s.normalize("NFC").replace(/[\s\u200B\u200C\u200D\u00AD*#`|_~[\]]/g, "");

/**
 * 백엔드 앵커 텍스트(청크 앞부분)를 렌더된 미리보기에서 찾아 해당 블록 요소를 반환.
 * offset 좌표가 아닌 텍스트 검색인 이유: kordoc 경로에서 page/offset이 비거나
 * 좌표 단위가 혼재해 신뢰 불가하기 때문. 못 찾으면 페이지 헤딩으로 폴백.
 */
export function findJumpTarget(
  root: HTMLElement,
  anchors: string[],
  page: number | null,
): HTMLElement | null {
  const blocks = Array.from(root.querySelectorAll<HTMLElement>(JUMP_BLOCK_SELECTOR));

  // 1) 앵커 텍스트 매칭 (검색 score 순 → 가장 관련 높은 청크 우선)
  for (const anchor of anchors) {
    const needle = normForMatch(anchor).slice(0, 20);
    if (needle.length < 6) continue;
    for (const el of blocks) {
      if (normForMatch(el.textContent || "").includes(needle)) return el;
    }
  }

  // 2) 페이지 헤딩 폴백 — DB 폴백 경로가 주입하는 "## N페이지" 헤딩
  if (page != null) {
    const tag = normForMatch(`${page}페이지`);
    for (const el of blocks) {
      if (/^h[1-6]$/i.test(el.tagName) && normForMatch(el.textContent || "").includes(tag)) {
        return el;
      }
    }
  }

  return null;
}

// ─── 미리보기 HTML 살균 스키마 (kordoc HTML 표 네이티브 렌더용) ──
//
// kordoc은 병합·중첩 표를 <table><td colspan rowspan> HTML로 방출한다(단순 표는 GFM).
// 과거엔 이 HTML을 정규식으로 GFM 파이프표에 눌러 담았으나, GFM은 colspan/rowspan·중첩표를
// 표현하지 못하고 non-greedy 정규식이 중첩표에서 바깥 표 닫힘을 놓쳐 결재문서(실측 194건 중
// 43%가 중첩표)의 도장란·문서번호표가 뭉개졌다. 이제 rehype-raw로 HTML 표를 그대로 렌더해
// 브라우저가 병합·중첩을 네이티브 처리하게 하고(rhwp HTML 백엔드·kordoc renderHtml과 동일 접근),
// rehype-sanitize로 XSS를 차단한다. 표 구조 태그 + 병합 속성 + KaTeX placeholder 통과용
// className만 기본 스키마에 추가 허용한다. (sanitize는 katex 앞에 두어 수식 출력은 통과.)
//
// className은 code 요소의 화이트리스트 값(language-*·math-inline·math-display, remark-math
// 공식 sanitize 레시피)으로만 허용한다. 전역("*") className 허용은 신뢰 불가 문서가 번들
// Tailwind 클래스(.fixed .inset-0 .z-50 .bg-white …)로 전창 오버레이 디페이스/피싱 UI를
// 구성할 수 있어 금지 — KaTeX 는 code.math-inline/math-display 만 보므로 수식 렌더는 유지.
export const PREVIEW_SANITIZE_SCHEMA = {
  ...defaultSchema,
  tagNames: [
    // "u" — kordoc v4.7.0+ 가 밑줄을 <u>…</u> 로 방출한다 (기본 스키마에 없어 그냥 두면
    // 태그가 통째로 사라져 원문의 밑줄 강조가 미리보기에서 소실된다). 속성 없는 표시 태그.
    ...(defaultSchema.tagNames ?? []),
    "table", "thead", "tbody", "tfoot", "tr", "td", "th", "col", "colgroup", "br", "u",
  ],
  attributes: {
    ...defaultSchema.attributes,
    td: [...(defaultSchema.attributes?.td ?? []), "colSpan", "rowSpan", "colspan", "rowspan", "align"],
    th: [...(defaultSchema.attributes?.th ?? []), "colSpan", "rowSpan", "colspan", "rowspan", "align", "scope"],
    col: ["span", "width"],
    code: [["className", /^language-./, "math-inline", "math-display"]],
  },
};

// ─── 마크다운 커스텀 컴포넌트 ──────────────────────────

/**
 * 문단 선두 불릿 문자로 위계 판정
 * - Level 1 (상위): ■ □ ▣ ▢ ◆ ◇ — 두껍고 크게
 * - Level 2 (중간): ● ○ ◉ ◎ ▸ ▹ — 보통
 * - Level 3 (하위): - * · • ◦ — 작고 흐리게
 */
function detectBulletLevel(text: string): 1 | 2 | 3 | null {
  const trimmed = text.trimStart();
  const first = trimmed.charAt(0);
  if (!first) return null;
  if (/[■□▣▢◆◇]/.test(first)) return 1;
  if (/[●○◉◎▸▹]/.test(first)) return 2;
  if (/[\-*·•◦]/.test(first)) return 3;
  return null;
}

/** React children에서 첫 문자열 추출 (불릿 감지용, React 엘리먼트 재귀 파고듦) */
function firstTextOf(children: React.ReactNode): string {
  if (children == null || typeof children === "boolean") return "";
  if (typeof children === "string") return children;
  if (typeof children === "number") return String(children);
  if (Array.isArray(children)) {
    for (const c of children) {
      const t = firstTextOf(c);
      if (t) return t;
    }
    return "";
  }
  // React 엘리먼트 — props.children 재귀 (kordoc가 전체 문단을 **bold**로 감싼 경우 대응)
  if (typeof children === "object" && "props" in (children as object)) {
    const el = children as { props?: { children?: React.ReactNode } };
    if (el.props?.children !== undefined) {
      return firstTextOf(el.props.children);
    }
  }
  return "";
}

export function createMarkdownComponents(
  searchRegex: RegExp | null,
  onOpenUrl: (url: string) => void,
): ComponentProps<typeof ReactMarkdown>["components"] {
  // 텍스트 노드에 하이라이트 적용하는 래퍼 (찾기 하이라이트는 렌더 후
  // CSS Custom Highlight 로 별도 적용 — 여기서는 검색어/법령만)
  const TextWrapper = ({ children }: { children: React.ReactNode }) => {
    if (typeof children === "string") {
      return <>{highlightTextWithLegal(children, searchRegex, onOpenUrl)}</>;
    }
    return <>{children}</>;
  };

  return {
    // 텍스트가 포함된 블록 요소에 하이라이트 적용
    p: ({ children }) => {
      const level = detectBulletLevel(firstTextOf(children));
      const bulletClass = level ? ` doc-bullet-${level}` : "";
      return (
        <p className={`doc-paragraph${bulletClass}`}>
          {Array.isArray(children)
            ? children.map((child, i) => <TextWrapper key={i}>{child}</TextWrapper>)
            : <TextWrapper>{children}</TextWrapper>}
        </p>
      );
    },
    // 헤딩
    h1: ({ children }) => <h1 className="doc-h1"><TextWrapper>{children}</TextWrapper></h1>,
    h2: ({ children }) => <h2 className="doc-h2"><TextWrapper>{children}</TextWrapper></h2>,
    h3: ({ children }) => <h3 className="doc-h3"><TextWrapper>{children}</TextWrapper></h3>,
    h4: ({ children }) => <h4 className="doc-h4"><TextWrapper>{children}</TextWrapper></h4>,
    h5: ({ children }) => <h5 className="doc-h5">{children}</h5>,
    h6: ({ children }) => <h6 className="doc-h6">{children}</h6>,
    // 테이블
    table: ({ children }) => (
      <div className="doc-table-wrapper">
        <table className="doc-table">{children}</table>
      </div>
    ),
    thead: ({ children }) => <thead className="doc-thead">{children}</thead>,
    // node 제외 후 나머지(colSpan/rowSpan/align 등 병합 속성) 전달 — HTML 표 병합셀 보존
    th: ({ children, node: _node, ...rest }) => <th className="doc-th" {...rest}><TextWrapper>{children}</TextWrapper></th>,
    td: ({ children, node: _node, ...rest }) => <td className="doc-td" {...rest}><TextWrapper>{children}</TextWrapper></td>,
    // 링크: 외부 브라우저로 열기
    a: ({ href, children }) => (
      <button
        onClick={() => href && onOpenUrl(href)}
        className="preview-inline-link inline underline decoration-dotted underline-offset-2 cursor-pointer"
        style={{ color: "var(--color-accent)" }}
        title={href}
      >
        {children}
      </button>
    ),
    // 리스트
    ul: ({ children }) => <ul className="doc-ul">{children}</ul>,
    ol: ({ children }) => <ol className="doc-ol">{children}</ol>,
    li: ({ children }) => <li className="doc-li"><TextWrapper>{children}</TextWrapper></li>,
    // 구분선
    hr: () => <hr className="doc-hr" />,
    // 인용문
    blockquote: ({ children }) => <blockquote className="doc-blockquote">{children}</blockquote>,
    // 강조
    strong: ({ children }) => <strong className="doc-strong">{children}</strong>,
    em: ({ children }) => <em className="doc-em">{children}</em>,
    del: ({ children }) => <del className="doc-del">{children}</del>,
    // 그림 자리: kordoc 은 그림 위치에 상대경로 참조(image_001.png)만 남기고 앱은 이미지
    // 파일을 받지 않는다. 그대로 두면 깨진 그림 아이콘이 뜨므로 자리만 표시한다.
    img: ({ alt }) => (
      <span className="doc-image-placeholder" role="img" aria-label={alt && alt !== "image" ? `그림: ${alt}` : "그림"}>
        <ImageIcon size={12} aria-hidden="true" />
        그림
      </span>
    ),
  };
}
