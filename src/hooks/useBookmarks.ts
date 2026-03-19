import { useState, useCallback, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

export interface BookmarkInfo {
  id: number;
  file_path: string;
  file_name: string;
  content_preview: string;
  page_number: number | null;
  location_hint: string | null;
  note: string | null;
  created_at: number;
}

interface UseBookmarksOptions {
  showToast?: (message: string, type: "success" | "error") => void;
}

export function useBookmarks({ showToast }: UseBookmarksOptions = {}) {
  const [bookmarks, setBookmarks] = useState<BookmarkInfo[]>([]);

  const loadBookmarks = useCallback(async () => {
    try {
      const result = await invoke<BookmarkInfo[]>("get_bookmarks");
      setBookmarks(result);
    } catch {
      // 테이블 미생성 시 무시
    }
  }, []);

  useEffect(() => {
    loadBookmarks();
  }, [loadBookmarks]);

  const addBookmark = useCallback(async (
    filePath: string,
    contentPreview: string,
    pageNumber?: number | null,
    locationHint?: string | null,
    note?: string | null,
  ) => {
    try {
      await invoke<number>("add_bookmark", {
        filePath,
        contentPreview,
        pageNumber: pageNumber ?? null,
        locationHint: locationHint ?? null,
        note: note ?? null,
      });
      await loadBookmarks();
      showToast?.("북마크가 추가되었습니다", "success");
    } catch (e) {
      showToast?.("북마크 추가 실패", "error");
    }
  }, [loadBookmarks, showToast]);

  const removeBookmark = useCallback(async (id: number) => {
    try {
      await invoke("remove_bookmark", { id });
      setBookmarks(prev => prev.filter(b => b.id !== id));
      showToast?.("북마크가 삭제되었습니다", "success");
    } catch {
      showToast?.("북마크 삭제 실패", "error");
    }
  }, [showToast]);

  const updateNote = useCallback(async (id: number, note: string | null) => {
    try {
      await invoke("update_bookmark_note", { id, note });
      setBookmarks(prev => prev.map(b => b.id === id ? { ...b, note } : b));
    } catch {
      showToast?.("메모 수정 실패", "error");
    }
  }, [showToast]);

  const isBookmarked = useCallback((filePath: string) => {
    return bookmarks.some(b => b.file_path === filePath);
  }, [bookmarks]);

  return {
    bookmarks,
    addBookmark,
    removeBookmark,
    updateNote,
    isBookmarked,
    loadBookmarks,
  };
}
