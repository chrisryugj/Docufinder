import { useState, useRef, useEffect, useCallback, useMemo } from "react";

/**
 * 파일명 검색 결과의 Everything식 컬럼 상태.
 * - 너비: 이름·경로·크기·수정일시 개별 리사이즈 (type은 남는 공간이라 제외)
 *   선호 폭(사용자가 정한 값, 영속)과 표시 폭(컨테이너 폭에 맞춘 렌더값)을 분리 —
 *   창을 좁혀도 선호 폭은 불변이라 다시 넓히면 자동 복원된다. (v3.3.5)
 * - 표시: 경로·크기·수정일시 토글 (이름·유형은 항상 표시)
 * - 정렬: 컬럼 클릭 오름/내림
 * 전부 localStorage에 기억한다.
 */
export type FilenameColKey = "name" | "path" | "size" | "time" | "type";
export type ResizableCol = "name" | "path" | "size" | "time";
export type ToggleableCol = "path" | "size" | "time";
export type FilenameSortField = FilenameColKey;

export type FilenameColumnWidths = Record<ResizableCol, number>;
export type FilenameVisible = Record<ToggleableCol, boolean>;
export interface FilenameSort {
  field: FilenameSortField;
  dir: "asc" | "desc";
}

const WIDTH_KEY = "docufinder_filename_columns";
const SORT_KEY = "docufinder_filename_sort";
const VISIBLE_KEY = "docufinder_filename_visible";

const DEFAULTS: FilenameColumnWidths = { name: 260, path: 360, size: 96, time: 150 };
const MIN: FilenameColumnWidths = { name: 120, path: 140, size: 64, time: 96 };
const MAX = 900;
const DEFAULT_VISIBLE: FilenameVisible = { path: true, size: true, time: true };

// 헤더/행 레이아웃 상수 — 컨테이너 폭에서 리사이즈 컬럼이 쓸 수 있는 공간을 산출할 때 사용.
const TYPE_MIN = 60; // 유형 컬럼(minmax) 최소폭 — 항상 이만큼은 오른쪽 끝에 남긴다
const COL_GAP = 8; // gap-2
const PAD_X = 20; // px-2.5 좌우 합
const RESIZABLE_COLS: ResizableCol[] = ["name", "path", "size", "time"];

/** 헤더와 각 행이 공유하는 grid 컬럼 정의 — 표시 컬럼만 포함(이름 첫째, 유형 1fr 마지막) */
export function buildFilenameGridTemplate(visible: FilenameVisible): string {
  const parts = ["var(--fn-name)"];
  if (visible.path) parts.push("var(--fn-path)");
  if (visible.size) parts.push("var(--fn-size)");
  if (visible.time) parts.push("var(--fn-time)");
  parts.push(`minmax(${TYPE_MIN}px, 1fr)`); // 유형 + 액션 — 남는 공간을 채워 창 폭에 맞춘다
  return parts.join(" ");
}

/** 표시 중인 리사이즈 컬럼 너비 합 (숨긴 컬럼은 grid에서 빠지므로 제외) */
function sumFixed(w: FilenameColumnWidths, visible: FilenameVisible): number {
  let s = w.name;
  if (visible.path) s += w.path;
  if (visible.size) s += w.size;
  if (visible.time) s += w.time;
  return s;
}

/** 유형 컬럼 최소폭을 남기고 리사이즈 컬럼들이 총합으로 쓸 수 있는 최대 px */
function fixedBudget(containerW: number, visible: FilenameVisible): number {
  const cols = 1 + (visible.path ? 1 : 0) + (visible.size ? 1 : 0) + (visible.time ? 1 : 0) + 1;
  const gaps = (cols - 1) * COL_GAP;
  return containerW - PAD_X - gaps - TYPE_MIN;
}

/**
 * 선호 폭 총합이 예산을 초과할 때의 표시 폭 — 비율 축소하되 MIN에 걸린 컬럼의
 * 부족분은 남은 컬럼이 더 줄어 흡수한다(미재분배 시 트랙 총합이 예산을 넘어
 * 컬럼이 컨테이너 밖으로 삐져나간다). 전부 MIN이면 오버플로 불가피 — 표시는
 * 리스트 래퍼의 overflow clip이 막는다.
 */
function shrinkToBudget(
  pref: FilenameColumnWidths,
  visible: FilenameVisible,
  budget: number
): FilenameColumnWidths {
  const cols = RESIZABLE_COLS.filter((k) => k === "name" || visible[k as ToggleableCol]);
  const next = { ...pref };
  const atMin = new Set<ResizableCol>();
  for (;;) {
    const free = cols.filter((k) => !atMin.has(k));
    const fixed = cols.length === free.length ? 0 : [...atMin].reduce((s, k) => s + MIN[k], 0);
    const freeSum = free.reduce((s, k) => s + pref[k], 0);
    const scale = freeSum > 0 ? (budget - fixed) / freeSum : 0;
    const newlyMin = free.filter((k) => pref[k] * scale < MIN[k]);
    if (newlyMin.length === 0) {
      for (const k of free) next[k] = Math.floor(pref[k] * scale);
      for (const k of atMin) next[k] = MIN[k];
      return next;
    }
    for (const k of newlyMin) atMin.add(k);
    if (atMin.size === cols.length) {
      for (const k of cols) next[k] = MIN[k];
      return next;
    }
  }
}

function sane(v: unknown, def: number, min: number): number {
  const n = Number(v);
  return Number.isFinite(n) ? Math.max(min, Math.min(MAX, n)) : def;
}

function loadWidths(): FilenameColumnWidths {
  try {
    const p = JSON.parse(localStorage.getItem(WIDTH_KEY) || "{}");
    return {
      name: sane(p.name, DEFAULTS.name, MIN.name),
      path: sane(p.path, DEFAULTS.path, MIN.path),
      size: sane(p.size, DEFAULTS.size, MIN.size),
      time: sane(p.time, DEFAULTS.time, MIN.time),
    };
  } catch {
    return { ...DEFAULTS };
  }
}

function loadSort(): FilenameSort | null {
  try {
    const raw = localStorage.getItem(SORT_KEY);
    if (!raw) return null;
    const p = JSON.parse(raw);
    const fields: FilenameSortField[] = ["name", "path", "size", "time", "type"];
    if (fields.includes(p.field) && (p.dir === "asc" || p.dir === "desc")) {
      return { field: p.field, dir: p.dir };
    }
  } catch {
    /* 무시 */
  }
  return null;
}

function loadVisible(): FilenameVisible {
  try {
    const p = JSON.parse(localStorage.getItem(VISIBLE_KEY) || "{}");
    return {
      path: p.path !== false,
      size: p.size !== false,
      time: p.time !== false,
    };
  } catch {
    return { ...DEFAULT_VISIBLE };
  }
}

export function useFilenameColumns() {
  // 결과 리스트 래퍼 — 리사이즈/맞춤 계산의 기준 폭. SearchResultList가 콜백 ref를 붙인다.
  const containerRef = useRef<HTMLDivElement | null>(null);
  const roRef = useRef<ResizeObserver | null>(null);

  // 선호 폭 — 사용자가 드래그/자동맞춤/초기화로 정한 값. 이것만 영속된다.
  // 창 축소에 따른 맞춤은 아래 표시 폭(widths) 계산에서만 일어난다.
  const [prefWidths, setPrefWidths] = useState<FilenameColumnWidths>(loadWidths);
  const widthsRef = useRef(prefWidths);
  useEffect(() => {
    widthsRef.current = prefWidths;
  }, [prefWidths]);
  useEffect(() => {
    const t = setTimeout(() => {
      try {
        localStorage.setItem(WIDTH_KEY, JSON.stringify(prefWidths));
      } catch {
        /* private mode 등 무시 */
      }
    }, 250);
    return () => clearTimeout(t);
  }, [prefWidths]);

  const [sort, setSort] = useState<FilenameSort | null>(loadSort);
  useEffect(() => {
    try {
      if (sort) localStorage.setItem(SORT_KEY, JSON.stringify(sort));
      else localStorage.removeItem(SORT_KEY);
    } catch {
      /* 무시 */
    }
  }, [sort]);

  const [visible, setVisible] = useState<FilenameVisible>(loadVisible);
  const visibleRef = useRef(visible);
  useEffect(() => {
    visibleRef.current = visible;
  }, [visible]);
  useEffect(() => {
    try {
      localStorage.setItem(VISIBLE_KEY, JSON.stringify(visible));
    } catch {
      /* 무시 */
    }
  }, [visible]);

  // 한 컬럼의 최대 허용폭 — 유형 컬럼 최소폭이 남도록(=총폭이 창 폭을 넘지 않도록) 제한.
  // 컨테이너가 아직 없으면 기존 MAX 상한만 적용.
  const clampToBudget = useCallback((col: ResizableCol, w: FilenameColumnWidths): number => {
    const el = containerRef.current;
    if (!el) return MAX;
    const others = sumFixed(w, visibleRef.current) - w[col];
    return Math.max(MIN[col], Math.min(MAX, fixedBudget(el.clientWidth, visibleRef.current) - others));
  }, []);

  // 컨테이너 폭 — ResizeObserver가 갱신. 표시 폭 재계산 트리거 전용(선호 폭은 불변).
  const [containerW, setContainerW] = useState<number | null>(null);

  // 표시 폭 — 선호 폭 총합이 예산을 초과하면 비율 축소(MIN 하한), 여유 있으면 선호 폭
  // 그대로. 선호 폭을 건드리지 않으므로 창 축소→재확대 왕복이 자동 복원된다.
  const widths = useMemo(() => {
    if (containerW == null) return prefWidths;
    const budget = fixedBudget(containerW, visible);
    const total = sumFixed(prefWidths, visible);
    if (budget <= 0 || total <= budget) return prefWidths;
    return shrinkToBudget(prefWidths, visible, budget);
  }, [prefWidths, containerW, visible]);

  // 콜백 ref — 노드가 붙는 시점(모드 전환 포함)에 ResizeObserver를 (재)연결.
  // useRef + useEffect([]) 로는 키워드→파일명 전환 시 노드가 뒤늦게 생겨 옵저버가 안 붙는다.
  const setContainer = useCallback((node: HTMLDivElement | null) => {
    containerRef.current = node;
    roRef.current?.disconnect();
    roRef.current = null;
    if (node && typeof ResizeObserver !== "undefined") {
      const ro = new ResizeObserver(() => setContainerW(node.clientWidth));
      ro.observe(node);
      roRef.current = ro;
      setContainerW(node.clientWidth); // 붙는 즉시 현재 폭 반영
    }
  }, []);

  // 헤더 컬럼 경계 드래그 → 해당 컬럼 너비 조정 (유형 컬럼이 오른쪽 끝에 남도록 clamp)
  const startResize = useCallback((col: ResizableCol, e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    const startX = e.clientX;
    const startW = widthsRef.current[col];
    const min = MIN[col];
    const onMove = (ev: MouseEvent) => {
      const delta = ev.clientX - startX;
      const max = clampToBudget(col, widthsRef.current);
      setPrefWidths((prev) => ({ ...prev, [col]: Math.max(min, Math.min(max, startW + delta)) }));
    };
    const onUp = () => {
      document.removeEventListener("mousemove", onMove);
      document.removeEventListener("mouseup", onUp);
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
    };
    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
  }, [clampToBudget]);

  // 컬럼 경계 더블클릭 자동맞춤에서 사용 — 측정한 px로 너비 설정(창 폭 예산 내로 clamp)
  const setColumnWidth = useCallback((col: ResizableCol, px: number) => {
    setPrefWidths((prev) => {
      const target = Math.max(MIN[col], Math.min(MAX, Math.round(px)));
      return { ...prev, [col]: Math.min(target, clampToBudget(col, prev)) };
    });
  }, [clampToBudget]);

  const resetWidths = useCallback(() => setPrefWidths({ ...DEFAULTS }), []);

  // 컬럼 헤더 클릭 정렬 — 같은 컬럼 재클릭 시 방향 토글 (날짜/크기는 첫 클릭 내림차순)
  const toggleSort = useCallback((field: FilenameSortField) => {
    setSort((prev) =>
      prev?.field === field
        ? { field, dir: prev.dir === "asc" ? "desc" : "asc" }
        : { field, dir: field === "time" || field === "size" ? "desc" : "asc" }
    );
  }, []);

  const toggleColumn = useCallback((col: ToggleableCol) => {
    setVisible((prev) => ({ ...prev, [col]: !prev[col] }));
  }, []);

  const gridTemplate = buildFilenameGridTemplate(visible);

  return {
    containerRef: setContainer,
    widths,
    visible,
    sort,
    gridTemplate,
    startResize,
    setColumnWidth,
    resetWidths,
    toggleSort,
    toggleColumn,
  };
}
