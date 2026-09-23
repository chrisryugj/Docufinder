import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode, type RefObject } from "react";
import {
  FIND_ACTIVE_HIGHLIGHT,
  FIND_HIGHLIGHT,
  collectFindRanges,
  cssHighlightsSupported,
} from "./markdown";

interface UsePreviewFindArgs {
  filePath: string | null;
  markdown: string | null;
  loading: boolean;
  /** 찾기 대상 본문 루트 (AI 요약·질문답변 영역 제외) */
  bodyRef: RefObject<HTMLDivElement | null>;
  /** 본문 렌더 노드. 재커밋되면 이전 Range 가 분리된 노드를 가리키므로 재수집 트리거로 쓴다. */
  bodyNode: ReactNode;
}

/** 미리보기 찾기 바(Ctrl+F): 입력·확정 검색어·매치 수집·활성 매치 이동. */
export function usePreviewFind({ filePath, markdown, loading, bodyRef, bodyNode }: UsePreviewFindArgs) {
  const findInputRef = useRef<HTMLInputElement>(null);
  const findComposingRef = useRef(false); // IME 조합 중 여부 (compositionend 후 검색)
  const findRangesRef = useRef<Range[]>([]);
  const [findOpen, setFindOpen] = useState(false);
  const [findInput, setFindInput] = useState("");
  const [findTerm, setFindTerm] = useState(""); // 디바운스 + 조합 완료 후 확정값
  const [findCount, setFindCount] = useState(0);
  const [findActiveIdx, setFindActiveIdx] = useState(0);

  // 파일이 바뀌면 찾기 상태 초기화
  useEffect(() => {
    setFindOpen(false);
    setFindInput("");
    setFindTerm("");
    setFindActiveIdx(0);
  }, [filePath]);

  // 찾기 바 정규식 (확정 검색어 기준, 바 닫힘 시 비활성)
  const findRegex = useMemo(() => {
    const term = findTerm.trim();
    if (!term) return null;
    return new RegExp(term.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), "gi");
  }, [findTerm]);
  const activeFindRegex = findOpen ? findRegex : null;

  const closeFind = useCallback(() => {
    setFindOpen(false);
    setFindActiveIdx(0);
  }, []);

  // 찾기 토글 — 레이아웃 뷰에서도 인라인 SVG <text> 매치를 LayoutView 가 처리하므로
  // 뷰 전환 없이 찾기 바만 토글한다 (버튼·Ctrl+F 공용).
  const handleFindToggle = useCallback(() => {
    setFindOpen((v) => !v);
  }, []);

  const handleFindNav = useCallback((dir: 1 | -1) => {
    setFindActiveIdx((prev) => (findCount > 0 ? (prev + dir + findCount) % findCount : 0));
  }, [findCount]);

  const handleFindInputKeyDown = useCallback((e: React.KeyboardEvent<HTMLInputElement>) => {
    // 전역 단축키와 격리 — Escape(선택 해제→프리뷰 닫힘), ↑/↓(결과 이동) 누출 방지
    e.stopPropagation();
    if (e.key === "Escape") {
      e.preventDefault();
      closeFind();
    } else if (e.key === "Enter") {
      if (e.nativeEvent.isComposing) return;
      e.preventDefault();
      handleFindNav(e.shiftKey ? -1 : 1);
    } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "f") {
      e.preventDefault();
      e.currentTarget.select();
    }
  }, [closeFind, handleFindNav]);

  // 찾기 바 열린 상태에서 패널 내 어디서든 Esc → 찾기 바만 닫기 (프리뷰 닫힘 차단)
  const handlePanelKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === "Escape" && findOpen) {
      e.preventDefault();
      e.stopPropagation();
      closeFind();
    }
  }, [findOpen, closeFind]);

  // 입력 디바운스 → 확정 검색어 (IME 조합 중에는 compositionend 핸들러가 확정)
  useEffect(() => {
    if (findComposingRef.current) return;
    const timer = setTimeout(() => setFindTerm(findInput), 150);
    return () => clearTimeout(timer);
  }, [findInput]);

  // Ctrl/Cmd+F 토글 — 미리보기가 열려 있으면 포커스 위치와 무관하게 동작.
  // (이전엔 패널 포커스/호버를 요구해 결과 리스트에 마우스를 둔 채 누르면
  //  무반응이라 "안 먹는" 것으로 보였다. 앱 내 다른 Ctrl+F 소비자가 없고
  //  Tauri WebView 에는 브라우저 기본 찾기도 없어 충돌 대상이 없다.)
  useEffect(() => {
    if (!filePath) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey) || e.shiftKey || e.altKey) return;
      if (e.key.toLowerCase() !== "f" && e.code !== "KeyF") return;
      // 다이얼로그(크게 보기 팝업 등)가 열려 있으면 무시 — 오버레이 뒤 안 보이는 찾기 바를
      // 열어 포커스를 모달 밖으로 빼가는 것을 막는다(bare-key 뷰 전환 가드와 동일 관례).
      if (document.querySelector("[role='dialog']")) return;
      e.preventDefault();
      handleFindToggle();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [filePath, handleFindToggle]);

  // 열릴 때 입력 포커스 (기존 검색어 유지 시 전체 선택)
  useEffect(() => {
    if (findOpen) {
      findInputRef.current?.focus();
      findInputRef.current?.select();
    }
  }, [findOpen]);

  // 매치 수집 + 하이라이트 등록 — 커밋된 본문 DOM 에 Range 만 등록, 재파싱 없음 (T3-6).
  // bodyNode 의존: 본문 서브트리가 재커밋되면(내용·검색어 변경) 이전
  // Range 가 분리된 노드를 가리키므로 새 DOM 에서 재수집해야 한다.
  useEffect(() => {
    const body = bodyRef.current;
    findRangesRef.current = [];
    if (cssHighlightsSupported()) {
      CSS.highlights.delete(FIND_HIGHLIGHT);
      CSS.highlights.delete(FIND_ACTIVE_HIGHLIGHT);
    }
    if (!activeFindRegex || loading || !markdown || !body) {
      setFindCount(0);
      setFindActiveIdx(0);
      return;
    }
    const ranges = collectFindRanges(body, activeFindRegex);
    findRangesRef.current = ranges;
    if (cssHighlightsSupported() && ranges.length > 0) {
      CSS.highlights.set(FIND_HIGHLIGHT, new Highlight(...ranges));
    }
    setFindCount(ranges.length);
    setFindActiveIdx(0);
    return () => {
      // 패널 언마운트 시 전역 레지스트리 잔류 방지
      if (cssHighlightsSupported()) {
        CSS.highlights.delete(FIND_HIGHLIGHT);
        CSS.highlights.delete(FIND_ACTIVE_HIGHLIGHT);
      }
    };
  }, [activeFindRegex, markdown, loading, bodyNode, bodyRef]);

  // 활성 매치 강조 + 스크롤 — Highlight 레지스트리 엔트리만 교체 (재렌더 없음)
  useEffect(() => {
    const active = findRangesRef.current[findActiveIdx];
    if (!active) {
      if (cssHighlightsSupported()) CSS.highlights.delete(FIND_ACTIVE_HIGHLIGHT);
      return;
    }
    if (cssHighlightsSupported()) {
      CSS.highlights.set(FIND_ACTIVE_HIGHLIGHT, new Highlight(active));
    }
    // Range 자체는 scrollIntoView 가 없어 매치가 속한 요소 기준으로 스크롤
    active.startContainer.parentElement?.scrollIntoView({ block: "center" });
  }, [findActiveIdx, findCount, activeFindRegex, bodyNode]);

  return {
    findOpen,
    findInput,
    setFindInput,
    findTerm,
    setFindTerm,
    findCount,
    findActiveIdx,
    findInputRef,
    findComposingRef,
    closeFind,
    handleFindToggle,
    handleFindNav,
    handleFindInputKeyDown,
    handlePanelKeyDown,
  };
}

export type PreviewFind = ReturnType<typeof usePreviewFind>;
