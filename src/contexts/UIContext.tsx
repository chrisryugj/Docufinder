import React, { createContext, useContext, useRef, useState, useCallback, useEffect, useMemo, type ReactNode, type Dispatch, type SetStateAction } from "react";
import { useToast, useTheme } from "../hooks";
import { useFirstRun } from "../hooks/useFirstRun";
import { useBookmarks } from "../hooks/useBookmarks";
import { useFileTags } from "../hooks/useFileTags";
import type { ToastData, ToastType } from "../components/ui/Toast";
import type { AddFolderResult } from "../types/index";

// ── Types ──────────────────────────────────────────────

export interface UIContextValue {
  // Toast
  toasts: ToastData[];
  showToast: (message: string, type?: ToastType, duration?: number) => string;
  updateToast: (id: string, update: Partial<ToastData>) => void;
  dismissToast: (id: string) => void;

  // Theme
  setTheme: (theme: import("../hooks/useTheme").Theme) => void;

  // First Run
  showOnboarding: boolean;
  completeOnboarding: () => void;
  skipOnboarding: () => void;

  // Sidebar
  sidebarOpen: boolean;
  toggleSidebar: () => void;
  setSidebarOpen: Dispatch<SetStateAction<boolean>>;

  // Modals
  settingsOpen: boolean;
  setSettingsOpen: (open: boolean) => void;
  helpOpen: boolean;
  setHelpOpen: (open: boolean) => void;
  statsOpen: boolean;
  setStatsOpen: (open: boolean) => void;
  duplicateOpen: boolean;
  setDuplicateOpen: (open: boolean) => void;

  // Auto Index Prompt
  showAutoIndexPrompt: boolean;
  setShowAutoIndexPrompt: (show: boolean) => void;
  /** 최초 1회만 프롬프트 표시 (이미 표시했으면 무시) */
  tryShowAutoIndexPrompt: () => void;

  // Report (인덱싱 결과)
  reportResults: AddFolderResult[];
  setReportResults: Dispatch<SetStateAction<AddFolderResult[]>>;

  // Preview
  previewFilePath: string | null;
  setPreviewFilePath: Dispatch<SetStateAction<string | null>>;
  previewWidth: number;
  handlePreviewClose: () => void;
  handleResizeStart: (e: React.MouseEvent) => void;

  // Bookmarks
  bookmarks: ReturnType<typeof useBookmarks>["bookmarks"];
  addBookmark: ReturnType<typeof useBookmarks>["addBookmark"];
  removeBookmark: ReturnType<typeof useBookmarks>["removeBookmark"];
  isBookmarked: ReturnType<typeof useBookmarks>["isBookmarked"];

  // Tags
  allTags: ReturnType<typeof useFileTags>["allTags"];
  getFileTags: ReturnType<typeof useFileTags>["getFileTags"];
  previewTags: string[];
  tagSuggestions: string[];
  handleAddTag: (filePath: string, tag: string) => Promise<void>;
  handleRemoveTag: (filePath: string, tag: string) => Promise<void>;

  // Refs
  isMountedRef: React.RefObject<boolean>;
}

// ── Context ────────────────────────────────────────────

const UIContext = createContext<UIContextValue | null>(null);

export function useUIContext(): UIContextValue {
  const ctx = useContext(UIContext);
  if (!ctx) throw new Error("useUIContext must be used within UIProvider");
  return ctx;
}

// ── Provider ───────────────────────────────────────────

export function UIProvider({ children }: { children: ReactNode }) {
  // Mounted ref
  const isMountedRef = useRef(true);
  useEffect(() => () => { isMountedRef.current = false; }, []);

  // Toast
  const { toasts, showToast, updateToast, dismissToast } = useToast();

  // Theme
  const { setTheme } = useTheme();

  // First Run
  const firstRun = useFirstRun();

  // Sidebar
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const toggleSidebar = useCallback(() => setSidebarOpen((p) => !p), []);

  // Modals
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [helpOpen, setHelpOpen] = useState(false);
  const [statsOpen, setStatsOpen] = useState(false);
  const [duplicateOpen, setDuplicateOpen] = useState(false);

  // Auto Index Prompt
  const [showAutoIndexPrompt, setShowAutoIndexPrompt] = useState(false);
  const autoIndexPromptShownRef = useRef(false);
  const tryShowAutoIndexPrompt = useCallback(() => {
    if (!autoIndexPromptShownRef.current) {
      autoIndexPromptShownRef.current = true;
      setShowAutoIndexPrompt(true);
    }
  }, []);

  // Report
  const [reportResults, setReportResults] = useState<AddFolderResult[]>([]);

  // Preview
  const [previewFilePath, setPreviewFilePath] = useState<string | null>(null);
  const [previewWidth, setPreviewWidth] = useState(() => Math.max(380, Math.round(window.innerWidth * 0.3)));
  const previewWidthRef = useRef(previewWidth);
  useEffect(() => { previewWidthRef.current = previewWidth; }, [previewWidth]);
  const isResizingRef = useRef(false);
  const handlePreviewClose = useCallback(() => {
    setPreviewFilePath(null);
    // 선택된 결과 아이템 또는 검색창으로 포커스 복귀
    requestAnimationFrame(() => {
      const selectedEl = document.querySelector<HTMLElement>('[role="option"][aria-selected="true"]');
      if (selectedEl) {
        selectedEl.focus();
      } else {
        document.querySelector<HTMLInputElement>('[aria-label="검색어 입력"]')?.focus();
      }
    });
  }, []);
  const handleResizeStart = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    isResizingRef.current = true;
    const startX = e.clientX;
    const startWidth = previewWidthRef.current;
    const onMove = (ev: MouseEvent) => {
      if (!isResizingRef.current) return;
      const delta = startX - ev.clientX;
      setPreviewWidth(Math.max(380, Math.min(Math.round(window.innerWidth * 0.5), startWidth + delta)));
    };
    const onUp = () => {
      isResizingRef.current = false;
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

  // Bookmarks
  const { bookmarks, addBookmark, removeBookmark, isBookmarked } = useBookmarks({ showToast });

  // Tags
  const { allTags, getFileTags, addTag, removeTag } = useFileTags(showToast);
  const [previewTags, setPreviewTags] = useState<string[]>([]);
  const tagSuggestions = useMemo(() => allTags.map((t) => t.tag), [allTags]);

  useEffect(() => {
    let cancelled = false;
    if (previewFilePath) {
      getFileTags(previewFilePath).then((tags) => {
        if (!cancelled) setPreviewTags(tags);
      });
    } else {
      setPreviewTags([]);
    }
    return () => { cancelled = true; };
  }, [previewFilePath, getFileTags]);

  const handleAddTag = useCallback(async (filePath: string, tag: string) => {
    await addTag(filePath, tag);
    const updated = await getFileTags(filePath);
    setPreviewTags(updated);
  }, [addTag, getFileTags]);

  const handleRemoveTag = useCallback(async (filePath: string, tag: string) => {
    await removeTag(filePath, tag);
    const updated = await getFileTags(filePath);
    setPreviewTags(updated);
  }, [removeTag, getFileTags]);

  const value: UIContextValue = {
    toasts, showToast, updateToast, dismissToast,
    setTheme,
    ...firstRun,
    sidebarOpen, toggleSidebar, setSidebarOpen,
    settingsOpen, setSettingsOpen,
    helpOpen, setHelpOpen,
    statsOpen, setStatsOpen,
    duplicateOpen, setDuplicateOpen,
    showAutoIndexPrompt, setShowAutoIndexPrompt, tryShowAutoIndexPrompt,
    reportResults, setReportResults,
    previewFilePath, setPreviewFilePath, previewWidth, handlePreviewClose, handleResizeStart,
    bookmarks, addBookmark, removeBookmark, isBookmarked,
    allTags, getFileTags, previewTags, tagSuggestions, handleAddTag, handleRemoveTag,
    isMountedRef,
  };

  return <UIContext.Provider value={value}>{children}</UIContext.Provider>;
}
