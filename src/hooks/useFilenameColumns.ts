import { useState, useRef, useEffect, useCallback } from "react";

/**
 * 파일명 검색 결과의 Everything식 컬럼 상태.
 * - 너비: 이름·경로·크기·수정일시 개별 리사이즈 (type은 남는 공간이라 제외)
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

  const [widths, setWidths] = useState<FilenameColumnWidths>(loadWidths);
  const widthsRef = useRef(widths);
  useEffect(() => {
    widthsRef.current = widths;
  }, [widths]);
  useEffect(() => {
    const t = setTimeout(() => {
      try {
        localStorage.setItem(WIDTH_KEY, JSON.stringify(widths));
      } catch {
        /* private mode 등 무시 */
      }
    }, 250);
    return () => clearTimeout(t);
  }, [widths]);

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

  // 창이 좁아져 리사이즈 컬럼 총합이 예산을 초과하면 비율로 축소해 총폭=창폭 유지.
  const fitToContainer = useCallback(() => {
    const el = containerRef.current;
    if (!el) return;
    setWidths((prev) => {
      const budget = fixedBudget(el.clientWidth, visibleRef.current);
      const total = sumFixed(prev, visibleRef.current);
      if (budget <= 0 || total <= budget) return prev; // 여유 있으면 그대로
      const scale = budget / total;
      let changed = false;
      const next = { ...prev };
      for (const k of RESIZABLE_COLS) {
        if (k !== "name" && !visibleRef.current[k as ToggleableCol]) continue; // 숨긴 컬럼은 손대지 않음
        const scaled = Math.max(MIN[k], Math.round(prev[k] * scale));
        if (scaled !== prev[k]) changed = true;
        next[k] = scaled;
      }
      return changed ? next : prev; // 전부 MIN이라 더 못 줄이면 그대로(오버플로우 불가피)
    });
  }, []);

  // 콜백 ref — 노드가 붙는 시점(모드 전환 포함)에 ResizeObserver를 (재)연결.
  // useRef + useEffect([]) 로는 키워드→파일명 전환 시 노드가 뒤늦게 생겨 옵저버가 안 붙는다.
  const setContainer = useCallback(
    (node: HTMLDivElement | null) => {
      containerRef.current = node;
      roRef.current?.disconnect();
      roRef.current = null;
      if (node && typeof ResizeObserver !== "undefined") {
        const ro = new ResizeObserver(() => fitToContainer());
        ro.observe(node);
        roRef.current = ro;
        fitToContainer(); // 붙는 즉시 현재 폭에 맞춤
      }
    },
    [fitToContainer]
  );

  // 컬럼 표시/숨김 토글로 예산이 바뀌면 재맞춤(폭 변화가 없어 ResizeObserver가 안 뜀)
  useEffect(() => {
    fitToContainer();
  }, [visible, fitToContainer]);

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
      setWidths((prev) => ({ ...prev, [col]: Math.max(min, Math.min(max, startW + delta)) }));
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
    setWidths((prev) => {
      const target = Math.max(MIN[col], Math.min(MAX, Math.round(px)));
      return { ...prev, [col]: Math.min(target, clampToBudget(col, prev)) };
    });
  }, [clampToBudget]);

  const resetWidths = useCallback(() => setWidths({ ...DEFAULTS }), []);

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
