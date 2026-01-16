import { useRef, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

// Hooks
import { useSearch, useIndexStatus, useKeyboardShortcuts, useRecentSearches } from "./hooks";

// Components
import { Header, StatusBar, ErrorBanner } from "./components/layout";
import { SearchBar, SearchFilters, SearchResultList } from "./components/search";
import { Sidebar } from "./components/sidebar";
import { SettingsModal } from "./components/settings/SettingsModal";

function App() {
  const searchInputRef = useRef<HTMLInputElement>(null);
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [selectedIndex, setSelectedIndex] = useState<number>(-1);
  const [settingsOpen, setSettingsOpen] = useState(false);

  // 검색 상태
  const {
    query,
    setQuery,
    filteredResults,
    searchTime,
    isLoading,
    error: searchError,
    clearError: clearSearchError,
    searchMode,
    setSearchMode,
    filters,
    setFilters,
  } = useSearch({ debounceMs: 300 });

  // 인덱스 상태
  const {
    status,
    isIndexing,
    error: indexError,
    clearError: clearIndexError,
    addFolder,
  } = useIndexStatus();

  // 최근 검색
  const {
    searches: recentSearches,
    addSearch,
    removeSearch,
    clearSearches,
  } = useRecentSearches();

  // 에러 통합
  const error = searchError || indexError;
  const clearError = useCallback(() => {
    clearSearchError();
    clearIndexError();
  }, [clearSearchError, clearIndexError]);

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

  // 경로 복사
  const handleCopyPath = useCallback(async (path: string) => {
    try {
      await navigator.clipboard.writeText(path);
      // TODO: 토스트 알림 표시
    } catch (err) {
      console.error("Failed to copy path:", err);
    }
  }, []);

  // 폴더 열기
  const handleOpenFolder = useCallback(async (folderPath: string) => {
    try {
      await invoke("open_folder", { path: folderPath });
    } catch (err) {
      console.error("Failed to open folder:", err);
    }
  }, []);

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
    <div className="min-h-screen bg-gray-900 text-white">
      {/* 사이드바 */}
      <Sidebar
        isOpen={sidebarOpen}
        onToggle={toggleSidebar}
        watchedFolders={status?.watched_folders ?? []}
        onAddFolder={addFolder}
        recentSearches={recentSearches}
        onSelectSearch={handleSelectSearch}
        onRemoveSearch={removeSearch}
        onClearSearches={clearSearches}
      />

      {/* 메인 콘텐츠 (사이드바 열림에 따라 마진 조정) */}
      <div
        className={`flex flex-col min-h-screen transition-[margin] duration-200
          ${sidebarOpen ? "ml-[var(--sidebar-width)]" : "ml-0"}`}
      >
        {/* Header */}
        <Header
          onAddFolder={addFolder}
          onOpenSettings={() => setSettingsOpen(true)}
          isIndexing={isIndexing}
        />

        {/* Search Bar */}
        <div className="p-6 border-b border-gray-800">
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
          {error && <ErrorBanner message={error} onDismiss={clearError} />}
        </div>

        {/* 필터 바 (결과가 있을 때만 표시) */}
        {query && filteredResults.length > 0 && (
          <div className="px-6 border-b border-gray-800">
            <div className="max-w-4xl mx-auto">
              <SearchFilters
                filters={filters}
                onFiltersChange={setFilters}
              />
            </div>
          </div>
        )}

        {/* Results Area */}
        <main className="flex-1 px-6 py-4 overflow-auto">
          <div className="max-w-4xl mx-auto">
            <SearchResultList
              results={filteredResults}
              query={query}
              isLoading={isLoading}
              selectedIndex={selectedIndex}
              onOpenFile={handleOpenFile}
              onCopyPath={handleCopyPath}
              onOpenFolder={handleOpenFolder}
            />
          </div>
        </main>

        {/* Status Bar */}
        <StatusBar status={status} />
      </div>

      {/* Settings Modal */}
      <SettingsModal
        isOpen={settingsOpen}
        onClose={() => setSettingsOpen(false)}
      />
    </div>
  );
}

export default App;
