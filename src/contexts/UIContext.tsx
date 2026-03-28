import React, { createContext, useContext, useRef, useState, useCallback, useEffect, useMemo, type ReactNode, type Dispatch, type SetStateAction } from "react";
import { useToast, useTheme } from "../hooks";
import { useFirstRun } from "../hooks/useFirstRun";
import { useUpdater } from "../hooks/useUpdater";
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
  showDisclaimer: boolean;
  showOnboarding: boolean;
  acceptDisclaimer: () => void;
  completeOnboarding: () => void;
  skipOnboarding: () => void;
  exitApp: () => void;

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
  expiryOpen: boolean;
  setExpiryOpen: (open: boolean) => void;

  // Auto Index Prompt
  showAutoIndexPrompt: boolean;
  setShowAutoIndexPrompt: (show: boolean) => void;
  /** 최초 1회만 프롬프트 표시 (이미 표시했으면 무시) */
  tryShowAutoIndexPrompt: () => void;

  // Report (인덱싱 결과)
  reportResults: AddFolderResult[];
  setReportResults: Dispatch<SetStateAction<AddFolderResult[]>>;
  pendingHwpFiles: string[];
  setPendingHwpFiles: Dispatch<SetStateAction<string[]>>;

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

  // Updater
  updater: ReturnType<typeof useUpdater>;

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
  const [expiryOpen, setExpiryOpen] = useState(false);

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
  const [pendingHwpFiles, setPendingHwpFiles] = useState<string[]>([]);

  // Preview
  const [previewFilePath, setPreviewFilePath] = useState<string | null>(null);
  const [previewWidth, setPreviewWidth] = useState(() => Math.round(window.innerWidth * 0.3));
  const isResizingRef = useRef(false);
  const handlePreviewClose = useCallback(() => setPreviewFilePath(null), []);
  const handleResizeStart = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    isResizingRef.current = true;
    const startX = e.clientX;
    const startWidth = previewWidth;
    const onMove = (ev: MouseEvent) => {
      if (!isResizingRef.current) return;
      const delta = startX - ev.clientX;
      setPreviewWidth(Math.max(280, Math.min(Math.round(window.innerWidth * 0.5), startWidth + delta)));
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
  }, [previewWidth]);

  // Bookmarks
  const { bookmarks, addBookmark, removeBookmark, isBookmarked } = useBookmarks({ showToast });

  // Tags
  const { allTags, getFileTags, addTag, removeTag } = useFileTags(showToast);
  const [previewTags, setPreviewTags] = useState<string[]>([]);
  const tagSuggestions = useMemo(() => allTags.map((t) => t.tag), [allTags]);

  useEffect(() => {
    if (previewFilePath) {
      getFileTags(previewFilePath).then(setPreviewTags);
    } else {
      setPreviewTags([]);
    }
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

  // Updater
  const updater = useUpdater();

  const value: UIContextValue = {
    toasts, showToast, updateToast, dismissToast,
    setTheme,
    ...firstRun,
    sidebarOpen, toggleSidebar, setSidebarOpen,
    settingsOpen, setSettingsOpen,
    helpOpen, setHelpOpen,
    statsOpen, setStatsOpen,
    duplicateOpen, setDuplicateOpen,
    expiryOpen, setExpiryOpen,
    showAutoIndexPrompt, setShowAutoIndexPrompt, tryShowAutoIndexPrompt,
    reportResults, setReportResults,
    pendingHwpFiles, setPendingHwpFiles,
    previewFilePath, setPreviewFilePath, previewWidth, handlePreviewClose, handleResizeStart,
    bookmarks, addBookmark, removeBookmark, isBookmarked,
    allTags, getFileTags, previewTags, tagSuggestions, handleAddTag, handleRemoveTag,
    updater,
    isMountedRef,
  };

  return <UIContext.Provider value={value}>{children}</UIContext.Provider>;
}
