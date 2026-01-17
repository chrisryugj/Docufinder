import { useRef, useState, useCallback, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Settings } from "./types/settings";

// Hooks
import { useSearch, useIndexStatus, useKeyboardShortcuts, useRecentSearches, useExport, useTheme } from "./hooks";

// Components
import { Header, StatusBar, ErrorBanner } from "./components/layout";
import { SearchBar, SearchFilters, SearchResultList } from "./components/search";
import { Sidebar } from "./components/sidebar";
import { SettingsModal } from "./components/settings/SettingsModal";

function App() {
  const searchInputRef = useRef<HTMLInputElement>(null);
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [selectedIndex, setSelectedIndex] = useState<number>(-1);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [minConfidence, setMinConfidence] = useState(0);

  // 테마
  const { setTheme } = useTheme();

  // 검색 상태
  const {
    query,
    setQuery,
    filteredResults,
    groupedResults,
    searchTime,
    isLoading,
    error: searchError,
    clearError: clearSearchError,
    searchMode,
    setSearchMode,
    filters,
    setFilters,
    viewMode,
    setViewMode,
  } = useSearch({ debounceMs: 300, minConfidence });

  // 인덱스 상태
  const {
    status,
    isIndexing,
    error: indexError,
    clearError: clearIndexError,
    addFolder,
    removeFolder,
  } = useIndexStatus();

  // 최근 검색
  const {
    searches: recentSearches,
    addSearch,
    removeSearch,
    clearSearches,
  } = useRecentSearches();

  // 내보내기
  const { exportToCSV, copyToClipboard, toast, showToast } = useExport();

  // 에러 통합
  const error = searchError || indexError;
  const clearError = useCallback(() => {
    clearSearchError();
    clearIndexError();
  }, [clearSearchError, clearIndexError]);

  // 설정 로드 (최소 신뢰도 적용)
  useEffect(() => {
    const loadSettings = async () => {
      try {
        const settings = await invoke<Settings>("get_settings");
        setMinConfidence(settings.min_confidence ?? 0);
      } catch (err) {
        console.warn("Failed to load settings:", err);
      }
    };

    loadSettings();
  }, []);

  // 윈도우 포커스 복귀 시 검색창 포커스 재설정 (IME 전환 안정화)
  useEffect(() => {
    let unlisten: (() => void) | null = null;

    const resetSearchFocus = () => {
      if (settingsOpen) return;
      const input = searchInputRef.current;
      if (!input) return;

      const activeElement = document.activeElement;
      const isEditable =
        activeElement?.tagName === "INPUT" ||
        activeElement?.tagName === "TEXTAREA" ||
        (activeElement instanceof HTMLElement && activeElement.isContentEditable);

      if (isEditable && activeElement !== input) {
        return;
      }

      if (activeElement === input) {
        input.blur();
      }

      requestAnimationFrame(() => {
        input.focus();
      });
    };

    const setup = async () => {
      const window = getCurrentWindow();
      try {
        if (await window.isFocused()) {
          resetSearchFocus();
        }
        unlisten = await window.onFocusChanged(({ payload }) => {
          if (payload) {
            resetSearchFocus();
          }
        });
      } catch (err) {
        console.warn("Failed to register focus handler:", err);
      }
    };

    setup();

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, [settingsOpen]);

  useEffect(() => {
    if (searchMode !== "hybrid" && filters.keywordOnly) {
      setFilters({ ...filters, keywordOnly: false });
    }
  }, [searchMode, filters, setFilters]);

  // 파일 열기
  const handleOpenFile = useCallback(
    async (filePath: string, page?: number | null) => {
      try {
        await invoke("open_file", { path: filePath, page: page ?? null });
      } catch (err) {
        console.error("Failed to open file:", err);
      }
    },
    []
  );

  // 경로 복사 (\\?\ 접두사 제거)
  const handleCopyPath = useCallback(async (path: string) => {
    try {
      const cleanPath = path.replace(/^\\\\\?\\/, "");
      await navigator.clipboard.writeText(cleanPath);
      showToast("경로가 복사되었습니다", "success");
    } catch (err) {
      console.error("Failed to copy path:", err);
      showToast("경로 복사 실패", "error");
    }
  }, [showToast]);

  // 폴더 열기 (\\?\ 접두사 제거)
  const handleOpenFolder = useCallback(async (folderPath: string) => {
    try {
      const cleanPath = folderPath.replace(/^\\\\\?\\/, "");
      await invoke("open_folder", { path: cleanPath });
    } catch (err) {
      console.error("Failed to open folder:", err);
      showToast("폴더 열기 실패", "error");
    }
  }, [showToast]);

  // 사이드바 토글
  const toggleSidebar = useCallback(() => {
    setSidebarOpen((prev) => !prev);
  }, []);

  // 검색어 선택 (최근 검색에서)
  const handleSelectSearch = useCallback(
    (searchQuery: string) => {
      setQuery(searchQuery);
      searchInputRef.current?.focus();
    },
    [setQuery]
  );

  // 검색 실행 시 최근 검색에 추가 (300ms 디바운스 후 결과가 있을 때)
  const handleQueryChange = useCallback(
    (newQuery: string) => {
      setQuery(newQuery);
      // 검색어가 2자 이상이면 최근 검색에 추가 (지연)
      if (newQuery.trim().length >= 2) {
        const timeoutId = setTimeout(() => {
          addSearch(newQuery.trim());
        }, 1000);
        return () => clearTimeout(timeoutId);
      }
    },
    [setQuery, addSearch]
  );

  // 키보드 단축키
  useKeyboardShortcuts(
    {
      onFocusSearch: () => {
        searchInputRef.current?.focus();
        searchInputRef.current?.select();
      },
      onEscape: () => {
        if (selectedIndex >= 0) {
          setSelectedIndex(-1);
        } else {
          setQuery("");
          searchInputRef.current?.blur();
        }
      },
      onToggleSidebar: toggleSidebar,
      onArrowUp: () => {
        setSelectedIndex((prev) => Math.max(0, prev - 1));
      },
      onArrowDown: () => {
        setSelectedIndex((prev) =>
          Math.min(filteredResults.length - 1, prev + 1)
        );
      },
      onEnter: () => {
        if (selectedIndex >= 0 && selectedIndex < filteredResults.length) {
          const result = filteredResults[selectedIndex];
          handleOpenFile(result.file_path, result.page_number);
        }
      },
      onCopy: () => {
        if (selectedIndex >= 0 && selectedIndex < filteredResults.length) {
          const result = filteredResults[selectedIndex];
          handleCopyPath(result.file_path);
        }
      },
    },
    searchInputRef
  );

  // 결과가 변경되면 선택 초기화
  const prevResultsLength = useRef(filteredResults.length);
  if (prevResultsLength.current !== filteredResults.length) {
    prevResultsLength.current = filteredResults.length;
    if (selectedIndex >= filteredResults.length) {
      setSelectedIndex(filteredResults.length > 0 ? 0 : -1);
    }
  }

  return (
    <div className="min-h-screen" style={{ backgroundColor: 'var(--color-bg-primary)', color: 'var(--color-text-primary)' }}>
      {/* 사이드바 */}
      <Sidebar
        isOpen={sidebarOpen}
        onToggle={toggleSidebar}
        watchedFolders={status?.watched_folders ?? []}
        onAddFolder={addFolder}
        onRemoveFolder={removeFolder}
        recentSearches={recentSearches}
        onSelectSearch={handleSelectSearch}
        onRemoveSearch={removeSearch}
        onClearSearches={clearSearches}
      />

      {/* 메인 콘텐츠 (사이드바 열림에 따라 전체 이동) */}
      <div
        className={`flex flex-col h-screen transition-all duration-300 ease-in-out
          ${sidebarOpen ? "pl-[var(--sidebar-width)]" : "pl-0"}`}
      >
        {/* Sticky Header - 사이드바와 함께 이동 */}
        <div className="sticky top-0 z-20 bg-[var(--color-bg-primary)]/90 backdrop-blur-md border-b" style={{ borderColor: 'var(--color-border)' }}>
          <Header
            onAddFolder={addFolder}
            onOpenSettings={() => setSettingsOpen(true)}
            isIndexing={isIndexing}
            isSidebarOpen={sidebarOpen}
          />
        </div>

        {/* Scrollable Content Area */}
        <div className="flex-1 overflow-y-auto overflow-x-hidden">
          {/* Search Bar Area */}
          <div className="px-6 py-8">
            <SearchBar
              ref={searchInputRef}
              query={query}
              onQueryChange={handleQueryChange}
              searchMode={searchMode}
              onSearchModeChange={setSearchMode}
              isLoading={isLoading}
              status={status}
              resultCount={filteredResults.length}
              searchTime={searchTime}
            />

            {/* 에러 메시지 */}
            {error && <div className="mt-4"><ErrorBanner message={error} onDismiss={clearError} /></div>}
          </div>

          {/* 필터 바 (결과가 있을 때만 표시) */}
          {query && filteredResults.length > 0 && (
            <div className="sticky top-0 z-10 px-6 py-2 bg-[var(--color-bg-primary)]/95 backdrop-blur border-y" style={{ borderColor: 'var(--color-border)' }}>
              <div className="max-w-4xl mx-auto">
              <SearchFilters
                filters={filters}
                onFiltersChange={setFilters}
                viewMode={viewMode}
                onViewModeChange={setViewMode}
                resultCount={filteredResults.length}
                searchMode={searchMode}
              />
              </div>
            </div>
          )}

          {/* Results Area */}
          <main className="px-6 pb-20">
            <div className="max-w-4xl mx-auto mt-4">
              <SearchResultList
                results={filteredResults}
                groupedResults={groupedResults}
                viewMode={viewMode}
                query={query}
                isLoading={isLoading}
                selectedIndex={selectedIndex}
                onOpenFile={handleOpenFile}
                onCopyPath={handleCopyPath}
                onOpenFolder={handleOpenFolder}
                onExportCSV={() => exportToCSV(filteredResults, query)}
                onCopyAll={() => copyToClipboard(filteredResults, query)}
              />
            </div>
          </main>
        </div>

        {/* Status Bar (Fixed at bottom) */}
        <StatusBar status={status} />
      </div>

      {/* Settings Modal */}
      <SettingsModal
        isOpen={settingsOpen}
        onClose={() => setSettingsOpen(false)}
        onThemeChange={setTheme}
        onSettingsSaved={(settings) => setMinConfidence(settings.min_confidence ?? 0)}
      />

      {/* Toast */}
      {toast && (
        <div
          className="fixed bottom-20 right-6 px-4 py-2 rounded-lg text-sm z-50 text-white"
          style={{
            backgroundColor: toast.type === "success" ? 'var(--color-success)' : 'var(--color-error)',
            boxShadow: 'var(--shadow-lg)'
          }}
          role="alert"
        >
          {toast.message}
        </div>
      )}
    </div>
  );
}

export default App;
