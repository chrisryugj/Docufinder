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

/** 헤더와 각 행이 공유하는 grid 컬럼 정의 — 표시 컬럼만 포함(이름 첫째, 유형 1fr 마지막) */
export function buildFilenameGridTemplate(visible: FilenameVisible): string {
  const parts = ["var(--fn-name)"];
  if (visible.path) parts.push("var(--fn-path)");
  if (visible.size) parts.push("var(--fn-size)");
  if (visible.time) parts.push("var(--fn-time)");
  parts.push("minmax(60px, 1fr)"); // 유형 + 액션
  return parts.join(" ");
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
  useEffect(() => {
    try {
      localStorage.setItem(VISIBLE_KEY, JSON.stringify(visible));
    } catch {
      /* 무시 */
    }
  }, [visible]);

  // 헤더 컬럼 경계 드래그 → 해당 컬럼 너비 조정
  const startResize = useCallback((col: ResizableCol, e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    const startX = e.clientX;
    const startW = widthsRef.current[col];
    const min = MIN[col];
    const onMove = (ev: MouseEvent) => {
      const delta = ev.clientX - startX;
      setWidths((prev) => ({ ...prev, [col]: Math.max(min, Math.min(MAX, startW + delta)) }));
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
  }, []);

  // 컬럼 경계 더블클릭 자동맞춤에서 사용 — 측정한 px로 너비 설정
  const setColumnWidth = useCallback((col: ResizableCol, px: number) => {
    setWidths((prev) => ({ ...prev, [col]: Math.max(MIN[col], Math.min(MAX, Math.round(px))) }));
  }, []);

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
